use std::{
    borrow::Cow,
    ffi::{self, c_char, CStr, CString},
    os::fd::AsRawFd,
};

use anyhow::Context;
use ash::{
    ext, khr,
    vk::{self, DisplaySurfaceCreateInfoKHR},
    Device, Entry, Instance,
};
use tracing::{info, warn};

use crate::Gpu;

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    warn!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

pub(crate) struct VulkanState {
    pub entry: Entry,
    pub instance: Instance,
    pub device: Device,
    pub surface_loader: khr::surface::Instance,
    pub swapchain_loader: khr::swapchain::Device,
    pub debug_utils_loader: ext::debug_utils::Instance,
    pub debug_call_back: vk::DebugUtilsMessengerEXT,

    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub present_images: Vec<vk::Image>,
    pub present_image_views: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub draw_command_buffer: vk::CommandBuffer,
    pub setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,

    pub draw_commands_reuse_fence: vk::Fence,
    pub setup_commands_reuse_fence: vk::Fence,
}

impl VulkanState {
    pub fn new(gpu: Gpu) -> anyhow::Result<Self> {
        unsafe {
            let entry = Entry::linked();
            let support_instance_extensions =
                entry.enumerate_instance_extension_properties(None)?;

            let required_extensions = [
                ext::debug_utils::NAME.as_ptr(),
                ext::acquire_drm_display::NAME.as_ptr(),
                ext::direct_mode_display::NAME.as_ptr(),
                khr::display::NAME.as_ptr(),
                khr::surface::NAME.as_ptr(),
            ];

            'outer: for required_extension in required_extensions.iter() {
                for supported_extension in support_instance_extensions.iter() {
                    if CStr::from_ptr(*required_extension)
                        == CStr::from_ptr(supported_extension.extension_name.as_ptr())
                    {
                        continue 'outer;
                    }
                }
                anyhow::bail!("Required extension not supported: {required_extension:?}");
            }
            info!("All required vulkan extensions are supported");

            let app_name = c"scape";

            // TODO: Find out why layer does not exist on instance creation
            // let layer_names = entry
            //     .enumerate_instance_layer_properties()?
            //     .into_iter()
            //     .filter_map(|layer| {
            //         if CStr::from_ptr(layer.layer_name.as_ptr()) == c"VK_LAYER_KHRONOS_validation" {
            //             Some(layer.layer_name.as_ptr())
            //         } else {
            //             None
            //         }
            //     })
            //     .collect::<Vec<_>>();
            let layer_names = [];

            let appinfo = vk::ApplicationInfo::default()
                .application_name(app_name)
                .application_version(1)
                .engine_name(app_name)
                .engine_version(1)
                .api_version(vk::make_api_version(0, 1, 0, 0));

            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&appinfo)
                .enabled_layer_names(&layer_names)
                .enabled_extension_names(&required_extensions);

            let instance: Instance = entry
                .create_instance(&create_info, None)
                .context("Instance creation error")?;

            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_utils_loader = ext::debug_utils::Instance::new(&entry, &instance);
            let debug_callback = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .context("Failed to create debug callback")?;

            let physical_devices = instance
                .enumerate_physical_devices()
                .context("Unable to get physical devices")?;
            for physical_device in physical_devices.iter() {
                let properties = instance.get_physical_device_properties(*physical_device);
                let name = properties.device_name;
                let t = properties.device_type;
                info!("Physical device: {:?}", CStr::from_ptr(name.as_ptr()));
                info!("Physical device: {t:?}");
            }
            let physical_device = physical_devices
                .first()
                .context("No physical devices found")?;

            let display_loader = khr::display::Instance::new(&entry, &instance);
            let display_properties = display_loader
                .get_physical_device_display_properties(*physical_device)
                .context("Unable to get display properties")?;
            info!("starting now, len: {}", display_properties.len());
            for display_property in &display_properties {
                info!("Display property: {:?}", display_property);
            }

            let acquire_drm_display_loader =
                ext::acquire_drm_display::Instance::new(&entry, &instance);
            // acquire_drm_display_loader
            //     .acquire_drm_display(*physical_device, gpu.fd.as_raw_fd(), display)
            //     .context("Unable to acquire drm display")?;

