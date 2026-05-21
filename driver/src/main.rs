mod virtual_device;
mod physical_device;
mod config;
mod gui;

use clap::Parser;
use signal_hook::consts::signal::*;
use signal_hook::flag::register;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use physical_device::PhysicalDevice;
use virtual_device::{DeviceDispatcher, RawDataReader};
use config::AppConfig;
use std::fs;

const VID: u16 = 0x08f2;
const PID: u16 = 0x6811;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config: bool,
}

fn main() {
    let args = Args::parse();

    if args.config {
        println!("Running config tool...");
        if let Err(e) = gui::run_gui() {
            eprintln!("GUI Error: {}", e);
        }
        return;
    }

    let initial_config = AppConfig::load();
    println!("Loaded config: Threshold={}, Sensitivity={}",
             initial_config.pressure_threshold, initial_config.sensitivity);

    let config = Arc::new(RwLock::new(initial_config));

    let mut data_reader = RawDataReader::new();
    let mut device_dispatcher = DeviceDispatcher::new(config.clone());

    let mut physical_device: Option<PhysicalDevice> = None;

    println!("Driver started. Waiting for device...");

    let shutdown = Arc::new(AtomicBool::new(false));

    let signals: Vec<i32> = vec![SIGINT, SIGTERM, SIGQUIT];
    for signal in signals {
        register(signal, Arc::clone(&shutdown)).expect("Error registering interrupt signals.");
    }

    // Config file monitor — stops when the main shutdown flag is set.
    let config_monitor = config.clone();
    let shutdown_monitor = Arc::clone(&shutdown);
    thread::spawn(move || {
        let path = AppConfig::get_config_path();
        let mut last_mtime = fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok();

        while !shutdown_monitor.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(1000));

            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(mtime) = metadata.modified() {
                    let changed = match last_mtime {
                        None => true,
                        Some(last) => mtime > last,
                    };

                    if changed {
                        println!("Config file changed, reloading...");
                        let new_config = AppConfig::load();
                        println!("New config: Threshold={}, Sensitivity={}",
                                 new_config.pressure_threshold, new_config.sensitivity);

                        if let Ok(mut w) = config_monitor.write() {
                            *w = new_config;
                        }
                        last_mtime = Some(mtime);
                    }
                }
            }
        }
    });

    while !shutdown.load(Ordering::Relaxed) {
        if let Some(device) = &mut physical_device {
            match device.read_device_responses(&mut data_reader.data) {
                Ok(len) if len > 0 => {
                    device_dispatcher.dispatch(&data_reader);
                    if device_dispatcher.syn().is_err() {
                        eprintln!("Error emitting SYN.");
                    }
                }
                Ok(_) => {}
                Err(rusb::Error::Timeout) => {}
                Err(e) => {
                    eprintln!("Device error/disconnected: {}", e);
                    physical_device = None;
                }
            }
        } else {
            match PhysicalDevice::new(VID, PID) {
                Ok(mut dev) => {
                    println!("Device connected!");
                    dev.init().set_full_mode();
                    physical_device = Some(dev);
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
    }

    println!();
    println!("The driver has exited.");
}
