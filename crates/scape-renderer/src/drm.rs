use std::borrow::Cow;
use std::num::NonZeroU32;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use anyhow::Context;
use drm::control::{
    self, atomic, connector, crtc, property, AtomicCommitFlags, Device as ControlDevice,
};
use drm::Device;
use gbm::Format as DrmFourcc;
use raw_window_handle::{DisplayHandle, DrmDisplayHandle, DrmWindowHandle, WindowHandle};
use scape_shared::MainMessage;
use tracing::info;
use wgpu::{InstanceDescriptor, SurfaceTarget, SurfaceTargetUnsafe};

use crate::{Gpu, RendererState};

impl Device for Gpu {}
impl ControlDevice for Gpu {}

impl RendererState {
    pub(crate) fn test_drm(&mut self) {
        let gpu = &self.gpus.values().next().unwrap();

        gpu.set_client_capability(drm::ClientCapability::UniversalPlanes, true)
            .expect("Unable to request UniversalPlanes capability");
        gpu.set_client_capability(drm::ClientCapability::Atomic, true)
            .expect("Unable to request Atomic capability");

        // Load the information.
        let res = gpu
            .resource_handles()
            .expect("Could not load normal resource ids.");
        let coninfo: Vec<connector::Info> = res
            .connectors()
            .iter()
            .flat_map(|con| gpu.get_connector(*con, true))
            .collect();
        let crtcinfo: Vec<crtc::Info> = res
            .crtcs()
            .iter()
            .flat_map(|crtc| gpu.get_crtc(*crtc))
            .collect();

        for crtc in &crtcinfo {
            info!("CRTC: {:?}", crtc);
        }

        // Filter each connector until we find one that's connected.
        let con = coninfo
            .iter()
            .find(|&i| i.state() == connector::State::Connected)
            .expect("No connected connectors");

        let &mode = con.modes().first().expect("No modes found on connector");

        let (disp_width, disp_height) = mode.size();

        // Find a crtc and FB
        let crtc = crtcinfo.first().expect("No crtcs found");

        let fmt = DrmFourcc::Xrgb8888;

        let mut db = gpu
            .create_dumb_buffer((disp_width.into(), disp_height.into()), fmt, 32)
            .expect("Could not create dumb buffer");

        {
            let mut map = gpu
                .map_dumb_buffer(&mut db)
                .expect("Could not map dumbbuffer");
            for b in map.as_mut() {
                *b = 128;
            }
        }

        let fb = gpu
            .add_framebuffer(&db, 24, 32)
            .expect("Could not create FB");

        let planes = gpu.plane_handles().expect("Could not list planes");
        let (better_planes, compatible_planes): (
            Vec<control::plane::Handle>,
            Vec<control::plane::Handle>,
        ) = planes
            .iter()
            .filter(|&&plane| {
                gpu.get_plane(plane)
                    .map(|plane_info| {
                        let compatible_crtcs = res.filter_crtcs(plane_info.possible_crtcs());
                        compatible_crtcs.contains(&crtc.handle())
                    })
                    .unwrap_or(false)
            })
            .partition(|&&plane| {
                if let Ok(props) = gpu.get_properties(plane) {
                    for (&id, &val) in props.iter() {
                        if let Ok(info) = gpu.get_property(id) {
                            if info.name().to_str().map(|x| x == "type").unwrap_or(false) {
                                return val == (drm::control::PlaneType::Primary as u32).into();
                            }
                        }
                    }
                }
                false
            });
        let plane = *better_planes.first().unwrap_or(&compatible_planes[0]);

        tracing::error!("{:#?}", mode);
        tracing::error!("{:#?}", fb);
        tracing::error!("{:#?}", db);
        tracing::error!("{:#?}", plane);

        let card = gpu;

        let con_props = card
            .get_properties(con.handle())
            .expect("Could not get props of connector")
            .as_hashmap(*card)
            .expect("Could not get a prop from connector");
        let crtc_props = card
            .get_properties(crtc.handle())
            .expect("Could not get props of crtc")
            .as_hashmap(*card)
            .expect("Could not get a prop from crtc");
        let plane_props = card
            .get_properties(plane)
            .expect("Could not get props of plane")
            .as_hashmap(*card)
            .expect("Could not get a prop from plane");

        tracing::error!("props: {:#?}", con_props.keys());
        tracing::error!("crtc: {:#?}", crtc_props.keys());
        tracing::error!("plane: {:#?}", plane_props.keys());
        let mut atomic_req = atomic::AtomicModeReq::new();
        atomic_req.add_property(
            con.handle(),
            con_props["CRTC_ID"].handle(),
            property::Value::CRTC(Some(crtc.handle())),
        );
        let blob = card
            .create_property_blob(&mode)
            .expect("Failed to create blob");
        atomic_req.add_property(crtc.handle(), crtc_props["MODE_ID"].handle(), blob);
        atomic_req.add_property(
            crtc.handle(),
            crtc_props["ACTIVE"].handle(),
            property::Value::Boolean(true),
        );
        atomic_req.add_property(
            plane,
            plane_props["FB_ID"].handle(),
            property::Value::Framebuffer(Some(fb)),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_ID"].handle(),
            property::Value::CRTC(Some(crtc.handle())),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_X"].handle(),
            property::Value::UnsignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_Y"].handle(),
            property::Value::UnsignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_W"].handle(),
            property::Value::UnsignedRange((mode.size().0 as u64) << 16),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_H"].handle(),
            property::Value::UnsignedRange((mode.size().1 as u64) << 16),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_X"].handle(),
            property::Value::SignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_Y"].handle(),
            property::Value::SignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_W"].handle(),
            property::Value::UnsignedRange(mode.size().0 as u64),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_H"].handle(),
            property::Value::UnsignedRange(mode.size().1 as u64),
        );

        // Set the crtc
        // On many setups, this requires root access.
        card.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, atomic_req)
            .expect("Failed to set mode");

