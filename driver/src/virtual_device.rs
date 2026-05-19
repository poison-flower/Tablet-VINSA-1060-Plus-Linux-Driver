use std::collections::HashMap;
use std::io::Error;
use std::sync::{Arc, RwLock};

use evdev::{
    AbsInfo, AbsoluteAxisType, AttributeSet, EventType, InputEvent, Key, Synchronization,
    UinputAbsSetup,
    uinput::{VirtualDevice, VirtualDeviceBuilder},
};

use crate::config::AppConfig;

#[derive(Default)]
pub struct RawDataReader {
    pub data: Vec<u8>,
}

impl RawDataReader {
    const X_AXIS_HIGH: usize = 1;
    const X_AXIS_LOW: usize = 2;
    const Y_AXIS_HIGH: usize = 3;
    const Y_AXIS_LOW: usize = 4;
    const PRESSURE_HIGH: usize = 5;
    const PRESSURE_LOW: usize = 6;
    const PEN_BUTTONS: usize = 9;
    const TABLET_BUTTONS_HIGH: usize = 12;
    const TABLET_BUTTONS_LOW: usize = 11;

    pub fn new() -> Self {
        RawDataReader {
            data: vec![0u8; 64],
        }
    }

    fn u16_from_2_u8(&self, high: u8, low: u8) -> u16 {
        (high as u16) << 8 | low as u16
    }

    fn i16_from_2_u8(&self, high: u8, low: u8) -> i16 {
        ((high as u16) << 8 | low as u16) as i16
    }

    fn x_axis(&self) -> i32 {
        let raw = self.i16_from_2_u8(self.data[Self::X_AXIS_HIGH], self.data[Self::X_AXIS_LOW]);
        raw as i32
    }

    fn y_axis(&self) -> i32 {
        let raw = self.i16_from_2_u8(self.data[Self::Y_AXIS_HIGH], self.data[Self::Y_AXIS_LOW]);
        raw as i32
    }

    fn pressure(&self) -> i32 {
        self.u16_from_2_u8(
            self.data[Self::PRESSURE_HIGH],
            self.data[Self::PRESSURE_LOW],
        ) as i32
    }

    fn tablet_buttons_as_binary_flags(&self) -> u16 {
        let idx_h = Self::TABLET_BUTTONS_HIGH;
        let idx_l = Self::TABLET_BUTTONS_LOW;

        if idx_h >= self.data.len() || idx_l >= self.data.len() {
            return 0 | (0xcc << 8);
        }

        self.u16_from_2_u8(self.data[idx_h], self.data[idx_l]) | (0xcc << 8)
    }

    fn pen_buttons(&self) -> u8 {
        self.data[Self::PEN_BUTTONS]
    }
}

pub struct DeviceDispatcher {
    config: Arc<RwLock<AppConfig>>,
    tablet_last_raw_pressed_buttons: u16,
    pen_last_raw_pressed_button: u8,
    last_pressed_media_button: u8,
    media_button_id_to_key_code_map: HashMap<u8, Vec<Key>>,
    tablet_button_id_to_key_code_map: HashMap<u8, Vec<Key>>,
    pen_button_id_to_key_code_map: HashMap<u8, Vec<Key>>,
    virtual_pen: VirtualDevice,
    virtual_keyboard: VirtualDevice,
    media_keyboard: VirtualDevice,
    was_touching: bool,
    last_x: f32,
    last_y: f32,
    media_button_width: i32,
}

impl DeviceDispatcher {
    const PRESSED: i32 = 1;
    const RELEASED: i32 = 0;
    const HOLD: i32 = 2;
    const MAX_X: i32 = 4095;
    const MAX_Y: i32 = 4095;
    const MAX_PRESSURE: i32 = 8191;
    const RAW_PRESSURE_POINTS: i32 = 2000;
    const MEDIA_BUTTONS_COUNT: i32 = 10;

    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        let default_media_button_id_to_key_code_map: HashMap<u8, Vec<Key>> = [
            (0, vec![Key::KEY_MUTE]),
            (1, vec![Key::KEY_VOLUMEDOWN]),
            (2, vec![Key::KEY_VOLUMEUP]),
            (3, vec![Key::KEY_PLAYER]),
            (4, vec![Key::KEY_PLAYPAUSE]),
            (5, vec![Key::KEY_PREVIOUSSONG]),
            (6, vec![Key::KEY_NEXTSONG]),
            (7, vec![Key::KEY_HOME]),
            (8, vec![Key::KEY_CALC]),
            (9, vec![Key::KEY_LEFTMETA, Key::KEY_D]),
        ]
        .iter()
        .cloned()
        .collect();

