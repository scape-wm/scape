use crate::state::{BackendData, SurfaceDmabufFeedback};
use crate::{
    drawing::*,
    render::*,
    shell::WindowElement,
    state::{post_repaint, take_presentation_feedback, State},
};
use anyhow::{anyhow, Result};
#[cfg(feature = "renderer_sync")]
use smithay::backend::drm::compositor::PrimaryPlaneElement;
use smithay::backend::drm::DrmSurface;
use smithay::backend::renderer::element::{RenderElement, RenderElementStates};
use smithay::backend::renderer::sync::SyncPoint;
#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;
#[cfg(feature = "debug")]
use smithay::backend::renderer::ImportMem;
use smithay::backend::renderer::{ExportMem, ImportMemWl, Offscreen};
use smithay::delegate_drm_lease;
use smithay::reexports::drm::control::Device;
use smithay::reexports::drm::control::{connector, ModeTypeFlags};
use smithay::utils::{Physical, Rectangle};
use smithay::wayland::dmabuf::ImportNotifier;
use smithay::wayland::drm_lease::{
    DrmLease, DrmLeaseBuilder, DrmLeaseHandler, DrmLeaseRequest, DrmLeaseState, LeaseRejected,
};
use smithay::{
    backend::{
        allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        allocator::{
            dmabuf::{AnyError, Dmabuf, DmabufAllocator},
            vulkan::{ImageUsageFlags, VulkanAllocator},
            Allocator, Fourcc,
        },
        drm::{
            compositor::DrmCompositor, CreateDrmNodeError, DrmDevice, DrmDeviceFd, DrmError,
            DrmEvent, DrmEventMetadata, DrmNode, GbmBufferedSurface, NodeType,
        },
        egl::{self, EGLDevice, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::{Error as OutputDamageTrackerError, OutputDamageTracker},
            element::{texture::TextureBuffer, AsRenderElements},
            gles::{GlesRenderer, GlesTexture},
            multigpu::{gbm::GbmGlesBackend, GpuManager, MultiRenderer, MultiTexture},
            Bind, DebugFlags, ImportDma, Renderer,
        },
        session::{
            libseat::{self, LibSeatSession},
            Event as SessionEvent, Session,
        },
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
        vulkan::{version::Version, Instance, PhysicalDevice},
        SwapBuffersError,
    },
    desktop::{
        space::{Space, SurfaceTree},
        utils::OutputPresentationFeedback,
    },
    input::pointer::{CursorImageAttributes, CursorImageStatus},
    output::{Mode as WlMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        ash::vk::ExtPhysicalDeviceDrmFn,
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop, LoopHandle, RegistrationToken,
        },
        drm::{control::crtc, Device as _},
        input::Libinput,
        rustix::fs::OFlags,
        wayland_protocols::wp::{
            linux_dmabuf::zv1::server::zwp_linux_dmabuf_feedback_v1,
            presentation_time::server::wp_presentation_feedback,
        },
        wayland_server::{backend::GlobalId, protocol::wl_surface, DisplayHandle},
    },
    utils::{Clock, DeviceFd, IsAlive, Logical, Monotonic, Point, Scale, Transform},
    wayland::{
        compositor,
        dmabuf::{DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufState},
    },
};
use smithay_drm_extras::{
    drm_scanner::{DrmScanEvent, DrmScanner},
    edid::EdidInfo,
};
use std::time::Instant;
use std::{
    collections::{hash_map::HashMap, HashSet},
    convert::TryInto,
    io,
    path::Path,
    sync::Mutex,
    time::Duration,
};
use tracing::{error, info, trace, warn};

// we cannot simply pick the first supported format of the intersection of *all* formats, because:
// - we do not want something like Abgr4444, which looses color information, if something better is available
// - some formats might perform terribly
// - we might need some work-arounds, if one supports modifiers, but the other does not
//
// So lets just pick `ARGB2101010` (10-bit) or `ARGB8888` (8-bit) for now, they are widely supported.
const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];
const SUPPORTED_FORMATS_8BIT_ONLY: &[Fourcc] = &[Fourcc::Abgr8888, Fourcc::Argb8888];

type UdevRenderer<'a, 'b> =
    MultiRenderer<'a, 'a, 'b, GbmGlesBackend<GlesRenderer>, GbmGlesBackend<GlesRenderer>>;

#[derive(Debug, PartialEq)]
struct UdevOutputId {
    device_id: DrmNode,
    crtc: crtc::Handle,
}

pub struct UdevData {
    pub session: LibSeatSession,
    dmabuf_state: Option<(DmabufState, DmabufGlobal)>,
    primary_gpu: DrmNode,
    allocator: Option<Box<dyn Allocator<Buffer = Dmabuf, Error = AnyError>>>,
    gpus: GpuManager<GbmGlesBackend<GlesRenderer>>,
    backends: HashMap<DrmNode, DeviceData>,
    pointer_images: Vec<(xcursor::parser::Image, TextureBuffer<MultiTexture>)>,
    pointer_element: PointerElement<MultiTexture>,
    #[cfg(feature = "debug")]
    fps_texture: Option<MultiTexture>,
    pointer_image: crate::cursor::Cursor,
    debug_flags: DebugFlags,
}

impl std::fmt::Debug for UdevData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdevData")
            .field("session", &self.session)
            .field("dmabuf_state", &self.dmabuf_state)
            .field("primary_gpu", &self.primary_gpu)
            .field("gpus", &self.gpus)
            .field("backends", &self.backends)
            .field("pointer_images", &self.pointer_images)
            .field("pointer_element", &self.pointer_element)
            .field("pointer_image", &self.pointer_image)
            .field("debug_flags", &self.debug_flags)
            .finish()
    }
}

impl UdevData {
    pub fn set_debug_flags(&mut self, flags: DebugFlags) {
        if self.debug_flags != flags {
            self.debug_flags = flags;

            for (_, backend) in self.backends.iter_mut() {
                for (_, surface) in backend.surfaces.iter_mut() {
                    surface.compositor.set_debug_flags(flags);
                }
            }
        }
    }

    pub fn debug_flags(&self) -> DebugFlags {
        self.debug_flags
    }
}

impl UdevData {
    pub fn seat_name(&self) -> String {
        self.session.seat()
    }

    pub fn reset_buffers(&mut self, output: &Output) {
        if let Some(id) = output.user_data().get::<UdevOutputId>() {
            if let Some(gpu) = self.backends.get_mut(&id.device_id) {
                if let Some(surface) = gpu.surfaces.get_mut(&id.crtc) {
                    surface.compositor.reset_buffers();
                }
            }
        }
    }

