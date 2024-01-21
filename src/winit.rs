use crate::{
    drawing::*,
    render::CustomRenderElements,
    state::{post_repaint, take_presentation_feedback, BackendData, State},
};
use anyhow::{anyhow, Result};
use calloop::timer::{TimeoutAction, Timer};
#[cfg(feature = "debug")]
use smithay::backend::{allocator::Fourcc, renderer::ImportMem};
use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        egl::EGLDevice,
        input::InputEvent,
        renderer::{
            damage::{Error as OutputDamageTrackerError, OutputDamageTracker},
            gles::{GlesRenderer, GlesTexture},
            ImportDma,
        },
        winit::{self, WinitGraphicsBackend},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_server::{protocol::wl_surface, DisplayHandle},
        winit::raw_window_handle::{HasWindowHandle, RawWindowHandle},
    },
    utils::Transform,
    wayland::dmabuf::{DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufState},
};
use smithay::{
    backend::{
        renderer::ImportMemWl,
        winit::{WinitEvent, WinitEventLoop, WinitInput},
    },
    reexports::{
        wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
        winit::platform::pump_events::PumpStatus,
    },
};
use smithay::{
    backend::{
        renderer::{element::AsRenderElements, ImportEgl},
        SwapBuffersError,
    },
    input::pointer::{CursorImageAttributes, CursorImageStatus},
};
use smithay::{
    utils::{IsAlive, Scale},
    wayland::dmabuf::ImportNotifier,
};
use std::sync::Mutex;
use std::time::Duration;
use tracing::{error, info, warn};

pub const OUTPUT_NAME: &str = "winit";

#[derive(Debug)]
pub struct WinitData {
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
    dmabuf_state: (DmabufState, DmabufGlobal, Option<DmabufFeedback>),
    pointer_element: PointerElement<GlesTexture>,
    full_redraw: u8,
    winit_loop: WinitEventLoop,
    pending_input_events: Vec<InputEvent<WinitInput>>,
    output: Output,
    #[cfg(feature = "debug")]
    pub fps: fps_ticker::Fps,
    #[cfg(feature = "debug")]
    fps_texture: GlesTexture,
}

impl WinitData {
    pub fn has_relative_motion(&self) -> bool {
        false
    }

    pub fn seat_name(&self) -> String {
        String::from("winit")
    }

    pub fn reset_buffers(&mut self, _output: &Output) {
        self.full_redraw = 4;
    }

    pub fn early_import(&mut self, _surface: &wl_surface::WlSurface) {}

    pub fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state.0
    }

    pub fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self.backend.renderer().import_dmabuf(&dmabuf, None).is_ok() {
            let _ = notifier.successful::<State>();
        } else {
            notifier.failed();
        }
    }
}

pub fn init_winit(
    display_handle: DisplayHandle,
    event_loop: &mut EventLoop<State>,
) -> Result<BackendData> {
    #[cfg_attr(not(feature = "egl"), allow(unused_mut))]
    let (mut backend, winit) = winit::init::<GlesRenderer>().map_err(|e| {
        error!("Failed to initialize Winit backend: {}", e);
        anyhow!("Winit backend cannot be started")
    })?;

    let size = backend.window_size();
    let mode = Mode {
        size,
        refresh: 60_000,
    };
    let output = Output::new(
        OUTPUT_NAME.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Scape".into(),
            model: "Winit".into(),
        },
    );
    let _global = output.create_global::<State>(&display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);

    #[cfg(feature = "debug")]
    let fps_image = image::io::Reader::with_format(
        std::io::Cursor::new(FPS_NUMBERS_PNG),
        image::ImageFormat::Png,
    )
    .decode()
    .unwrap();
    #[cfg(feature = "debug")]
    let fps_texture = backend
        .renderer()
        .import_memory(
            &fps_image.to_rgba8(),
            Fourcc::Abgr8888,
            (fps_image.width() as i32, fps_image.height() as i32).into(),
            false,
        )
        .expect("Unable to upload FPS texture");

    let render_node = EGLDevice::device_for_display(backend.renderer().egl_context().display())
        .and_then(|device| device.try_get_render_node());

    let dmabuf_default_feedback = match render_node {
        Ok(Some(node)) => {
            let dmabuf_formats = backend.renderer().dmabuf_formats().collect::<Vec<_>>();
            let dmabuf_default_feedback = DmabufFeedbackBuilder::new(node.dev_id(), dmabuf_formats)
                .build()
                .unwrap();
            Some(dmabuf_default_feedback)
        }
        Ok(None) => {
            warn!("failed to query render node, dmabuf will use v3");
            None
        }
        Err(err) => {
            warn!(?err, "failed to egl device for display, dmabuf will use v3");
            None
        }
    };

    // if we failed to build dmabuf feedback we fall back to dmabuf v3
    // Note: egl on Mesa requires either v4 or wl_drm (initialized with bind_wl_display)
    let dmabuf_state = if let Some(default_feedback) = dmabuf_default_feedback {
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_global = dmabuf_state
            .create_global_with_default_feedback::<State>(&display_handle, &default_feedback);
        (dmabuf_state, dmabuf_global, Some(default_feedback))
    } else {
        let dmabuf_formats = backend.renderer().dmabuf_formats().collect::<Vec<_>>();
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_global = dmabuf_state.create_global::<State>(&display_handle, dmabuf_formats);
        (dmabuf_state, dmabuf_global, None)
    };

    #[cfg(feature = "egl")]
    if backend.renderer().bind_wl_display(&display_handle).is_ok() {
        info!("EGL hardware-acceleration enabled");
    };

    let pointer_element = PointerElement::<GlesTexture>::default();

    let damage_tracker = OutputDamageTracker::from_output(&output);

    event_loop
        .handle()
        .insert_source(Timer::immediate(), |_event, &mut (), state| {
            let output = state.backend_data.winit().output.clone();
            state.space.map_output(&output, (0, 0));
            TimeoutAction::Drop
        })
        .unwrap();

    event_loop
        .handle()
        .insert_source(Timer::immediate(), |_, _, state| {
            run_tick(state);
            TimeoutAction::ToDuration(Duration::from_millis(16))
        })
        .unwrap();

    Ok(BackendData::Winit(WinitData {
        backend,
        damage_tracker,
        dmabuf_state,
        pointer_element,
        full_redraw: 0,
        winit_loop: winit,
        pending_input_events: vec![],
        output,
        #[cfg(feature = "debug")]
        fps: fps_ticker::Fps::default(),
        #[cfg(feature = "debug")]
        fps_texture,
    }))
}

