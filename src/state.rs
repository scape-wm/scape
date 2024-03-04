use crate::{cursor::Cursor, shell::ApplicationWindow, udev::UdevData, winit::WinitData};
use anyhow::{anyhow, Result};
use calloop::{EventLoop, LoopSignal};
use smithay::backend::drm::DrmNode;
use smithay::input::keyboard::{Keysym, LedState};
use smithay::wayland::dmabuf::ImportNotifier;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::selection::wlr_data_control::DataControlState;
use smithay::wayland::tablet_manager::TabletManagerState;
use smithay::{
    backend::{
        allocator::dmabuf::Dmabuf,
        renderer::{
            element::{
                default_primary_scanout_output_compare, utils::select_dmabuf_feedback,
                RenderElementStates,
            },
            DebugFlags,
        },
        session::Session,
    },
    desktop::{
        utils::{surface_primary_scanout_output, update_surface_primary_scanout_output},
        PopupManager, Space,
    },
    input::{
        keyboard::XkbConfig,
        pointer::{CursorImageStatus, PointerHandle},
        Seat, SeatState,
    },
    output::Output,
    reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::{self, WlSurface},
            Display, DisplayHandle,
        },
    },
    utils::{Clock, Monotonic, Point, Size},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        dmabuf::{DmabufFeedback, DmabufGlobal, DmabufState},
        fractional_scale::{with_fractional_scale, FractionalScaleManagerState},
        input_method::InputMethodManagerState,
        keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState,
        output::OutputManagerState,
        pointer_constraints::PointerConstraintsState,
        pointer_gestures::PointerGesturesState,
        presentation::PresentationState,
        relative_pointer::RelativePointerManagerState,
        security_context::{SecurityContext, SecurityContextState},
        selection::data_device::DataDeviceState,
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::{decoration::XdgDecorationState, XdgShellState},
        },
        shm::ShmState,
        socket::ListeningSocketSource,
        text_input::TextInputManagerState,
        viewporter::ViewporterState,
        virtual_keyboard::VirtualKeyboardManagerState,
        xdg_activation::XdgActivationState,
        xwayland_keyboard_grab::XWaylandKeyboardGrabState,
    },
    xwayland::{X11Wm, XWayland, XWaylandEvent},
};
use std::{sync::Arc, time::Duration};
use tracing::{info, warn};

#[derive(Debug, Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    pub security_context: Option<SecurityContext>,
}

impl ClientData for ClientState {
    /// Notification that a client was initialized
    fn initialized(&self, _client_id: ClientId) {}
    /// Notification that a client is disconnected
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

#[derive(Debug)]
pub struct State {
    pub socket_name: String,
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Self>,
    pub loop_signal: LoopSignal,

    pub backend_data: BackendData,

    // desktop
    pub space: Space<ApplicationWindow>,
    pub popups: PopupManager,

    // smithay state
    pub compositor_state: CompositorState,
    pub data_device_state: DataDeviceState,
    pub layer_shell_state: WlrLayerShellState,
    pub output_manager_state: OutputManagerState,
    pub primary_selection_state: PrimarySelectionState,
    pub data_control_state: DataControlState,
    pub seat_state: SeatState<State>,
    pub keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState,
    pub shm_state: ShmState,
    pub viewporter_state: ViewporterState,
    pub xdg_activation_state: XdgActivationState,
    pub xdg_decoration_state: XdgDecorationState,
    pub xdg_shell_state: XdgShellState,
    pub presentation_state: PresentationState,
    pub fractional_scale_manager_state: FractionalScaleManagerState,

    pub dnd_icon: Option<WlSurface>,

    // input-related fields
    pub suppressed_keys: Vec<Keysym>,
    pub cursor_status: CursorImageStatus,
    pub seat_name: String,
    pub seat: Seat<State>,
    pub clock: Clock<Monotonic>,
    pub pointer: PointerHandle<State>,