    pub fn early_import(&mut self, surface: &wl_surface::WlSurface) {
        if let Err(err) = self
            .gpus
            .early_import(Some(self.primary_gpu), self.primary_gpu, surface)
        {
            tracing::warn!("Early buffer import failed: {}", err);
        }
    }

    pub fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state.as_mut().unwrap().0
    }

    pub fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self
            .gpus
            .single_renderer(&self.primary_gpu)
            .and_then(|mut renderer| renderer.import_dmabuf(&dmabuf, None))
            .is_ok()
        {
            let _ = notifier.successful::<State>();
        } else {
            notifier.failed();
        }
    }
}

fn select_primary_gpu(session: &LibSeatSession) -> Result<DrmNode> {
    primary_gpu(session.seat())?
        .and_then(|x| {
            DrmNode::from_path(x)
                .ok()?
                .node_with_type(NodeType::Render)?
                .ok()
        })
        .or_else(|| {
            all_gpus(session.seat())
                .ok()?
                .into_iter()
                .find_map(|x| DrmNode::from_path(x).ok())
        })
        .ok_or(anyhow!("Unable to select primary gpu"))
}

fn select_allocator(
    primary_gpu: DrmNode,
) -> Option<Box<dyn Allocator<Buffer = Dmabuf, Error = AnyError>>> {
    if let Ok(instance) = Instance::new(Version::VERSION_1_2, None) {
        if let Some(physical_device) =
            PhysicalDevice::enumerate(&instance)
                .ok()
                .and_then(|devices| {
                    devices
                        .filter(|phd| phd.has_device_extension(ExtPhysicalDeviceDrmFn::name()))
                        .find(|phd| {
                            phd.primary_node().unwrap() == Some(primary_gpu)
                                || phd.render_node().unwrap() == Some(primary_gpu)
                        })
                })
        {
            match VulkanAllocator::new(
                &physical_device,
                ImageUsageFlags::COLOR_ATTACHMENT | ImageUsageFlags::SAMPLED,
            ) {
                Ok(allocator) => {
                    return Some(Box::new(DmabufAllocator(allocator))
                        as Box<dyn Allocator<Buffer = Dmabuf, Error = AnyError>>);
                }
                Err(err) => {
                    warn!("Failed to create vulkan allocator: {}", err);
                }
            }
        }
    }

    info!("No vulkan allocator found, using GBM.");
    // TODO use backends from caller
    let backends: HashMap<DrmNode, DeviceData> = HashMap::new();
    let gbm = backends
        .get(&primary_gpu)
        // If the primary_gpu failed to initialize, we likely have a kmsro device
        .or_else(|| backends.values().next())
        // Don't fail, if there is no allocator. There is a chance, that this a single gpu system and we don't need one.
        .map(|backend| backend.gbm.clone());
    gbm.map(|gbm| {
        Box::new(DmabufAllocator(GbmAllocator::new(
            gbm,
            GbmBufferFlags::RENDERING,
        ))) as Box<_>
    })
}

pub fn init_udev(event_loop: &mut EventLoop<State>) -> Result<BackendData> {
    let (session, notifier) = LibSeatSession::new().map_err(|e| {
        error!("Could not initialize lib seat session: {}", e);
        e
    })?;

    let primary_gpu = select_primary_gpu(&session).map_err(|e| {
        error!("Could not select primary gpu: {}", e);
        e
    })?;
    info!("Using {} as primary gpu", primary_gpu);

    let gpus = GpuManager::new(GbmGlesBackend::with_context_priority(
        egl::context::ContextPriority::High,
    ))
    .map_err(|e| {
        error!("Could not initialize GpuManager: {}", e);
        e
    })?;

    let allocator = select_allocator(primary_gpu);

    let udev_data = UdevData {
        dmabuf_state: None,
        session: session.clone(),
        primary_gpu,
        gpus,
        allocator,
        backends: HashMap::new(),
        pointer_image: crate::cursor::Cursor::load(),
        pointer_images: Vec::new(),
        pointer_element: PointerElement::default(),
        #[cfg(feature = "debug")]
        fps_texture: None,
        debug_flags: DebugFlags::empty(),
    };

    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    let mut libinput_context_2 = libinput_context.clone();
    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_event, &mut (), state| {
            libinput_context_2
                .udev_assign_seat(&state.seat_name)
                .unwrap();
            TimeoutAction::Drop
        })
        .unwrap();

    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
    event_loop
        .handle()
        .insert_source(libinput_backend, move |event, _, state| {
            state.process_input_event(event)
        })
        .unwrap();
    event_loop
        .handle()
        .insert_source(notifier, move |event, &mut (), state| {
            let BackendData::Udev(udev_data) = &mut state.backend_data else {
                error!("Received non udev backend data");
                return;
            };
            match event {
                SessionEvent::PauseSession => {
                    libinput_context.suspend();
                    info!("pausing session");
                    for backend in udev_data.backends.values_mut() {
                        backend.drm.pause();
                        backend.active_leases.clear();
                        if let Some(lease_global) = backend.leasing_global.as_mut() {
                            lease_global.suspend();
                        }
                    }
                }
                SessionEvent::ActivateSession => {
                    info!("resuming session");
                    if let Err(err) = libinput_context.resume() {
                        error!("Failed to resume libinput context: {:?}", err);
                    }
                    for (node, backend) in udev_data
                        .backends
                        .iter_mut()
                        .map(|(handle, backend)| (*handle, backend))
                    {
                        backend.drm.activate();
                        if let Some(lease_global) = backend.leasing_global.as_mut() {
                            lease_global.resume::<State>();
                        }
                        for surface in backend.surfaces.values_mut() {
                            if let Err(err) = surface.compositor.surface().reset_state() {
                                warn!("Failed to reset drm surface state: {}", err);
                            }
                            // reset the buffers after resume to trigger a full redraw
                            // this is important after a vt switch as the primary plane
                            // has no content and damage tracking may prevent a redraw
                            // otherwise
                            surface.compositor.reset_buffers();
                        }
                        state
                            .loop_handle
                            .insert_idle(move |state| render(state, node, None));
                    }
                }
            }
        })
        .unwrap();

    let udev_backend = UdevBackend::new(session.seat()).map_err(|e| {
        error!("Could not initialize udev backend: {}", e);
        e
    })?;

    for (device_id, path) in udev_backend
        .device_list()
        .map(|(device_id, path)| (device_id, path.to_owned()))
    {
        event_loop
            .handle()
            .insert_source(Timer::immediate(), move |_, _, state| {
                if let Err(err) = DrmNode::from_dev_id(device_id)
                    .map_err(DeviceAddError::DrmNode)
                    .and_then(|node| device_added(state, node, &path))
                {
                    error!("Skipping device {device_id}: {err}");
                }
                TimeoutAction::Drop
            })
            .map_err(|e| {
                error!("Unable to insert timer into loop: {e}");
                anyhow!("Error during insert into loop")
            })?;
    }
    event_loop
        .handle()
        .insert_source(udev_backend, move |event, _, state| match event {
            UdevEvent::Added { device_id, path } => {
                if let Err(err) = DrmNode::from_dev_id(device_id)
                    .map_err(DeviceAddError::DrmNode)
                    .and_then(|node| device_added(state, node, &path))
                {
                    error!("Skipping device {device_id}: {err}");
                }
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    device_changed(state, node)
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    device_removed(state, node)
                }
            }
        })
        .unwrap();

    let primary_gpu = primary_gpu.clone();
    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_, _, state| {
            state.shm_state.update_formats(
                state
                    .backend_data
                    .udev_mut()
                    .gpus
                    .single_renderer(&primary_gpu)
                    .unwrap()
                    .shm_formats(),
            );

            let udev_data = state.backend_data.udev_mut();

            let mut renderer = udev_data.gpus.single_renderer(&primary_gpu).unwrap();

            #[cfg(feature = "debug")]
            {
                let fps_image = image::io::Reader::with_format(
                    std::io::Cursor::new(FPS_NUMBERS_PNG),
                    image::ImageFormat::Png,
                )
                .decode()
                .unwrap();
                let fps_texture = renderer
                    .import_memory(
                        &fps_image.to_rgba8(),
                        Fourcc::Abgr8888,
                        (fps_image.width() as i32, fps_image.height() as i32).into(),
                        false,
                    )
                    .expect("Unable to upload FPS texture");

                for backend in udev_data.backends.values_mut() {
                    for surface in backend.surfaces.values_mut() {
                        surface.fps_element = Some(FpsElement::new(fps_texture.clone()));
                    }
                }
                udev_data.fps_texture = Some(fps_texture);
            }

            info!(
                "Trying to initialize EGL Hardware Acceleration via {:?}",
                primary_gpu
            );

            match renderer.bind_wl_display(&state.display_handle) {
                Ok(_) => info!("EGL hardware-acceleration enabled"),
                Err(err) => info!(?err, "Failed to initialize EGL hardware-acceleration"),
            }

            // init dmabuf support with format list from our primary gpu
            let dmabuf_formats = renderer.dmabuf_formats().collect::<Vec<_>>();
            let default_feedback = DmabufFeedbackBuilder::new(primary_gpu.dev_id(), dmabuf_formats)
                .build()
                .unwrap();
            let mut dmabuf_state = DmabufState::new();
            let global = dmabuf_state.create_global_with_default_feedback::<State>(
                &state.display_handle,
                &default_feedback,
            );
            udev_data.dmabuf_state = Some((dmabuf_state, global));

            let gpus = &mut udev_data.gpus;
            udev_data.backends.values_mut().for_each(|backend_data| {
                // Update the per drm surface dmabuf feedback
                backend_data.surfaces.values_mut().for_each(|surface_data| {
                    surface_data.dmabuf_feedback =
                        surface_data.dmabuf_feedback.take().or_else(|| {
                            get_surface_dmabuf_feedback(
                                primary_gpu,
                                surface_data.render_node,
                                gpus,
                                &surface_data.compositor,
                            )
                        });
                });
            });

            TimeoutAction::Drop
        })
        .unwrap();

    Ok(BackendData::Udev(udev_data))
}