fn run_tick(state: &mut State) {
    let winit_data = state.backend_data.winit_mut();
    let mut handle_events = false;
    if let PumpStatus::Exit(_) = winit_data
        .winit_loop
        .dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                // We only have one output
                let output = state.space.outputs().next().unwrap().clone();
                state
                    .shm_state
                    .update_formats(winit_data.backend.renderer().shm_formats());
                state.space.map_output(&output, (0, 0));
                let mode = Mode {
                    size,
                    refresh: 60_000,
                };
                output.change_current_state(Some(mode), None, None, None);
                output.set_preferred(mode);
                crate::shell::fixup_positions(&mut state.space, state.pointer.current_location());
            }
            WinitEvent::Input(event) => {
                winit_data.pending_input_events.push(event);
                handle_events = true;
            }
            _ => (),
        })
    {
        // TODO: probably exit the main loop
        return;
    }

    if handle_events {
        state
            .loop_handle
            .insert_source(Timer::immediate(), |_, _, state| {
                let display_handle = state.display_handle.clone();
                let pending_events = std::mem::replace(
                    &mut state.backend_data.winit_mut().pending_input_events,
                    vec![],
                );
                for event in pending_events {
                    state.process_input_event_windowed(&display_handle, event, OUTPUT_NAME);
                }
                TimeoutAction::Drop
            })
            .unwrap();
    }

    // drawing logic
    {
        let backend = &mut winit_data.backend;
        let output = state.space.outputs().next().unwrap().clone();

        // draw the cursor as relevant
        // reset the cursor if the surface is no longer alive
        let mut reset = false;
        if let CursorImageStatus::Surface(ref surface) = state.cursor_status {
            reset = !surface.alive();
        }
        if reset {
            state.cursor_status = CursorImageStatus::default_named();
        }
        let cursor_visible = !matches!(state.cursor_status, CursorImageStatus::Surface(_));

        winit_data
            .pointer_element
            .set_status(state.cursor_status.clone());

        #[cfg(feature = "debug")]
        let mut fps_element = FpsElement::new(winit_data.fps_texture.clone());
        #[cfg(feature = "debug")]
        {
            let fps = winit_data.fps.avg().round() as u32;
            fps_element.update_fps(fps);
        }

        let full_redraw = &mut winit_data.full_redraw;
        *full_redraw = full_redraw.saturating_sub(1);
        let space = &mut state.space;
        let damage_tracker = &mut winit_data.damage_tracker;
        let show_window_preview = state.show_window_preview;

        let dnd_icon = state.dnd_icon.as_ref();

        let scale = Scale::from(output.current_scale().fractional_scale());
        let cursor_hotspot = if let CursorImageStatus::Surface(ref surface) = state.cursor_status {
            smithay::wayland::compositor::with_states(surface, |states| {
                if let Ok(attr) = states
                    .data_map
                    .get::<Mutex<CursorImageAttributes>>()
                    .unwrap()
                    .try_lock()
                {
                    attr.hotspot
                } else {
                    warn!("Unable to lock CursorImageAttributes in run_tick");
                    (0, 0).into()
                }
            })
        } else {
            (0, 0).into()
        };
        let cursor_pos = state.pointer.current_location() - cursor_hotspot.to_f64();
        let cursor_pos_scaled = cursor_pos.to_physical(scale).to_i32_round();

        #[cfg(feature = "debug")]
        let mut renderdoc = state.renderdoc.as_mut();
        let render_res = backend.bind().and_then(|_| {
            #[cfg(feature = "debug")]
            if let Some(renderdoc) = renderdoc.as_mut() {
                renderdoc.start_frame_capture(
                    backend.renderer().egl_context().get_context_handle(),
                    backend
                        .window()
                        .window_handle()
                        .map(|handle| {
                            if let RawWindowHandle::Wayland(handle) = handle.as_raw() {
                                handle.surface.as_ptr()
                            } else {
                                std::ptr::null_mut()
                            }
                        })
                        .unwrap_or_else(|_| std::ptr::null_mut()),
                );
            }
            let age = if *full_redraw > 0 {
                0
            } else {
                backend.buffer_age().unwrap_or(0)
            };

            let renderer = backend.renderer();

            let mut elements = Vec::<CustomRenderElements<GlesRenderer>>::new();

            elements.extend(winit_data.pointer_element.render_elements(
                renderer,
                cursor_pos_scaled,
                scale,
                1.0,
            ));

            // draw the dnd icon if any
            if let Some(surface) = dnd_icon {
                if surface.alive() {
                    elements.extend(AsRenderElements::<GlesRenderer>::render_elements(
                        &smithay::desktop::space::SurfaceTree::from_surface(surface),
                        renderer,
                        cursor_pos_scaled,
                        scale,
                        1.0,
                    ));
                }
            }

            #[cfg(feature = "debug")]
            elements.push(CustomRenderElements::Fps(fps_element.clone()));

            crate::render::render_output(
                &output,
                space,
                elements,
                renderer,
                damage_tracker,
                age,
                show_window_preview,
            )
            .map_err(|err| match err {
                OutputDamageTrackerError::Rendering(err) => err.into(),
                _ => unreachable!(),
            })
        });

        match render_res {
            Ok(render_output_result) => {
                let has_rendered = render_output_result.damage.is_some();
                if let Some(damage) = render_output_result.damage {
                    if let Err(err) = backend.submit(Some(&*damage)) {
                        warn!("Failed to submit buffer: {}", err);
                    }
                }
                #[cfg(feature = "debug")]
                if let Some(renderdoc) = renderdoc.as_mut() {
                    renderdoc.end_frame_capture(
                        backend.renderer().egl_context().get_context_handle(),
                        backend
                            .window()
                            .window_handle()
                            .map(|handle| {
                                if let RawWindowHandle::Wayland(handle) = handle.as_raw() {
                                    handle.surface.as_ptr()
                                } else {
                                    std::ptr::null_mut()
                                }
                            })
                            .unwrap_or_else(|_| std::ptr::null_mut()),
                    );
                }
                backend.window().set_cursor_visible(cursor_visible);

                // Send frame events so that client start drawing their next frame
                let time = state.clock.now();
                post_repaint(
                    &output,
                    &render_output_result.states,
                    &state.space,
                    None,
                    time,
                );

                if has_rendered {
                    let mut output_presentation_feedback = take_presentation_feedback(
                        &output,
                        &state.space,
                        &render_output_result.states,
                    );
                    output_presentation_feedback.presented(
                        time,
                        output
                            .current_mode()
                            .map(|mode| Duration::from_secs_f64(1_000f64 / mode.refresh as f64))
                            .unwrap_or_default(),
                        0,
                        wp_presentation_feedback::Kind::Vsync,
                    )
                }
            }
            Err(SwapBuffersError::ContextLost(err)) => {
                #[cfg(feature = "debug")]
                if let Some(renderdoc) = renderdoc.as_mut() {
                    renderdoc.discard_frame_capture(
                        backend.renderer().egl_context().get_context_handle(),
                        backend
                            .window()
                            .window_handle()
                            .map(|handle| {
                                if let RawWindowHandle::Wayland(handle) = handle.as_raw() {
                                    handle.surface.as_ptr()
                                } else {
                                    std::ptr::null_mut()
                                }
                            })
                            .unwrap_or_else(|_| std::ptr::null_mut()),
                    );
                }
                error!("Critical Rendering Error: {}", err);
                state.loop_signal.stop();
            }
            Err(err) => warn!("Rendering error: {}", err),
        }
    }

    #[cfg(feature = "debug")]
    winit_data.fps.tick();

    #[cfg(feature = "profiling")]
    profiling::finish_frame!();
}
