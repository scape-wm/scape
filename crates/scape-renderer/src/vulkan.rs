use std::{ffi::CString, ptr};

use ash::{vk, Entry};

pub(crate) fn init_vulkan() {
    // TODO: Maybe use linked entry
    let entry = unsafe { Entry::load().expect("Failed to load entry!") };
    let instance = create_instance(&entry);
}

fn create_instance(entry: &ash::Entry) -> ash::Instance {
    let app_info = vk::ApplicationInfo {
        api_version: vk::make_api_version(0, 1, 0, 0),
        ..Default::default()
    };
    let create_info = vk::InstanceCreateInfo {
        p_application_info: &app_info,
        ..Default::default()
    };

    // TODO: destroy instance
    let instance: ash::Instance = unsafe {
        entry
            .create_instance(&create_info, None)
            .expect("Failed to create instance!")
    };

    instance
}
