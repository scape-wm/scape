#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::ffi::CString;

mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use bindings::*;

/// A safe wrapper around udev context
pub struct Udev {
    ptr: *mut udev,
}

impl Udev {
    /// Create a new udev context
    pub fn new() -> Option<Self> {
        let ptr = unsafe { udev_new() };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Create a new udev monitor
    pub fn new_monitor(&self) -> Option<Monitor> {
        let ptr = unsafe { udev_monitor_new_from_netlink(self.ptr, c"udev".as_ptr()) };
        if ptr.is_null() {
            None
        } else {
            Some(Monitor { ptr })
        }
    }

    /// Create a new udev enumerate
    pub fn new_enumerate(&self) -> Option<Enumerate> {
        let ptr = unsafe { udev_enumerate_new(self.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(Enumerate { ptr })
        }
    }
}

impl Drop for Udev {
    fn drop(&mut self) {
        unsafe {
            udev_unref(self.ptr);
        }
    }
}

/// A safe wrapper around udev monitor
pub struct Monitor {
    ptr: *mut udev_monitor,
}

impl Monitor {
    /// Enable receiving events
    pub fn enable_receiving(&mut self) -> Result<(), ()> {
        let ret = unsafe { udev_monitor_enable_receiving(self.ptr) };
        if ret < 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Get the file descriptor for the monitor
    pub fn get_fd(&self) -> i32 {
        unsafe { udev_monitor_get_fd(self.ptr) }
    }

    /// Receive a device from the monitor
    pub fn receive_device(&mut self) -> Option<Device> {
        let ptr = unsafe { udev_monitor_receive_device(self.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(Device { ptr })
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe {
            udev_monitor_unref(self.ptr);
        }
    }
}

/// A safe wrapper around udev enumerate
pub struct Enumerate {
    ptr: *mut udev_enumerate,
}

impl Enumerate {
    /// Add a match subsystem
    pub fn add_match_subsystem(&mut self, subsystem: &str) -> Result<(), ()> {
        let subsystem = CString::new(subsystem).map_err(|_| ())?;
        let ret = unsafe { udev_enumerate_add_match_subsystem(self.ptr, subsystem.as_ptr()) };
        if ret < 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Scan devices
    pub fn scan_devices(&mut self) -> Result<(), ()> {
        let ret = unsafe { udev_enumerate_scan_devices(self.ptr) };
        if ret < 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Get the first device in the enumeration
    pub fn get_list_entry(&self) -> Option<ListEntry> {
        let ptr = unsafe { udev_enumerate_get_list_entry(self.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(ListEntry { ptr })
        }
    }
}

impl Drop for Enumerate {
    fn drop(&mut self) {
        unsafe {
            udev_enumerate_unref(self.ptr);
        }
    }
}

/// A safe wrapper around udev device
pub struct Device {
    ptr: *mut udev_device,
}

impl Device {
    /// Get the device path
    pub fn get_devpath(&self) -> Option<String> {
        let path = unsafe { udev_device_get_devpath(self.ptr) };
        if path.is_null() {
            None
        } else {
            unsafe {
                let c_str = std::ffi::CStr::from_ptr(path);
                Some(c_str.to_string_lossy().into_owned())
            }
        }
    }

    /// Get a device property
    pub fn get_property_value(&self, key: &str) -> Option<String> {
        let key = CString::new(key).ok()?;
        let value = unsafe { udev_device_get_property_value(self.ptr, key.as_ptr()) };
        if value.is_null() {
            None
        } else {
            unsafe {
                let c_str = std::ffi::CStr::from_ptr(value);
                Some(c_str.to_string_lossy().into_owned())
            }
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            udev_device_unref(self.ptr);
        }
    }
}

/// A safe wrapper around udev list entry
pub struct ListEntry {
    ptr: *mut udev_list_entry,
}

impl ListEntry {
    /// Get the name of the list entry
    pub fn get_name(&self) -> Option<String> {
        let name = unsafe { udev_list_entry_get_name(self.ptr) };
        if name.is_null() {
            None
        } else {
            unsafe {
                let c_str = std::ffi::CStr::from_ptr(name);
                Some(c_str.to_string_lossy().into_owned())
            }
        }
    }

    /// Get the next entry in the list
    pub fn get_next(&self) -> Option<ListEntry> {
        let ptr = unsafe { udev_list_entry_get_next(self.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(ListEntry { ptr })
        }
    }
}