impl DrmLeaseHandler for State {
    fn drm_lease_state(&mut self, node: DrmNode) -> &mut DrmLeaseState {
        self.backend_data
            .udev_mut()
            .backends
            .get_mut(&node)
            .unwrap()
            .leasing_global
            .as_mut()
            .unwrap()
    }

    fn lease_request(
        &mut self,
        node: DrmNode,
        request: DrmLeaseRequest,
    ) -> Result<DrmLeaseBuilder, LeaseRejected> {
        let backend = self
            .backend_data
            .udev_mut()
            .backends
            .get(&node)
            .ok_or(LeaseRejected::default())?;

        let mut builder = DrmLeaseBuilder::new(&backend.drm);
        for conn in request.connectors {
            if let Some((_, crtc)) = backend
                .non_desktop_connectors
                .iter()
                .find(|(handle, _)| *handle == conn)
            {
                builder.add_connector(conn);
                builder.add_crtc(*crtc);
                let planes = backend
                    .drm
                    .planes(crtc)
                    .map_err(LeaseRejected::with_cause)?;
                builder.add_plane(planes.primary.handle);
                if let Some(cursor) = planes.cursor {
                    builder.add_plane(cursor.handle);
                }
            } else {
                tracing::warn!(
                    ?conn,
                    "Lease requested for desktop connector, denying request"
                );
                return Err(LeaseRejected::default());
            }
        }

        Ok(builder)
    }

    fn new_active_lease(&mut self, node: DrmNode, lease: DrmLease) {
        let backend = self
            .backend_data
            .udev_mut()
            .backends
            .get_mut(&node)
            .unwrap();
        backend.active_leases.push(lease);
    }

    fn lease_destroyed(&mut self, node: DrmNode, lease: u32) {
        let backend = self
            .backend_data
            .udev_mut()
            .backends
            .get_mut(&node)
            .unwrap();
        backend.active_leases.retain(|l| l.id() != lease);
    }
}
delegate_drm_lease!(State);

pub type RenderSurface =
    GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, Option<OutputPresentationFeedback>>;

pub type GbmDrmCompositor = DrmCompositor<
    GbmAllocator<DrmDeviceFd>,
    GbmDevice<DrmDeviceFd>,
    Option<OutputPresentationFeedback>,
    DrmDeviceFd,
>;

#[derive(Debug)]
enum SurfaceComposition {
    Surface {
        surface: RenderSurface,
        damage_tracker: OutputDamageTracker,
        debug_flags: DebugFlags,
    },
    Compositor(GbmDrmCompositor),
}

struct SurfaceCompositorRenderResult {
    rendered: bool,
    states: RenderElementStates,
    sync: Option<SyncPoint>,
    damage: Option<Vec<Rectangle<i32, Physical>>>,
}

impl SurfaceComposition {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn frame_submitted(
        &mut self,
    ) -> Result<Option<Option<OutputPresentationFeedback>>, SwapBuffersError> {
        match self {
            SurfaceComposition::Compositor(c) => {
                c.frame_submitted().map_err(Into::<SwapBuffersError>::into)
            }
            SurfaceComposition::Surface { surface, .. } => surface
                .frame_submitted()
                .map_err(Into::<SwapBuffersError>::into),
        }
    }

    fn format(&self) -> smithay::reexports::gbm::Format {
        match self {
            SurfaceComposition::Compositor(c) => c.format(),
            SurfaceComposition::Surface { surface, .. } => surface.format(),
        }
    }

