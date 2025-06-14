#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::ffi::CString;
use std::os::unix::io::RawFd;
use std::ptr;

mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use bindings::*;

/// A safe wrapper around libinput context
pub struct Libinput {
    ptr: *mut libinput,
}

impl Libinput {
    /// Create a new libinput context using udev
    pub fn new(udev: *mut udev) -> Option<Self> {
        let ptr = unsafe {
            libinput_udev_create_context(
                ptr::null_mut(), // interface
                ptr::null_mut(), // userdata
                udev,            // udev context
            )
        };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Create a new libinput context using udev with a custom interface
    pub fn new_with_interface(
        interface: *const libinput_interface,
        udev: *mut udev,
    ) -> Option<Self> {
        let ptr = unsafe {
            libinput_udev_create_context(
                interface,
                ptr::null_mut(), // userdata
                udev,            // udev context
            )
        };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Create a new libinput context using udev with a custom interface and userdata
    pub fn new_with_interface_and_userdata(
        interface: *const libinput_interface,
        userdata: *mut ::std::os::raw::c_void,
        udev: *mut udev,
    ) -> Option<Self> {
        let ptr = unsafe {
            libinput_udev_create_context(
                interface, userdata, udev, // udev context
            )
        };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Assign a seat to the context
    pub fn assign_seat(&mut self, seat: &str) -> Result<(), ()> {
        let seat = CString::new(seat).map_err(|_| ())?;
        let ret = unsafe { libinput_udev_assign_seat(self.ptr, seat.as_ptr()) };
        if ret < 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Add a device to the context
    pub fn add_device(&mut self, path: &str) -> Result<(), ()> {
        let path = CString::new(path).map_err(|_| ())?;
        let ret = unsafe { libinput_path_add_device(self.ptr, path.as_ptr()) };
        if ret.is_null() {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Get the next event from the context
    pub fn next_event(&mut self) -> Option<Event> {
        let event = unsafe { libinput_get_event(self.ptr) };
        if event.is_null() {
            None
        } else {
            Some(Event { ptr: event })
        }
    }

    /// Get the file descriptor for the context
    pub fn get_fd(&self) -> RawFd {
        unsafe { libinput_get_fd(self.ptr) }
    }

    /// Dispatch events
    pub fn dispatch(&mut self) -> i32 {
        unsafe { libinput_dispatch(self.ptr) }
    }
}

impl Drop for Libinput {
    fn drop(&mut self) {
        unsafe {
            libinput_unref(self.ptr);
        }
    }
}

/// A safe wrapper around libinput events
pub struct Event {
    ptr: *mut libinput_event,
}

impl Event {
    /// Get the type of the event
    pub fn get_type(&self) -> libinput_event_type {
        unsafe { libinput_event_get_type(self.ptr) }
    }

    /// Get the device that generated this event
    pub fn get_device(&self) -> Option<Device> {
        let device = unsafe { libinput_event_get_device(self.ptr) };
        if device.is_null() {
            None
        } else {
            Some(Device { ptr: device })
        }
    }

    /// Get the keyboard event if this is a keyboard event
    pub fn get_keyboard_event(&self) -> Option<KeyboardEvent> {
        if self.get_type() == libinput_event_type_LIBINPUT_EVENT_KEYBOARD_KEY {
            let ptr = unsafe { libinput_event_get_keyboard_event(self.ptr) };
            if ptr.is_null() {
                None
            } else {
                Some(KeyboardEvent { 
                    event: self,
                    ptr 
                })
            }
        } else {
            None
        }
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            libinput_event_destroy(self.ptr);
        }
    }
}

/// A safe wrapper around libinput device
pub struct Device {
    ptr: *mut libinput_device,
}

impl Device {
    /// Get the device name
    pub fn get_name(&self) -> Option<String> {
        let name = unsafe { libinput_device_get_name(self.ptr) };
        if name.is_null() {
            None
        } else {
            unsafe {
                let c_str = std::ffi::CStr::from_ptr(name);
                Some(c_str.to_string_lossy().into_owned())
            }
        }
    }

    /// Get the device output name
    pub fn get_output_name(&self) -> Option<String> {
        let name = unsafe { libinput_device_get_output_name(self.ptr) };
        if name.is_null() {
            None
        } else {
            unsafe {
                let c_str = std::ffi::CStr::from_ptr(name);
                Some(c_str.to_string_lossy().into_owned())
            }
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            libinput_device_unref(self.ptr);
        }
    }
}

/// A safe wrapper around libinput keyboard events
pub struct KeyboardEvent<'a> {
    event: &'a Event,
    ptr: *mut libinput_event_keyboard,
}

impl<'a> KeyboardEvent<'a> {
    /// Get the key code of the pressed/released key
    pub fn get_key(&self) -> u32 {
        unsafe { libinput_event_keyboard_get_key(self.ptr) }
    }

    /// Get the key state (pressed or released)
    pub fn get_key_state(&self) -> libinput_key_state {
        unsafe { libinput_event_keyboard_get_key_state(self.ptr) }
    }

    /// Get the time of the event in microseconds
    pub fn get_time_usec(&self) -> u64 {
        unsafe { libinput_event_keyboard_get_time_usec(self.ptr) }
    }

    /// Get the seat key count
    pub fn get_seat_key_count(&self) -> u32 {
        unsafe { libinput_event_keyboard_get_seat_key_count(self.ptr) }
    }
}