        let default_tablet_button_id_to_key_code_map: HashMap<u8, Vec<Key>> = [
            (0, vec![Key::KEY_TAB]),                        // TAB
            (1, vec![Key::KEY_SPACE]),                      // SPACE
            (2, vec![Key::KEY_LEFTALT]),                    // ALT
            (3, vec![Key::KEY_LEFTCTRL]),                   // CTRL
            (4, vec![Key::KEY_PAGEUP]),                     // MOUSE UP
            (5, vec![Key::KEY_PAGEDOWN]),                   // MOUSE DOWN
            (6, vec![Key::KEY_LEFTBRACE]),                  // MOUSE AREA -
            (7, vec![Key::KEY_LEFTCTRL, Key::KEY_KPMINUS]), // CTRL- ZOOM
            (8, vec![Key::KEY_LEFTCTRL, Key::KEY_KPPLUS]),  // CTRL+ ZOOM
            (9, vec![Key::KEY_ESC]),                        // ESC CANCEL
            (12, vec![Key::KEY_B]),                         // TOGGLE MOUSE/TABLET
            (13, vec![Key::KEY_RIGHTBRACE]),                // MOUSE AREA +
        ]
        .iter()
        .cloned()
        .collect();

        let default_pen_button_id_to_key_code_map: HashMap<u8, Vec<Key>> =
            [(4, vec![Key::BTN_STYLUS]), (6, vec![Key::BTN_STYLUS2])]
                .iter()
                .cloned()
                .collect();