        let five_seconds = ::std::time::Duration::from_millis(5000);
        ::std::thread::sleep(five_seconds);

        self.comms.main(MainMessage::Shutdown);
    }
}

pub(crate) async fn test_wgpu(gpu: Gpu) -> anyhow::Result<()> {
    gpu.set_client_capability(drm::ClientCapability::UniversalPlanes, true)
        .expect("Unable to request UniversalPlanes capability");
    gpu.set_client_capability(drm::ClientCapability::Atomic, true)
        .expect("Unable to request Atomic capability");

    // Load the information.
    let res = gpu
        .resource_handles()
        .expect("Could not load normal resource ids.");
    let coninfo: Vec<connector::Info> = res
        .connectors()
        .iter()
        .flat_map(|con| gpu.get_connector(*con, true))
        .collect();
    let crtcinfo: Vec<crtc::Info> = res
        .crtcs()
        .iter()
        .flat_map(|crtc| gpu.get_crtc(*crtc))
        .collect();

    for crtc in &crtcinfo {
        info!("CRTC: {:?}", crtc);
    }
    //
    // Filter each connector until we find one that's connected.
    let con = coninfo
        .iter()
        .find(|&i| i.state() == connector::State::Connected)
        .expect("No connected connectors");

    let &mode = con.modes().first().expect("No modes found on connector");

    let crtc = crtcinfo.first().expect("No crtcs found");

    let planes = gpu.plane_handles().expect("Could not list planes");
    let (better_planes, compatible_planes): (
        Vec<control::plane::Handle>,
        Vec<control::plane::Handle>,
    ) = planes
        .iter()
        .filter(|&&plane| {
            gpu.get_plane(plane)
                .map(|plane_info| {
                    let compatible_crtcs = res.filter_crtcs(plane_info.possible_crtcs());
                    compatible_crtcs.contains(&crtc.handle())
                })
                .unwrap_or(false)
        })
        .partition(|&&plane| {
            if let Ok(props) = gpu.get_properties(plane) {
                for (&id, &val) in props.iter() {
                    if let Ok(info) = gpu.get_property(id) {
                        if info.name().to_str().map(|x| x == "type").unwrap_or(false) {
                            return val == (drm::control::PlaneType::Primary as u32).into();
                        }
                    }
                }
            }
            false
        });
    let plane = *better_planes.first().unwrap_or(&compatible_planes[0]);

    let (disp_width, disp_height) = mode.size();
    let display_handle = unsafe {
        DisplayHandle::borrow_raw({
            let handle = DrmDisplayHandle::new(gpu.fd.as_raw_fd());
            handle.into()
        })
    };

    let window_handle = unsafe {
        WindowHandle::borrow_raw({
            let handle = DrmWindowHandle::new_with_connector_id(
                plane.into(),
                NonZeroU32::new(con.interface_id()).unwrap(),
            );
            handle.into()
        })
    };

    let instance = wgpu::Instance::new(InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });

    let surface_target = SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: display_handle.as_raw(),
        raw_window_handle: window_handle.as_raw(),
    };

    let surface = unsafe {
        instance
            .create_surface_unsafe(surface_target)
            .expect("Failed to create surface")
    };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .context("Failed to find an appropriate adapter")?;

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        )
        .await
        .context("Failed to create device")?;

    // Load the shaders from disk
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities.formats[0];

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let config = surface
        .get_default_config(&adapter, disp_width as u32, disp_height as u32)
        .expect("Surface not supported by adapter");

    surface.configure(&device, &config);

    let start = Instant::now();

    while Instant::now().duration_since(start) < Duration::from_secs(5) {
        let frame = surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&render_pipeline);
            rpass.draw(0..3, 0..1);
        }

        queue.submit(Some(encoder.finish()));
        frame.present();
    }

    Ok(())
}