    pub xwayland: XWayland,
    pub xwm: Option<X11Wm>,
    pub xdisplay: Option<u32>,

    #[cfg(feature = "debug")]
    pub renderdoc: Option<renderdoc::RenderDoc<renderdoc::V141>>,

    pub show_window_preview: bool,
    pub session_paused: bool,
    pub last_node: Option<DrmNode>,
}

impl State {
    pub fn stop_loop(&mut self) {
        info!("Stopping loop");
        self.loop_signal.stop();
    }
}

impl State {
    pub fn init(
        display: Display<State>,
        backend_data: BackendData,
        event_loop: &mut EventLoop<'static, State>,
    ) -> anyhow::Result<State> {
        info!("Initializing state");
        let clock = Clock::new();
        let loop_handle = event_loop.handle();

        // init wayland clients
        let source = ListeningSocketSource::new_auto()?;
        let socket_name = source.socket_name().to_string_lossy().into_owned();
        loop_handle
            .insert_source(source, |client_stream, _, state| {
                if let Err(err) = state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                {
                    warn!("Error adding wayland client: {}", err);
                };
            })
            .expect("Failed to init wayland socket source");
        info!(socket_name, "Listening on wayland socket");
        ::std::env::set_var("WAYLAND_DISPLAY", &socket_name);

        let dh = display.handle();

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    #[cfg(feature = "profiling")]
                    profiling::scope!("dispatch_clients");

                    // Safety: the display is not dropped
                    unsafe {
                        display.get_mut().dispatch_clients(state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .expect("Failed to init wayland server source");

        // init globals
        let compositor_state = CompositorState::new::<Self>(&dh);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let data_control_state =
            DataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
        let mut seat_state = SeatState::new();
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let viewporter_state = ViewporterState::new::<Self>(&dh);
        let xdg_activation_state = XdgActivationState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let presentation_state = PresentationState::new::<Self>(&dh, clock.id() as u32);
        let fractional_scale_manager_state = FractionalScaleManagerState::new::<Self>(&dh);
        let _text_input_manager_state = TextInputManagerState::new::<Self>(&dh);
        let _input_method_manager_state = InputMethodManagerState::new::<Self, _>(&dh, |_client| {
            // TODO: implement filtering based on the client
            true
        });
        let _virtual_keyboard_manager_state =
            VirtualKeyboardManagerState::new::<Self, _>(&dh, |_client| true);
        let _relative_pointer_manager_state = RelativePointerManagerState::new::<Self>(&dh);
        PointerConstraintsState::new::<Self>(&dh);
        let _pointer_gestures_state = PointerGesturesState::new::<Self>(&dh);
        let _tablet_manager_state = TabletManagerState::new::<Self>(&dh);
        SecurityContextState::new::<Self, _>(&dh, |client| {
            client
                .get_data::<ClientState>()
                .map_or(true, |client_state| client_state.security_context.is_none())
        });

        // init input
        let seat_name = match &backend_data {
            BackendData::Udev(udev_data) => udev_data.seat_name(),
            BackendData::Winit(winit_data) => winit_data.seat_name(),
        };
        let mut seat = seat_state.new_wl_seat(&dh, seat_name.clone());

        let cursor_status = CursorImageStatus::default_named();
        let pointer = seat.add_pointer();
        seat.add_keyboard(
            XkbConfig {
                layout: "de",
                ..Default::default()
            },
            200,
            25,
        )
        .expect("Failed to initialize the keyboard");

        // TODO: add tablet to seat and handle cursor event
        // let cursor_status2 = cursor_status.clone();
        // seat.tablet_seat()
        //     .on_cursor_surface(move |_tool, new_status| {
        //         // TODO: tablet tools should have their own cursors
        //         *cursor_status2.lock().unwrap() = new_status;
        //     });

        let keyboard_shortcuts_inhibit_state = KeyboardShortcutsInhibitState::new::<Self>(&dh);

        let xwayland = {
            XWaylandKeyboardGrabState::new::<Self>(&dh);

            let (xwayland, channel) = XWayland::new(&dh);
            let ret = loop_handle.insert_source(channel, move |event, _, state| match event {
                XWaylandEvent::Ready {
                    connection,
                    client,
                    client_fd: _,
                    display,
                } => {
                    let mut wm = X11Wm::start_wm(
                        state.loop_handle.clone(),
                        state.display_handle.clone(),
                        connection,
                        client,
                    )
                    .expect("Failed to attach X11 Window Manager");
                    let cursor = Cursor::load();
                    let image = cursor.get_image(1, Duration::ZERO);
                    wm.set_cursor(
                        &image.pixels_rgba,
                        Size::from((image.width as u16, image.height as u16)),
                        Point::from((image.xhot as u16, image.yhot as u16)),
                    )
                    .expect("Failed to set xwayland default cursor");
                    state.xwm = Some(wm);
                    state.xdisplay = Some(display);
                }
                XWaylandEvent::Exited => {
                    let _ = state.xwm.take();
                }
            });
            if let Err(e) = ret {
                tracing::error!(
                    "Failed to insert the XWaylandSource into the event loop: {}",
                    e
                );
            }
            xwayland
        };

        let loop_signal = event_loop.get_signal();

        Ok(State {
            display_handle: dh,
            loop_handle,
            loop_signal,
            backend_data,
            socket_name,
            space: Space::default(),
            popups: PopupManager::default(),
            compositor_state,
            data_device_state,
            layer_shell_state,
            output_manager_state,
            primary_selection_state,
            data_control_state,
            seat_state,
            keyboard_shortcuts_inhibit_state,
            shm_state,
            viewporter_state,
            xdg_activation_state,
            xdg_decoration_state,
            xdg_shell_state,
            presentation_state,
            fractional_scale_manager_state,
            dnd_icon: None,
            suppressed_keys: Vec::new(),
            cursor_status,
            seat_name,
            seat,
            pointer,
            clock,
            xwayland,
            xwm: None,
            xdisplay: None,
            #[cfg(feature = "debug")]
            renderdoc: renderdoc::RenderDoc::new().ok(),
            show_window_preview: false,
            session_paused: false,
            last_node: None,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SurfaceDmabufFeedback<'a> {
    pub render_feedback: &'a DmabufFeedback,
    pub scanout_feedback: &'a DmabufFeedback,
}

#[cfg_attr(feature = "profiling", profiling::function)]
pub fn post_repaint(
    output: &Output,
    render_element_states: &RenderElementStates,
    space: &Space<ApplicationWindow>,
    dmabuf_feedback: Option<SurfaceDmabufFeedback<'_>>,
    time: impl Into<Duration>,
) {
    let time = time.into();
    let throttle = Some(Duration::from_secs(1));

    space.elements().for_each(|window| {
        window.with_surfaces(|surface, states| {
            let primary_scanout_output = update_surface_primary_scanout_output(
                surface,
                output,
                states,
                render_element_states,
                default_primary_scanout_output_compare,
            );

            if let Some(output) = primary_scanout_output {
                with_fractional_scale(states, |fraction_scale| {
                    fraction_scale.set_preferred_scale(output.current_scale().fractional_scale());
                });
            }
        });

        if space.outputs_for_element(window).contains(output) {
            window.send_frame(output, time, throttle, surface_primary_scanout_output);
            if let Some(dmabuf_feedback) = dmabuf_feedback {
                window.send_dmabuf_feedback(
                    output,
                    surface_primary_scanout_output,
                    |surface, _| {
                        select_dmabuf_feedback(
                            surface,
                            render_element_states,
                            dmabuf_feedback.render_feedback,
                            dmabuf_feedback.scanout_feedback,
                        )
                    },
                );
            }
        }
    });
    let map = smithay::desktop::layer_map_for_output(output);
    for layer_surface in map.layers() {
        layer_surface.with_surfaces(|surface, states| {
            let primary_scanout_output = update_surface_primary_scanout_output(
                surface,
                output,
                states,
                render_element_states,
                default_primary_scanout_output_compare,
            );

            if let Some(output) = primary_scanout_output {
                with_fractional_scale(states, |fraction_scale| {
                    fraction_scale.set_preferred_scale(output.current_scale().fractional_scale());
                });
            }
        });

        layer_surface.send_frame(output, time, throttle, surface_primary_scanout_output);
        if let Some(dmabuf_feedback) = dmabuf_feedback {
            layer_surface.send_dmabuf_feedback(
                output,
                surface_primary_scanout_output,
                |surface, _| {
                    select_dmabuf_feedback(
                        surface,
                        render_element_states,
                        dmabuf_feedback.render_feedback,
                        dmabuf_feedback.scanout_feedback,
                    )
                },
            );
        }
    }
}

#[derive(Debug)]
pub enum BackendData {
    Udev(UdevData),
    Winit(WinitData),
}

impl BackendData {
    pub fn udev(&self) -> &UdevData {
        match self {
            BackendData::Udev(udev_data) => udev_data,
            _ => unreachable!("Requeted udev_data, but is not udev backend data"),
        }
    }

    pub fn udev_mut(&mut self) -> &mut UdevData {
        match self {
            BackendData::Udev(udev_data) => udev_data,
            _ => unreachable!("Requeted mut udev_data, but is not udev backend data"),
        }
    }

    pub fn winit(&self) -> &WinitData {
        match self {
            BackendData::Winit(winit_data) => winit_data,
            _ => unreachable!("Requeted winit_data, but is not winit backend data"),
        }
    }

    pub fn winit_mut(&mut self) -> &mut WinitData {
        match self {
            BackendData::Winit(winit_data) => winit_data,
            _ => unreachable!("Requeted mut winit_data, but is not udev backend data"),
        }
    }

    pub fn seat_name(&self) -> String {
        match self {
            BackendData::Udev(ref udev_data) => udev_data.seat_name(),
            BackendData::Winit(ref winit_data) => winit_data.seat_name(),
        }
    }

    pub fn reset_buffers(&mut self, output: &Output) {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.reset_buffers(output),
            BackendData::Winit(ref mut winit_data) => winit_data.reset_buffers(output),
        }
    }

    pub fn early_import(&mut self, surface: &wl_surface::WlSurface) {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.early_import(surface),
            BackendData::Winit(ref mut winit_data) => winit_data.early_import(surface),
        }
    }

    pub fn dmabuf_state(&mut self) -> &mut DmabufState {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.dmabuf_state(),
            BackendData::Winit(ref mut winit_data) => winit_data.dmabuf_state(),
        }
    }

    pub fn update_led_state(&mut self, led_state: LedState) {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.update_led_state(led_state),
            _ => {}
        }
    }

    pub fn dmabuf_imported(
        &mut self,
        global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        match self {
            BackendData::Udev(ref mut udev_data) => {
                udev_data.dmabuf_imported(global, dmabuf, notifier)
            }
            BackendData::Winit(ref mut winit_data) => {
                winit_data.dmabuf_imported(global, dmabuf, notifier)
            }
        }
    }

    pub fn set_debug_flags(&mut self, flags: DebugFlags) {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.set_debug_flags(flags),
            BackendData::Winit(_) => (),
        }
    }

    pub fn debug_flags(&self) -> DebugFlags {
        match self {
            BackendData::Udev(ref udev_data) => udev_data.debug_flags(),
            BackendData::Winit(_) => DebugFlags::empty(),
        }
    }

    pub fn switch_vt(&mut self, vt: i32) -> Result<()> {
        match self {
            BackendData::Udev(ref mut udev_data) => {
                udev_data.session.change_vt(vt).map_err(|e| anyhow!(e))
            }
            _ => Ok(()),
        }
    }
}
