use rusb::{
    devices, ConfigDescriptor, Device, DeviceHandle, Error as RusbError, GlobalContext,
    InterfaceDescriptor, TransferType,
};

use std::time::Duration;

pub struct PhysicalDevice {
    device: Device<GlobalContext>,
    device_handle: DeviceHandle<GlobalContext>,
    endpoint_address: Option<u8>,
}

impl PhysicalDevice {
    pub fn new(vid: u16, pid: u16) -> Result<Self, RusbError> {
        let device = Self::get_target_device(vid, pid)?; 
        
        let device_handle = device.open()?; 

        Ok(PhysicalDevice {
            endpoint_address: None,
            device_handle,
            device,
        })
    }

    pub fn init(&mut self) -> &mut Self {
        let _ = self.device_handle.set_auto_detach_kernel_driver(true);

        let configurations = Self::get_configurations(&self.device);
        let interface_descriptors = Self::get_hid_interface_descriptors(&configurations);

        for interface_descriptor in interface_descriptors {
            if self.device_handle
                .claim_interface(interface_descriptor.interface_number())
                .is_ok() 
            {
                for endpoint_descriptor in interface_descriptor.endpoint_descriptors() {
                    if endpoint_descriptor.transfer_type() == TransferType::Interrupt
                        && endpoint_descriptor.max_packet_size() == 64
                    {
                        self.endpoint_address = Some(endpoint_descriptor.address());
                    }
                }
            }
        }
        self.reset();
        self
    }

    pub fn reset(&mut self) {
        let _ = self.device_handle.reset();
    }

    pub fn read_device_responses(&self, buffer: &mut [u8]) -> Result<usize, RusbError> {
        let addr = self.endpoint_address.ok_or(RusbError::NotFound)?;
        self.device_handle
            .read_interrupt(addr, buffer, Duration::from_secs(1))
    }

    pub fn set_full_mode(&mut self) -> &mut Self {
        const REPORTS: [[u8; 8]; 1] = [[0x08, 0x03, 0x00, 0xff, 0xf0, 0x00, 0xff, 0xf0]];
        let reports_as_slices: Vec<&[u8]> = REPORTS.iter().map(|r| &r[..]).collect();
        let _ = self.set_report(&reports_as_slices);
        self
    }

    pub fn set_report(&mut self, reports: &[&[u8]]) -> Result<(), RusbError> {
        for report in reports.iter() {
            self.device_handle.write_control(
                0x21,
                0x9,
                0x0308,
                2,
                report,
                Duration::from_millis(250),
            )?;
        }

        Ok(())
    }

    fn is_target_device(vid: u16, pid: u16, device: &Device<GlobalContext>) -> bool {
        if let Ok(device_descriptor) = device.device_descriptor() {
            return device_descriptor.vendor_id() == vid && device_descriptor.product_id() == pid;
        }
        false
    }

    fn get_target_device(vid: u16, pid: u16) -> Result<Device<GlobalContext>, RusbError> {
        match devices()?
            .iter()
            .find(|device| Self::is_target_device(vid, pid, device))
        {
            Some(device) => Ok(device),
            None => Err(RusbError::NoDevice),
        }
    }
    
    fn get_hid_interface_descriptors(
        config_descriptors: &[ConfigDescriptor],
    ) -> Vec<InterfaceDescriptor<'_>> {
        config_descriptors
            .iter()
            .flat_map(|config_descriptor| config_descriptor.interfaces())
            .flat_map(|interface| interface.descriptors())
            .filter(|interface_descriptor| {
                interface_descriptor.class_code() == rusb::constants::LIBUSB_CLASS_HID
            })
            .collect()
    }

    fn get_configurations(device: &Device<GlobalContext>) -> Vec<ConfigDescriptor> {
        if let Ok(device_descriptor) = device.device_descriptor() {
            (0..device_descriptor.num_configurations())
            .filter_map(|n| device.config_descriptor(n).ok())
            .collect()
        } else {
            vec![]
        }
    }
}