    fn surface(&self) -> &DrmSurface {
        match self {
            SurfaceComposition::Compositor(c) => c.surface(),
            SurfaceComposition::Surface { surface, .. } => surface.surface(),
        }
    }

    fn reset_buffers(&mut self) {
        match self {
            SurfaceComposition::Compositor(c) => c.reset_buffers(),
            SurfaceComposition::Surface { surface, .. } => surface.reset_buffers(),
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn queue_frame(
        &mut self,
        sync: Option<SyncPoint>,
        damage: Option<Vec<Rectangle<i32, Physical>>>,
        user_data: Option<OutputPresentationFeedback>,
    ) -> Result<(), SwapBuffersError> {
        match self {
            SurfaceComposition::Surface { surface, .. } => surface
                .queue_buffer(sync, damage, user_data)
                .map_err(Into::<SwapBuffersError>::into),
            SurfaceComposition::Compositor(c) => c
                .queue_frame(user_data)
                .map_err(Into::<SwapBuffersError>::into),
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn render_frame<'a, R, E, Target>(
        &mut self,
        renderer: &mut R,
        elements: &'a [E],
        clear_color: [f32; 4],
    ) -> Result<SurfaceCompositorRenderResult, SwapBuffersError>
    where
        R: Renderer + Bind<Dmabuf> + Bind<Target> + Offscreen<Target> + ExportMem,
        <R as Renderer>::TextureId: 'static,
        <R as Renderer>::Error: Into<SwapBuffersError>,
        E: RenderElement<R>,
    {
        match self {
            SurfaceComposition::Surface {
                surface,
                damage_tracker,
                debug_flags,
            } => {
                let (dmabuf, age) = surface
                    .next_buffer()
                    .map_err(Into::<SwapBuffersError>::into)?;
                renderer
                    .bind(dmabuf)
                    .map_err(Into::<SwapBuffersError>::into)?;
                let current_debug_flags = renderer.debug_flags();
                renderer.set_debug_flags(*debug_flags);
                let res = damage_tracker
                    .render_output(renderer, age.into(), elements, clear_color)
                    .map(|res| {
                        #[cfg(feature = "renderer_sync")]
                        res.sync.wait();
                        let rendered = res.damage.is_some();
                        SurfaceCompositorRenderResult {
                            rendered,
                            damage: res.damage,
                            states: res.states,
                            sync: rendered.then_some(res.sync),
                        }
                    })
                    .map_err(|err| match err {
                        OutputDamageTrackerError::Rendering(err) => err.into(),
                        _ => unreachable!(),
                    });
                renderer.set_debug_flags(current_debug_flags);
                res
            }
            SurfaceComposition::Compositor(compositor) => compositor
                .render_frame(renderer, elements, clear_color)
                .map(|render_frame_result| {
                    #[cfg(feature = "renderer_sync")]
                    if let PrimaryPlaneElement::Swapchain(element) =
                        render_frame_result.primary_element
                    {
                        element.sync.wait();
                    }
                    SurfaceCompositorRenderResult {
                        rendered: render_frame_result.damage.is_some(),
                        damage: None,
                        states: render_frame_result.states,
                        sync: None,
                    }
                })
                .map_err(|err| match err {
                    smithay::backend::drm::compositor::RenderFrameError::PrepareFrame(err) => {
                        err.into()
                    }
                    smithay::backend::drm::compositor::RenderFrameError::RenderFrame(
                        OutputDamageTrackerError::Rendering(err),
                    ) => err.into(),
                    _ => unreachable!(),
                }),
        }
    }

    fn set_debug_flags(&mut self, flags: DebugFlags) {
        match self {
            SurfaceComposition::Surface {
                surface,
                debug_flags,
                ..
            } => {
                *debug_flags = flags;
                surface.reset_buffers();
            }
            SurfaceComposition::Compositor(c) => c.set_debug_flags(flags),
        }
    }
}

#[derive(Debug)]
struct DrmSurfaceDmabufFeedback {
    render_feedback: DmabufFeedback,
    scanout_feedback: DmabufFeedback,
}

#[derive(Debug)]
struct SurfaceData {
    dh: DisplayHandle,
    device_id: DrmNode,
    render_node: DrmNode,
    global: Option<GlobalId>,
    compositor: SurfaceComposition,
    #[cfg(feature = "debug")]
    fps: fps_ticker::Fps,
    #[cfg(feature = "debug")]
    fps_element: Option<FpsElement<MultiTexture>>,
    dmabuf_feedback: Option<DrmSurfaceDmabufFeedback>,
}

impl Drop for SurfaceData {
    fn drop(&mut self) {
        if let Some(global) = self.global.take() {
            self.dh.remove_global::<State>(global);
        }
    }
}

#[derive(Debug)]
struct DeviceData {
    surfaces: HashMap<crtc::Handle, SurfaceData>,
    non_desktop_connectors: Vec<(connector::Handle, crtc::Handle)>,
    leasing_global: Option<DrmLeaseState>,
    active_leases: Vec<DrmLease>,
    gbm: GbmDevice<DrmDeviceFd>,
    drm: DrmDevice,
    drm_scanner: DrmScanner,
    render_node: DrmNode,
    registration_token: RegistrationToken,
}

#[derive(Debug, thiserror::Error)]
enum DeviceAddError {
    #[error("Failed to open device using libseat: {0}")]
    DeviceOpen(libseat::Error),
    #[error("Failed to initialize drm device: {0}")]
    DrmDevice(DrmError),
    #[error("Failed to initialize gbm device: {0}")]
    GbmDevice(std::io::Error),
    #[error("Failed to access drm node: {0}")]
    DrmNode(CreateDrmNodeError),
    #[error("Failed to add device to GpuManager: {0}")]
    AddNode(egl::Error),
}

fn get_surface_dmabuf_feedback(
    primary_gpu: DrmNode,
    render_node: DrmNode,
    gpus: &mut GpuManager<GbmGlesBackend<GlesRenderer>>,
    composition: &SurfaceComposition,
) -> Option<DrmSurfaceDmabufFeedback> {
    let primary_formats = gpus
        .single_renderer(&primary_gpu)
        .ok()?
        .dmabuf_formats()
        .collect::<HashSet<_>>();

    let render_formats = gpus
        .single_renderer(&render_node)
        .ok()?
        .dmabuf_formats()
        .collect::<HashSet<_>>();

    let all_render_formats = primary_formats
        .iter()
        .chain(render_formats.iter())
        .copied()
        .collect::<HashSet<_>>();

    let surface = composition.surface();
    let planes = surface.planes().clone();
    // We limit the scan-out trache to formats we can also render from
    // so that there is always a fallback render path available in case
    // the supplied buffer can not be scanned out directly
    let planes_formats = planes
        .primary
        .formats
        .into_iter()
        .chain(planes.overlay.into_iter().flat_map(|p| p.formats))
        .collect::<HashSet<_>>()
        .intersection(&all_render_formats)
        .copied()
        .collect::<Vec<_>>();

    let builder = DmabufFeedbackBuilder::new(primary_gpu.dev_id(), primary_formats);
    let render_feedback = builder
        .clone()
        .add_preference_tranche(render_node.dev_id(), None, render_formats.clone())
        .build()
        .unwrap();

    let scanout_feedback = builder
        .add_preference_tranche(
            surface.device_fd().dev_id().unwrap(),
            Some(zwp_linux_dmabuf_feedback_v1::TrancheFlags::Scanout),
            planes_formats,
        )
        .add_preference_tranche(render_node.dev_id(), None, render_formats)
        .build()
        .unwrap();

    Some(DrmSurfaceDmabufFeedback {
        render_feedback,
        scanout_feedback,
    })
}

fn device_added(state: &mut State, node: DrmNode, path: &Path) -> Result<(), DeviceAddError> {
    let udev_data = state.backend_data.udev_mut();
    // Try to open the device
    let fd = udev_data
        .session
        .open(
            path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(DeviceAddError::DeviceOpen)?;

    let fd = DrmDeviceFd::new(DeviceFd::from(fd));

    let (drm, notifier) = DrmDevice::new(fd.clone(), true).map_err(DeviceAddError::DrmDevice)?;
    let gbm = GbmDevice::new(fd).map_err(DeviceAddError::GbmDevice)?;

    let registration_token = state
        .loop_handle
        .insert_source(notifier, move |event, metadata, state| match event {
            DrmEvent::VBlank(crtc) => {
                #[cfg(feature = "profiling")]
                profiling::scope!("vblank", &format!("{crtc:?}"));
                frame_finish(state, node, crtc, metadata);
            }
            DrmEvent::Error(error) => {
                error!("{:?}", error);
            }
        })
        .unwrap();

    let render_node = EGLDevice::device_for_display(&EGLDisplay::new(gbm.clone()).unwrap())
        .ok()
        .and_then(|x| x.try_get_render_node().ok().flatten())
        .unwrap_or(node);

    udev_data
        .gpus
        .as_mut()
        .add_node(render_node, gbm.clone())
        .map_err(DeviceAddError::AddNode)?;

    udev_data.backends.insert(
        node,
        DeviceData {
            registration_token,
            gbm,
            drm,
            drm_scanner: DrmScanner::new(),
            non_desktop_connectors: Vec::new(),
            render_node,
            surfaces: HashMap::new(),
            leasing_global: DrmLeaseState::new::<State>(&state.display_handle, &node)
                .map_err(|err| {
                    // TODO replace with inspect_err, once stable
                    warn!(?err, "Failed to initialize drm lease global for: {}", node);
                    err
                })
                .ok(),
            active_leases: Vec::new(),
        },
    );

    device_changed(state, node);

    Ok(())
}

fn connector_connected(
    state: &mut State,
    node: DrmNode,
    connector: connector::Info,
    crtc: crtc::Handle,
) {
    let udev_data = state.backend_data.udev_mut();
    let device = if let Some(device) = udev_data.backends.get_mut(&node) {
        device
    } else {
        return;
    };

    let mut renderer = udev_data.gpus.single_renderer(&device.render_node).unwrap();
    let render_formats = renderer
        .as_mut()
        .egl_context()
        .dmabuf_render_formats()
        .clone();

    let output_name = format!(
        "{}-{}",
        connector.interface().as_str(),
        connector.interface_id()
    );
    info!(?crtc, "Trying to setup connector {}", output_name,);

    let non_desktop = device
        .drm
        .get_properties(connector.handle())
        .ok()
        .and_then(|props| {
            let (info, value) = props
                .into_iter()
                .filter_map(|(handle, value)| {
                    let info = device.drm.get_property(handle).ok()?;

                    Some((info, value))
                })
                .find(|(info, _)| info.name().to_str() == Ok("non-desktop"))?;

            info.value_type().convert_value(value).as_boolean()
        })
        .unwrap_or(false);

    let (make, model) = EdidInfo::for_connector(&device.drm, connector.handle())
        .map(|info| (info.manufacturer, info.model))
        .unwrap_or_else(|| ("Unknown".into(), "Unknown".into()));

    if non_desktop {
        info!(
            "Connector {} is non-desktop, setting up for leasing",
            output_name
        );
        device
            .non_desktop_connectors
            .push((connector.handle(), crtc));
        if let Some(lease_state) = device.leasing_global.as_mut() {
            lease_state.add_connector::<State>(
                connector.handle(),
                output_name,
                format!("{} {}", make, model),
            );
        }
    } else {
        let mode_id = connector
            .modes()
            .iter()
            .position(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .unwrap_or(0);

        let drm_mode = connector.modes()[mode_id];
        let wl_mode = WlMode::from(drm_mode);

        let surface = match device
            .drm
            .create_surface(crtc, drm_mode, &[connector.handle()])
        {
            Ok(surface) => surface,
            Err(err) => {
                warn!("Failed to create drm surface: {}", err);
                return;
            }
        };

        let (phys_w, phys_h) = connector.size().unwrap_or((0, 0));
        let output = Output::new(
            output_name,
            PhysicalProperties {
                size: (phys_w as i32, phys_h as i32).into(),
                subpixel: Subpixel::Unknown,
                make,
                model,
            },
        );
        let global = output.create_global::<State>(&state.display_handle);

        let x = state.space.outputs().fold(0, |acc, o| {
            acc + state.space.output_geometry(o).unwrap().size.w
        });
        let position = (x, 0).into();

        output.set_preferred(wl_mode);
        output.change_current_state(Some(wl_mode), None, None, Some(position));
        state.space.map_output(&output, position);

        output.user_data().insert_if_missing(|| UdevOutputId {
            crtc,
            device_id: node,
        });

        #[cfg(feature = "debug")]
        let fps_element = udev_data.fps_texture.clone().map(FpsElement::new);

        let allocator = GbmAllocator::new(
            device.gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );

        let color_formats = if std::env::var("ANVIL_DISABLE_10BIT").is_ok() {
            SUPPORTED_FORMATS_8BIT_ONLY
        } else {
            SUPPORTED_FORMATS
        };

        let compositor = if std::env::var("ANVIL_DISABLE_DRM_COMPOSITOR").is_ok() {
            let gbm_surface =
                match GbmBufferedSurface::new(surface, allocator, color_formats, render_formats) {
                    Ok(renderer) => renderer,
                    Err(err) => {
                        warn!("Failed to create rendering surface: {}", err);
                        return;
                    }
                };
            SurfaceComposition::Surface {
                surface: gbm_surface,
                damage_tracker: OutputDamageTracker::from_output(&output),
                debug_flags: udev_data.debug_flags,
            }
        } else {
            let driver = match device.drm.get_driver() {
                Ok(driver) => driver,
                Err(err) => {
                    warn!("Failed to query drm driver: {}", err);
                    return;
                }
            };

            let mut planes = surface.planes().clone();

            // Using an overlay plane on a nvidia card breaks
            if driver
                .name()
                .to_string_lossy()
                .to_lowercase()
                .contains("nvidia")
                || driver
                    .description()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("nvidia")
            {
                planes.overlay = vec![];
            }

            let mut compositor = match DrmCompositor::new(
                &output,
                surface,
                Some(planes),
                allocator,
                device.gbm.clone(),
                color_formats,
                render_formats,
                device.drm.cursor_size(),
                Some(device.gbm.clone()),
            ) {
                Ok(compositor) => compositor,
                Err(err) => {
                    warn!("Failed to create drm compositor: {}", err);
                    return;
                }
            };
            compositor.set_debug_flags(udev_data.debug_flags);
            SurfaceComposition::Compositor(compositor)
        };

        let dmabuf_feedback = get_surface_dmabuf_feedback(
            udev_data.primary_gpu,
            device.render_node,
            &mut udev_data.gpus,
            &compositor,
        );

        let surface = SurfaceData {
            dh: state.display_handle.clone(),
            device_id: node,
            render_node: device.render_node,
            global: Some(global),
            compositor,
            #[cfg(feature = "debug")]
            fps: fps_ticker::Fps::default(),
            #[cfg(feature = "debug")]
            fps_element,
            dmabuf_feedback,
        };

        device.surfaces.insert(crtc, surface);

        schedule_initial_render(udev_data, node, crtc, state.loop_handle.clone());
    }
}

fn connector_disconnected(
    state: &mut State,
    node: DrmNode,
    connector: connector::Info,
    crtc: crtc::Handle,
) {
    let udev_data = state.backend_data.udev_mut();
    let device = if let Some(device) = udev_data.backends.get_mut(&node) {
        device
    } else {
        return;
    };

    if let Some(pos) = device
        .non_desktop_connectors
        .iter()
        .position(|(handle, _)| *handle == connector.handle())
    {
        let _ = device.non_desktop_connectors.remove(pos);
        if let Some(leasing_state) = device.leasing_global.as_mut() {
            leasing_state.withdraw_connector(connector.handle());
        }
    } else {
        device.surfaces.remove(&crtc);

        let output = state
            .space
            .outputs()
            .find(|o| {
                o.user_data()
                    .get::<UdevOutputId>()
                    .map(|id| id.device_id == node && id.crtc == crtc)
                    .unwrap_or(false)
            })
            .cloned();

        if let Some(output) = output {
            state.space.unmap_output(&output);
        }
    }
}

fn device_changed(state: &mut State, node: DrmNode) {
    let udev_data = state.backend_data.udev_mut();

    let device = if let Some(device) = udev_data.backends.get_mut(&node) {
        device
    } else {
        return;
    };

    for event in device.drm_scanner.scan_connectors(&device.drm) {
        match event {
            DrmScanEvent::Connected {
                connector,
                crtc: Some(crtc),
            } => {
                connector_connected(state, node, connector, crtc);
            }
            DrmScanEvent::Disconnected {
                connector,
                crtc: Some(crtc),
            } => {
                connector_disconnected(state, node, connector, crtc);
            }
            _ => {}
        }
    }
    // fixup window coordinates
    crate::shell::fixup_positions(&mut state.space, state.pointer.current_location());
}

fn device_removed(state: &mut State, node: DrmNode) {
    let device = if let Some(device) = state.backend_data.udev_mut().backends.get_mut(&node) {
        device
    } else {
        return;
    };

    let crtcs: Vec<_> = device
        .drm_scanner
        .crtcs()
        .map(|(info, crtc)| (info.clone(), crtc))
        .collect();

    for (connector, crtc) in crtcs {
        connector_disconnected(state, node, connector, crtc);
    }

    tracing::debug!("Surfaces dropped");

    let udev_data = state.backend_data.udev_mut();
    // drop the backends on this side
    if let Some(mut backend_data) = udev_data.backends.remove(&node) {
        if let Some(mut leasing_global) = backend_data.leasing_global.take() {
            leasing_global.disable_global::<State>();
        }

        udev_data
            .gpus
            .as_mut()
            .remove_node(&backend_data.render_node);

        state.loop_handle.remove(backend_data.registration_token);

        tracing::debug!("Dropping device");
    }

    crate::shell::fixup_positions(&mut state.space, state.pointer.current_location());
}

fn frame_finish(
    state: &mut State,
    dev_id: DrmNode,
    crtc: crtc::Handle,
    metadata: &mut Option<DrmEventMetadata>,
) {
    #[cfg(feature = "profiling")]
    profiling::scope!("frame_finish", &format!("{crtc:?}"));

    let udev_data = state.backend_data.udev_mut();
    let device_backend = match udev_data.backends.get_mut(&dev_id) {
        Some(backend) => backend,
        None => {
            error!("Trying to finish frame on non-existent backend {}", dev_id);
            return;
        }
    };

    let surface = match device_backend.surfaces.get_mut(&crtc) {
        Some(surface) => surface,
        None => {
            error!("Trying to finish frame on non-existent crtc {:?}", crtc);
            return;
        }
    };

    let output = if let Some(output) = state.space.outputs().find(|o| {
        o.user_data().get::<UdevOutputId>()
            == Some(&UdevOutputId {
                device_id: surface.device_id,
                crtc,
            })
    }) {
        output.clone()
    } else {
        // somehow we got called with an invalid output
        return;
    };

    let schedule_render = match surface
        .compositor
        .frame_submitted()
        .map_err(Into::<SwapBuffersError>::into)
    {
        Ok(user_data) => {
            if let Some(mut feedback) = user_data.flatten() {
                let tp = metadata.as_ref().and_then(|metadata| match metadata.time {
                    smithay::backend::drm::DrmEventTime::Monotonic(tp) => Some(tp),
                    smithay::backend::drm::DrmEventTime::Realtime(_) => None,
                });
                let seq = metadata
                    .as_ref()
                    .map(|metadata| metadata.sequence)
                    .unwrap_or(0);

                let (clock, flags) = if let Some(tp) = tp {
                    (
                        tp.into(),
                        wp_presentation_feedback::Kind::Vsync
                            | wp_presentation_feedback::Kind::HwClock
                            | wp_presentation_feedback::Kind::HwCompletion,
                    )
                } else {
                    (state.clock.now(), wp_presentation_feedback::Kind::Vsync)
                };

                feedback.presented(
                    clock,
                    output
                        .current_mode()
                        .map(|mode| Duration::from_secs_f64(1_000f64 / mode.refresh as f64))
                        .unwrap_or_default(),
                    seq as u64,
                    flags,
                );
            }

            true
        }
        Err(err) => {
            warn!("Error during rendering: {:?}", err);
            match err {
                SwapBuffersError::AlreadySwapped => true,
                // If the device has been deactivated do not reschedule, this will be done
                // by session resume
                SwapBuffersError::TemporaryFailure(err)
                    if matches!(
                        err.downcast_ref::<DrmError>(),
                        Some(&DrmError::DeviceInactive)
                    ) =>
                {
                    false
                }
                SwapBuffersError::TemporaryFailure(err) => matches!(
                    err.downcast_ref::<DrmError>(),
                    Some(DrmError::Access {
                        source,
                        ..
                    }) if source.kind() == io::ErrorKind::PermissionDenied
                ),
                SwapBuffersError::ContextLost(err) => {
                    panic!("Rendering loop lost: {}", err)
                }
            }
        }
    };

    if schedule_render {
        let output_refresh = match output.current_mode() {
            Some(mode) => mode.refresh,
            None => return,
        };
        // What are we trying to solve by introducing a delay here:
        //
        // Basically it is all about latency of client provided buffers.
        // A client driven by frame callbacks will wait for a frame callback
        // to repaint and submit a new buffer. As we send frame callbacks
        // as part of the repaint in the compositor the latency would always
        // be approx. 2 frames. By introducing a delay before we repaint in
        // the compositor we can reduce the latency to approx. 1 frame + the
        // remaining duration from the repaint to the next VBlank.
        //
        // With the delay it is also possible to further reduce latency if
        // the client is driven by presentation feedback. As the presentation
        // feedback is directly sent after a VBlank the client can submit a
        // new buffer during the repaint delay that can hit the very next
        // VBlank, thus reducing the potential latency to below one frame.
        //
        // Choosing a good delay is a topic on its own so we just implement
        // a simple strategy here. We just split the duration between two
        // VBlanks into two steps, one for the client repaint and one for the
        // compositor repaint. Theoretically the repaint in the compositor should
        // be faster so we give the client a bit more time to repaint. On a typical
        // modern system the repaint in the compositor should not take more than 2ms
        // so this should be safe for refresh rates up to at least 120 Hz. For 120 Hz
        // this results in approx. 3.33ms time for repainting in the compositor.
        // A too big delay could result in missing the next VBlank in the compositor.
        //
        // A more complete solution could work on a sliding window analyzing past repaints
        // and do some prediction for the next repaint.
        let repaint_delay =
            Duration::from_millis(((1_000_000f32 / output_refresh as f32) * 0.6f32) as u64);

        let timer = if udev_data.primary_gpu != surface.render_node {
            // However, if we need to do a copy, that might not be enough.
            // (And without actual comparision to previous frames we cannot really know.)
            // So lets ignore that in those cases to avoid thrashing performance.
            trace!("scheduling repaint timer immediately on {:?}", crtc);
            Timer::immediate()
        } else {
            trace!(
                "scheduling repaint timer with delay {:?} on {:?}",
                repaint_delay,
                crtc
            );
            Timer::from_duration(repaint_delay)
        };

        state
            .loop_handle
            .insert_source(timer, move |_, _, state| {
                render(state, dev_id, Some(crtc));
                TimeoutAction::Drop
            })
            .expect("failed to schedule frame timer");
    }
}

// If crtc is `Some()`, render it, else render all crtcs
fn render(state: &mut State, node: DrmNode, crtc: Option<crtc::Handle>) {
    let device_backend = match state.backend_data.udev_mut().backends.get_mut(&node) {
        Some(backend) => backend,
        None => {
            error!("Trying to render on non-existent backend {}", node);
            return;
        }
    };

    if let Some(crtc) = crtc {
        render_surface_crtc(state, node, crtc);
    } else {
        let crtcs: Vec<_> = device_backend.surfaces.keys().copied().collect();
        for crtc in crtcs {
            render_surface_crtc(state, node, crtc);
        }
    };
}

fn render_surface_crtc(state: &mut State, node: DrmNode, crtc: crtc::Handle) {
    #[cfg(feature = "profiling")]
    profiling::scope!("render_surface", &format!("{crtc:?}"));
    let udev_data = state.backend_data.udev_mut();
    let device = if let Some(device) = udev_data.backends.get_mut(&node) {
        device
    } else {
        return;
    };

    let surface = if let Some(surface) = device.surfaces.get_mut(&crtc) {
        surface
    } else {
        return;
    };

    let start = Instant::now();

    // TODO get scale from the rendersurface when supporting HiDPI
    let frame = udev_data
        .pointer_image
        .get_image(1 /*scale*/, state.clock.now().try_into().unwrap());

    let render_node = surface.render_node;
    let primary_gpu = udev_data.primary_gpu;
    let mut renderer = if primary_gpu == render_node {
        udev_data.gpus.single_renderer(&render_node)
    } else {
        let format = surface.compositor.format();
        udev_data.gpus.renderer(
            &primary_gpu,
            &render_node,
            udev_data
                .allocator
                .as_mut()
                // TODO: We could build some kind of `GLAllocator` using Renderbuffers in theory for this case.
                //  That would work for memcpy's of offscreen contents.
                .expect("We need an allocator for multigpu systems")
                .as_mut(),
            format,
        )
    }
    .unwrap();

    let pointer_images = &mut udev_data.pointer_images;
    let pointer_image = pointer_images
        .iter()
        .find_map(|(image, texture)| {
            if image == &frame {
                Some(texture.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            let texture = TextureBuffer::from_memory(
                &mut renderer,
                &frame.pixels_rgba,
                Fourcc::Abgr8888,
                (frame.width as i32, frame.height as i32),
                false,
                1,
                Transform::Normal,
                None,
            )
            .expect("Failed to import cursor bitmap");
            pointer_images.push((frame, texture.clone()));
            texture
        });

    let output = if let Some(output) = state.space.outputs().find(|o| {
        o.user_data().get::<UdevOutputId>()
            == Some(&UdevOutputId {
                device_id: surface.device_id,
                crtc,
            })
    }) {
        output.clone()
    } else {
        // somehow we got called with an invalid output
        return;
    };

    let result = render_surface(
        surface,
        &mut renderer,
        &state.space,
        &output,
        state.pointer.current_location(),
        &pointer_image,
        &mut udev_data.pointer_element,
        &state.dnd_icon,
        &mut state.cursor_status.lock().unwrap(),
        &state.clock,
        state.show_window_preview,
    );
    let reschedule = match &result {
        Ok(has_rendered) => !has_rendered,
        Err(err) => {
            warn!("Error during rendering: {:?}", err);
            match err {
                SwapBuffersError::AlreadySwapped => false,
                SwapBuffersError::TemporaryFailure(err) => match err.downcast_ref::<DrmError>() {
                    Some(DrmError::DeviceInactive) => true,
                    Some(DrmError::Access { source, .. }) => {
                        source.kind() == io::ErrorKind::PermissionDenied
                    }
                    _ => false,
                },
                SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
            }
        }
    };

    if reschedule {
        let output_refresh = match output.current_mode() {
            Some(mode) => mode.refresh,
            None => return,
        };
        // If reschedule is true we either hit a temporary failure or more likely rendering
        // did not cause any damage on the output. In this case we just re-schedule a repaint
        // after approx. one frame to re-test for damage.
        let reschedule_duration =
            Duration::from_millis((1_000_000f32 / output_refresh as f32) as u64);
        trace!(
            "reschedule repaint timer with delay {:?} on {:?}",
            reschedule_duration,
            crtc,
        );
        let timer = Timer::from_duration(reschedule_duration);
        state
            .loop_handle
            .insert_source(timer, move |_, _, state| {
                render(state, node, Some(crtc));
                TimeoutAction::Drop
            })
            .expect("failed to schedule frame timer");
    } else {
        let elapsed = start.elapsed();
        tracing::trace!(?elapsed, "rendered surface");
    }

    #[cfg(feature = "profiling")]
    profiling::finish_frame!();
}

fn schedule_initial_render(
    udev_data: &mut UdevData,
    node: DrmNode,
    crtc: crtc::Handle,
    evt_handle: LoopHandle<'static, State>,
) {
    let device = if let Some(device) = udev_data.backends.get_mut(&node) {
        device
    } else {
        return;
    };

    let surface = if let Some(surface) = device.surfaces.get_mut(&crtc) {
        surface
    } else {
        return;
    };

    let node = surface.render_node;
    let result = {
        let mut renderer = udev_data.gpus.single_renderer(&node).unwrap();
        initial_render(surface, &mut renderer)
    };

    if let Err(err) = result {
        match err {
            SwapBuffersError::AlreadySwapped => {}
            SwapBuffersError::TemporaryFailure(err) => {
                // TODO dont reschedule after 3(?) retries
                warn!("Failed to submit page_flip: {}", err);
                let handle = evt_handle.clone();
                evt_handle.insert_idle(move |state| {
                    let BackendData::Udev(udev_data) = &mut state.backend_data else {
                        error!("Received non udev backend data");
                        return;
                    };
                    schedule_initial_render(udev_data, node, crtc, handle);
                });
            }
            SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(feature = "profiling", profiling::function)]
fn render_surface<'a, 'b>(
    surface: &'a mut SurfaceData,
    renderer: &mut UdevRenderer<'a, 'b>,
    space: &Space<WindowElement>,
    output: &Output,
    pointer_location: Point<f64, Logical>,
    pointer_image: &TextureBuffer<MultiTexture>,
    pointer_element: &mut PointerElement<MultiTexture>,
    dnd_icon: &Option<wl_surface::WlSurface>,
    cursor_status: &mut CursorImageStatus,
    clock: &Clock<Monotonic>,
    show_window_preview: bool,
) -> Result<bool, SwapBuffersError> {
    let output_geometry = space.output_geometry(output).unwrap();
    let scale = Scale::from(output.current_scale().fractional_scale());

    let mut custom_elements: Vec<CustomRenderElements<_>> = Vec::new();

    if output_geometry.to_f64().contains(pointer_location) {
        let cursor_hotspot = if let CursorImageStatus::Surface(ref surface) = cursor_status {
            compositor::with_states(surface, |states| {
                states
                    .data_map
                    .get::<Mutex<CursorImageAttributes>>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .hotspot
            })
        } else {
            (0, 0).into()
        };
        let cursor_pos = pointer_location - output_geometry.loc.to_f64() - cursor_hotspot.to_f64();
        let cursor_pos_scaled = cursor_pos.to_physical(scale).to_i32_round();

        // set cursor
        pointer_element.set_texture(pointer_image.clone());

        // draw the cursor as relevant
        {
            // reset the cursor if the surface is no longer alive
            let mut reset = false;
            if let CursorImageStatus::Surface(ref surface) = *cursor_status {
                reset = !surface.alive();
            }
            if reset {
                *cursor_status = CursorImageStatus::default_named();
            }

            pointer_element.set_status(cursor_status.clone());
        }

        custom_elements.extend(pointer_element.render_elements(
            renderer,
            cursor_pos_scaled,
            scale,
            1.0,
        ));

        // draw the dnd icon if applicable
        {
            if let Some(wl_surface) = dnd_icon.as_ref() {
                if wl_surface.alive() {
                    custom_elements.extend(
                        AsRenderElements::<UdevRenderer<'a, 'b>>::render_elements(
                            &SurfaceTree::from_surface(wl_surface),
                            renderer,
                            cursor_pos_scaled,
                            scale,
                            1.0,
                        ),
                    );
                }
            }
        }
    }

    #[cfg(feature = "debug")]
    if let Some(element) = surface.fps_element.as_mut() {
        element.update_fps(surface.fps.avg().round() as u32);
        surface.fps.tick();
        custom_elements.push(CustomRenderElements::Fps(element.clone()));
    }

    let (elements, clear_color) = output_elements(
        output,
        space,
        custom_elements,
        renderer,
        show_window_preview,
    );
    let res =
        surface
            .compositor
            .render_frame::<_, _, GlesTexture>(renderer, &elements, clear_color)?;

    post_repaint(
        output,
        &res.states,
        space,
        surface
            .dmabuf_feedback
            .as_ref()
            .map(|feedback| SurfaceDmabufFeedback {
                render_feedback: &feedback.render_feedback,
                scanout_feedback: &feedback.scanout_feedback,
            }),
        clock.now(),
    );

    if res.rendered {
        let output_presentation_feedback = take_presentation_feedback(output, space, &res.states);
        surface
            .compositor
            .queue_frame(res.sync, res.damage, Some(output_presentation_feedback))
            .map_err(Into::<SwapBuffersError>::into)?;
    }

    Ok(res.rendered)
}

fn initial_render(
    surface: &mut SurfaceData,
    renderer: &mut UdevRenderer<'_, '_>,
) -> Result<(), SwapBuffersError> {
    surface
        .compositor
        .render_frame::<_, CustomRenderElements<_>, GlesTexture>(renderer, &[], CLEAR_COLOR)?;
    surface.compositor.queue_frame(None, None, None)?;
    surface.compositor.reset_buffers();

    Ok(())
}