            // let surface_create_info = DisplaySurfaceCreateInfoKHR;
            //
            // let surface = ash_window::create_surface(
            //     &entry,
            //     &instance,
            //     window.display_handle()?.as_raw(),
            //     window.window_handle()?.as_raw(),
            //     None,
            // )
            // .unwrap();
            // let pdevices = instance
            //     .enumerate_physical_devices()
            //     .expect("Physical device error");
            // let surface_loader = surface::Instance::new(&entry, &instance);
            // let (pdevice, queue_family_index) = pdevices
            //     .iter()
            //     .find_map(|pdevice| {
            //         instance
            //             .get_physical_device_queue_family_properties(*pdevice)
            //             .iter()
            //             .enumerate()
            //             .find_map(|(index, info)| {
            //                 let supports_graphic_and_surface =
            //                     info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            //                         && surface_loader
            //                             .get_physical_device_surface_support(
            //                                 *pdevice,
            //                                 index as u32,
            //                                 surface,
            //                             )
            //                             .unwrap();
            //                 if supports_graphic_and_surface {
            //                     Some((*pdevice, index))
            //                 } else {
            //                     None
            //                 }
            //             })
            //     })
            //     .expect("Couldn't find suitable device.");
            // let queue_family_index = queue_family_index as u32;
            // let device_extension_names_raw = [
            //     swapchain::NAME.as_ptr(),
            //     #[cfg(any(target_os = "macos", target_os = "ios"))]
            //     ash::khr::portability_subset::NAME.as_ptr(),
            // ];
            // let features = vk::PhysicalDeviceFeatures {
            //     shader_clip_distance: 1,
            //     ..Default::default()
            // };
            // let priorities = [1.0];
            //
            // let queue_info = vk::DeviceQueueCreateInfo::default()
            //     .queue_family_index(queue_family_index)
            //     .queue_priorities(&priorities);
            //
            // let device_create_info = vk::DeviceCreateInfo::default()
            //     .queue_create_infos(std::slice::from_ref(&queue_info))
            //     .enabled_extension_names(&device_extension_names_raw)
            //     .enabled_features(&features);
            //
            // let device: Device = instance
            //     .create_device(pdevice, &device_create_info, None)
            //     .unwrap();
            //
            // let present_queue = device.get_device_queue(queue_family_index, 0);
            //
            // let surface_format = surface_loader
            //     .get_physical_device_surface_formats(pdevice, surface)
            //     .unwrap()[0];
            //
            // let surface_capabilities = surface_loader
            //     .get_physical_device_surface_capabilities(pdevice, surface)
            //     .unwrap();
            // let mut desired_image_count = surface_capabilities.min_image_count + 1;
            // if surface_capabilities.max_image_count > 0
            //     && desired_image_count > surface_capabilities.max_image_count
            // {
            //     desired_image_count = surface_capabilities.max_image_count;
            // }
            // let surface_resolution = match surface_capabilities.current_extent.width {
            //     u32::MAX => vk::Extent2D {
            //         width: window_width,
            //         height: window_height,
            //     },
            //     _ => surface_capabilities.current_extent,
            // };
            // let pre_transform = if surface_capabilities
            //     .supported_transforms
            //     .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            // {
            //     vk::SurfaceTransformFlagsKHR::IDENTITY
            // } else {
            //     surface_capabilities.current_transform
            // };
            // let present_modes = surface_loader
            //     .get_physical_device_surface_present_modes(pdevice, surface)
            //     .unwrap();
            // let present_mode = present_modes
            //     .iter()
            //     .cloned()
            //     .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
            //     .unwrap_or(vk::PresentModeKHR::FIFO);
            // let swapchain_loader = swapchain::Device::new(&instance, &device);
            //
            // let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            //     .surface(surface)
            //     .min_image_count(desired_image_count)
            //     .image_color_space(surface_format.color_space)
            //     .image_format(surface_format.format)
            //     .image_extent(surface_resolution)
            //     .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            //     .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            //     .pre_transform(pre_transform)
            //     .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            //     .present_mode(present_mode)
            //     .clipped(true)
            //     .image_array_layers(1);
            //
            // let swapchain = swapchain_loader
            //     .create_swapchain(&swapchain_create_info, None)
            //     .unwrap();
            //
            // let pool_create_info = vk::CommandPoolCreateInfo::default()
            //     .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            //     .queue_family_index(queue_family_index);
            //
            // let pool = device.create_command_pool(&pool_create_info, None).unwrap();
            //
            // let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            //     .command_buffer_count(2)
            //     .command_pool(pool)
            //     .level(vk::CommandBufferLevel::PRIMARY);
            //
            // let command_buffers = device
            //     .allocate_command_buffers(&command_buffer_allocate_info)
            //     .unwrap();
            // let setup_command_buffer = command_buffers[0];
            // let draw_command_buffer = command_buffers[1];
            //
            // let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();
            // let present_image_views: Vec<vk::ImageView> = present_images
            //     .iter()
            //     .map(|&image| {
            //         let create_view_info = vk::ImageViewCreateInfo::default()
            //             .view_type(vk::ImageViewType::TYPE_2D)
            //             .format(surface_format.format)
            //             .components(vk::ComponentMapping {
            //                 r: vk::ComponentSwizzle::R,
            //                 g: vk::ComponentSwizzle::G,
            //                 b: vk::ComponentSwizzle::B,
            //                 a: vk::ComponentSwizzle::A,
            //             })
            //             .subresource_range(vk::ImageSubresourceRange {
            //                 aspect_mask: vk::ImageAspectFlags::COLOR,
            //                 base_mip_level: 0,
            //                 level_count: 1,
            //                 base_array_layer: 0,
            //                 layer_count: 1,
            //             })
            //             .image(image);
            //         device.create_image_view(&create_view_info, None).unwrap()
            //     })
            //     .collect();
            // let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            // let depth_image_create_info = vk::ImageCreateInfo::default()
            //     .image_type(vk::ImageType::TYPE_2D)
            //     .format(vk::Format::D16_UNORM)
            //     .extent(surface_resolution.into())
            //     .mip_levels(1)
            //     .array_layers(1)
            //     .samples(vk::SampleCountFlags::TYPE_1)
            //     .tiling(vk::ImageTiling::OPTIMAL)
            //     .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            //     .sharing_mode(vk::SharingMode::EXCLUSIVE);
            //
            // let depth_image = device.create_image(&depth_image_create_info, None).unwrap();
            // let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            // let depth_image_memory_index = find_memorytype_index(
            //     &depth_image_memory_req,
            //     &device_memory_properties,
            //     vk::MemoryPropertyFlags::DEVICE_LOCAL,
            // )
            // .expect("Unable to find suitable memory index for depth image.");
            //
            // let depth_image_allocate_info = vk::MemoryAllocateInfo::default()
            //     .allocation_size(depth_image_memory_req.size)
            //     .memory_type_index(depth_image_memory_index);
            //
            // let depth_image_memory = device
            //     .allocate_memory(&depth_image_allocate_info, None)
            //     .unwrap();
            //
            // device
            //     .bind_image_memory(depth_image, depth_image_memory, 0)
            //     .expect("Unable to bind depth image memory");
            //
            // let fence_create_info =
            //     vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            //
            // let draw_commands_reuse_fence = device
            //     .create_fence(&fence_create_info, None)
            //     .expect("Create fence failed.");
            // let setup_commands_reuse_fence = device
            //     .create_fence(&fence_create_info, None)
            //     .expect("Create fence failed.");
            //
            // record_submit_commandbuffer(
            //     &device,
            //     setup_command_buffer,
            //     setup_commands_reuse_fence,
            //     present_queue,
            //     &[],
            //     &[],
            //     &[],
            //     |device, setup_command_buffer| {
            //         let layout_transition_barriers = vk::ImageMemoryBarrier::default()
            //             .image(depth_image)
            //             .dst_access_mask(
            //                 vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
            //                     | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            //             )
            //             .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
            //             .old_layout(vk::ImageLayout::UNDEFINED)
            //             .subresource_range(
            //                 vk::ImageSubresourceRange::default()
            //                     .aspect_mask(vk::ImageAspectFlags::DEPTH)
            //                     .layer_count(1)
            //                     .level_count(1),
            //             );
            //
            //         device.cmd_pipeline_barrier(
            //             setup_command_buffer,
            //             vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            //             vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            //             vk::DependencyFlags::empty(),
            //             &[],
            //             &[],
            //             &[layout_transition_barriers],
            //         );
            //     },
            // );
            //
            // let depth_image_view_info = vk::ImageViewCreateInfo::default()
            //     .subresource_range(
            //         vk::ImageSubresourceRange::default()
            //             .aspect_mask(vk::ImageAspectFlags::DEPTH)
            //             .level_count(1)
            //             .layer_count(1),
            //     )
            //     .image(depth_image)
            //     .format(depth_image_create_info.format)
            //     .view_type(vk::ImageViewType::TYPE_2D);
            //
            // let depth_image_view = device
            //     .create_image_view(&depth_image_view_info, None)
            //     .unwrap();
            //
            // let semaphore_create_info = vk::SemaphoreCreateInfo::default();
            //
            // let present_complete_semaphore = device
            //     .create_semaphore(&semaphore_create_info, None)
            //     .unwrap();
            // let rendering_complete_semaphore = device
            //     .create_semaphore(&semaphore_create_info, None)
            //     .unwrap();
            //
            // Ok(Self {
            //     entry,
            //     instance,
            //     device,
            //     queue_family_index,
            //     pdevice,
            //     device_memory_properties,
            //     surface_loader,
            //     surface_format,
            //     present_queue,
            //     surface_resolution,
            //     swapchain_loader,
            //     swapchain,
            //     present_images,
            //     present_image_views,
            //     pool,
            //     draw_command_buffer,
            //     setup_command_buffer,
            //     depth_image,
            //     depth_image_view,
            //     present_complete_semaphore,
            //     rendering_complete_semaphore,
            //     draw_commands_reuse_fence,
            //     setup_commands_reuse_fence,
            //     surface,
            //     debug_call_back,
            //     debug_utils_loader,
            //     depth_image_memory,
            // })
            unimplemented!()
        }
    }
}

impl Drop for VulkanState {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device
                .destroy_fence(self.draw_commands_reuse_fence, None);
            self.device
                .destroy_fence(self.setup_commands_reuse_fence, None);
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);
            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.device.destroy_command_pool(self.pool, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_call_back, None);
            self.instance.destroy_instance(None);
        }
    }
}