        DeviceDispatcher {
            config: config,
            tablet_last_raw_pressed_buttons: 0xFFFF,
            pen_last_raw_pressed_button: 0,
            last_pressed_media_button: 0,
            media_button_id_to_key_code_map: default_media_button_id_to_key_code_map.clone(),
            tablet_button_id_to_key_code_map: default_tablet_button_id_to_key_code_map.clone(),
            pen_button_id_to_key_code_map: default_pen_button_id_to_key_code_map.clone(),
            virtual_pen: Self::virtual_pen_builder(
                &default_pen_button_id_to_key_code_map
                    .values()
                    .flatten()
                    .cloned()
                    .collect::<Vec<Key>>(),
            )
            .expect("Error building virtual pen"),
            virtual_keyboard: Self::virtual_keyboard_builder(
                &default_tablet_button_id_to_key_code_map
                    .values()
                    .flatten()
                    .cloned()
                    .collect::<Vec<Key>>(),
            )
            .expect("Error building virtual keyboard"),
            media_keyboard: Self::virtual_keyboard_builder(
                &default_media_button_id_to_key_code_map
                    .values()
                    .flatten()
                    .cloned()
                    .collect::<Vec<Key>>(),
            )
            .expect("Error building media keyboard"),
            was_touching: false,
            last_x: (Self::MAX_X / 2) as f32,
            last_y: (Self::MAX_Y / 2) as f32,
            media_button_width: Self::MAX_X / Self::MEDIA_BUTTONS_COUNT,
        }
    }

    fn smooth_coordinates(&mut self, x: i32, y: i32) -> (i32, i32) {
        let target_x = x as f32;
        let target_y = y as f32;

        let dx = target_x - self.last_x;
        let dy = target_y - self.last_y;
        let dist_sq = dx * dx + dy * dy;
        let alpha = if dist_sq > 5000.0 {
            0.95
        } else if dist_sq > 1000.0 {
            0.6
        } else {
            0.3
        };

        self.last_x = self.last_x + (target_x - self.last_x) * alpha;
        self.last_y = self.last_y + (target_y - self.last_y) * alpha;

        (self.last_x as i32, self.last_y as i32)
    }

    pub fn syn(&mut self) -> Result<(), Error> {
        self.virtual_keyboard.emit(&[InputEvent::new(
            EventType::SYNCHRONIZATION,
            Synchronization::SYN_REPORT.0,
            0,
        )])?;
        self.virtual_pen.emit(&[InputEvent::new(
            EventType::SYNCHRONIZATION,
            Synchronization::SYN_REPORT.0,
            0,
        )])?;
        Ok(())
    }

    pub fn dispatch(&mut self, raw_data: &RawDataReader) {
        self.emit_pen_events(raw_data);
        self.emit_tablet_events(raw_data);
    }

    fn emit_tablet_events(&mut self, raw_data: &RawDataReader) {
        let raw_button_as_binary_flags = raw_data.tablet_buttons_as_binary_flags();
        self.binary_flags_to_tablet_key_events(raw_button_as_binary_flags);
        self.tablet_last_raw_pressed_buttons = raw_button_as_binary_flags;
    }

    fn virtual_keyboard_builder(tablet_emitted_keys: &[Key]) -> Result<VirtualDevice, Error> {
        let mut key_set = AttributeSet::<Key>::new();
        for key in tablet_emitted_keys {
            key_set.insert(*key);
        }
        VirtualDeviceBuilder::new()?
            .name("virtual_tablet")
            .with_keys(&key_set)?
            .build()
    }

    fn binary_flags_to_tablet_key_events(&mut self, raw_button_as_flags: u16) {
        (0..14)
            .filter(|i| ![10, 11].contains(i))
            .for_each(|i| self.emit_tablet_key_event(i, raw_button_as_flags));
    }

    pub fn emit_tablet_key_event(&mut self, i: u8, raw_button_as_flags: u16) {
        let id_as_binary_mask = 1 << i;
        let is_pressed = (raw_button_as_flags & id_as_binary_mask) == 0;
        let was_pressed = (self.tablet_last_raw_pressed_buttons & id_as_binary_mask) == 0;
        if let Some(state) = match (was_pressed, is_pressed) {
            (false, true) => Some(Self::PRESSED),
            (true, false) => Some(Self::RELEASED),
            (true, true) => Some(Self::HOLD),
            _ => None,
        } {
            if let Some(keys) = self.tablet_button_id_to_key_code_map.get(&i) {
                for &key in keys {
                    self.virtual_keyboard
                        .emit(&[InputEvent::new(EventType::KEY, key.code(), state)])
                        .expect("Error emitting virtual keyboard key.");
                }
                self.virtual_keyboard
                    .emit(&[InputEvent::new(
                        EventType::SYNCHRONIZATION,
                        Synchronization::SYN_REPORT.0,
                        0,
                    )])
                    .expect("Error emitting SYN.");
            }
        }
    }

    fn virtual_pen_builder(pen_emitted_keys: &[Key]) -> Result<VirtualDevice, Error> {
        let abs_x_setup = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_X,
            AbsInfo::new(0, 0, Self::MAX_X, 0, 0, 1),
        );
        let abs_y_setup = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_Y,
            AbsInfo::new(0, 0, Self::MAX_Y, 0, 0, 1),
        );
        let abs_pressure_setup = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_PRESSURE,
            AbsInfo::new(0, 0, Self::MAX_PRESSURE, 0, 0, 1),
        );
        let mut key_set = AttributeSet::<Key>::new();
        for key in pen_emitted_keys {
            key_set.insert(*key);
        }
        for key in &[Key::BTN_TOOL_PEN, Key::BTN_LEFT, Key::BTN_RIGHT] {
            key_set.insert(*key);
        }
        VirtualDeviceBuilder::new()?
            .name("virtual_tablet")
            .with_absolute_axis(&abs_x_setup)?
            .with_absolute_axis(&abs_y_setup)?
            .with_absolute_axis(&abs_pressure_setup)?
            .with_keys(&key_set)?
            .build()
    }

    fn emit_pen_events(&mut self, raw_data: &RawDataReader) {
        let y_raw = raw_data.y_axis();
        let is_multimedia_area = y_raw < 0;

        let raw_pen_buttons = raw_data.pen_buttons();
        self.raw_pen_buttons_to_pen_key_events(raw_pen_buttons);
        self.pen_last_raw_pressed_button = raw_pen_buttons;
        let normalized_pressure = self.normalize_pressure(raw_data.pressure());
        let (smoothed_x, smoothed_y) = self.smooth_coordinates(raw_data.x_axis(), y_raw);

        self.raw_pen_abs_to_pen_abs_events(
            smoothed_x,
            smoothed_y,
            normalized_pressure,
            is_multimedia_area,
        );
        self.pen_emit_touch(smoothed_x, is_multimedia_area, normalized_pressure);
    }

    fn normalize_pressure(&self, raw_pressure: i32) -> i32 {
        let val = Self::RAW_PRESSURE_POINTS - raw_pressure;

        let config = self.config.read().unwrap();

        if val <= config.pressure_threshold as i32 {
            0
        } else {
            (val as f32 * config.sensitivity) as i32
        }
    }

    fn raw_pen_abs_to_pen_abs_events(
        &mut self,
        x: i32,
        y: i32,
        pressure: i32,
        is_multimedia_area: bool,
    ) {
        if is_multimedia_area {
            return;
        }

        let events = [
            InputEvent::new(
                EventType::ABSOLUTE,
                AbsoluteAxisType::ABS_X.0,
                x.clamp(0, Self::MAX_X)
            ),
            InputEvent::new(
                EventType::ABSOLUTE,
                AbsoluteAxisType::ABS_Y.0,
                y.clamp(0, Self::MAX_Y),
            ),
            InputEvent::new(
                EventType::ABSOLUTE,
                AbsoluteAxisType::ABS_PRESSURE.0,
                pressure
            ),
            InputEvent::new(
                EventType::KEY,
                Key::BTN_TOOL_PEN.code(),
                            1
            ),
        ];

        self.virtual_pen
        .emit(&events)
        .expect("Error emitting pen input packet.");
    }

    fn pen_emit_touch(&mut self, x: i32, is_multimedia_area: bool, normalized_pressure: i32) {
        let is_touching = normalized_pressure > 0;
        if let Some(state) = match (self.was_touching, is_touching) {
            (false, true) => Some(Self::PRESSED),
            (true, false) => Some(Self::RELEASED),
            _ => None,
        } {
            if is_multimedia_area {
                if state == Self::PRESSED {
                    self.last_pressed_media_button = (x / self.media_button_width) as u8
                }
                if let Some(keys) = self
                    .media_button_id_to_key_code_map
                    .get(&self.last_pressed_media_button)
                {
                    for key in keys {
                        self.media_keyboard
                            .emit(&[InputEvent::new(EventType::KEY, key.code(), state)])
                            .expect("Error emitting media keys.")
                    }
                }
            } else {
                if let Some(keys) = self
                    .media_button_id_to_key_code_map
                    .get(&self.last_pressed_media_button)
                {
                    for key in keys {
                        self.media_keyboard
                            .emit(&[InputEvent::new(EventType::KEY, key.code(), Self::RELEASED)])
                            .expect("Error emitting media keys.")
                    }
                }
                self.virtual_pen
                    .emit(&[InputEvent::new(
                        EventType::KEY,
                        Key::BTN_TOUCH.code(),
                        state,
                    )])
                    .expect("Error emitting Touch");
            }
        }
        self.was_touching = is_touching;
    }

    fn raw_pen_buttons_to_pen_key_events(&mut self, pen_button: u8) {
        if let Some((state, id)) = match (self.pen_last_raw_pressed_button, pen_button) {
            (2, x) if x == 6 || x == 4 => Some((Self::PRESSED, x)),
            (x, 2) if x == 6 || x == 4 => Some((Self::RELEASED, x)),
            (x, y) if x != 2 && x == y => Some((Self::HOLD, x)),
            _ => None,
        } {
            if let Some(keys) = self.pen_button_id_to_key_code_map.get(&id) {
                for key in keys {
                    self.virtual_pen
                        .emit(&[InputEvent::new(EventType::KEY, key.code(), state)])
                        .expect("Error emitting pen keys.")
                }
            }
        }
    }
}
