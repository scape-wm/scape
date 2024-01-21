use crate::{
    cursor::Cursor, focus::FocusTarget, shell::WindowElement, udev::UdevData, winit::WinitData,
};
use anyhow::{anyhow, Result};
use calloop::{EventLoop, LoopSignal};
use smithay::backend::drm::DrmNode;
use smithay::delegate_data_control;
use smithay::desktop::space::SpaceElement;
use smithay::input::keyboard::Keysym;
use smithay::wayland::dmabuf::ImportNotifier;
use smithay::wayland::selection::primary_selection::set_primary_focus;
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::selection::wlr_data_control::{DataControlHandler, DataControlState};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};
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
    delegate_compositor, delegate_data_device, delegate_dmabuf, delegate_fractional_scale,
    delegate_input_method_manager, delegate_keyboard_shortcuts_inhibit, delegate_layer_shell,
    delegate_output, delegate_pointer_constraints, delegate_pointer_gestures,
    delegate_presentation, delegate_primary_selection, delegate_relative_pointer, delegate_seat,
    delegate_security_context, delegate_shm, delegate_tablet_manager, delegate_text_input_manager,
    delegate_viewporter, delegate_virtual_keyboard_manager, delegate_xdg_activation,
    delegate_xdg_decoration, delegate_xdg_shell, delegate_xwayland_keyboard_grab,
    desktop::{
        utils::{
            surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
            update_surface_primary_scanout_output, OutputPresentationFeedback,
        },
        PopupKind, PopupManager, Space,
    },
    input::{
        keyboard::XkbConfig,
        pointer::{CursorImageStatus, PointerHandle},
        Seat, SeatHandler, SeatState,
    },
    output::Output,
    reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction},
        wayland_protocols::xdg::decoration::{
            self as xdg_decoration,
            zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
        },
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{
                wl_data_source::WlDataSource,
                wl_surface::{self, WlSurface},
            },
            Display, DisplayHandle, Resource,
        },
    },
    utils::{Clock, Monotonic, Point, Rectangle, Size},
    wayland::{
        compositor::{get_parent, with_states, CompositorClientState, CompositorState},
        dmabuf::{DmabufFeedback, DmabufGlobal, DmabufHandler, DmabufState},
        fractional_scale::{
            with_fractional_scale, FractionalScaleHandler, FractionalScaleManagerState,
        },
        input_method::{InputMethodHandler, InputMethodManagerState, PopupSurface},
        keyboard_shortcuts_inhibit::{
            KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState,
            KeyboardShortcutsInhibitor,
        },
        output::OutputManagerState,
        pointer_constraints::{
            with_pointer_constraint, PointerConstraintsHandler, PointerConstraintsState,
        },
        pointer_gestures::PointerGesturesState,
        presentation::PresentationState,
        relative_pointer::RelativePointerManagerState,
        seat::WaylandFocus,
        security_context::{
            SecurityContext, SecurityContextHandler, SecurityContextListenerSource,
            SecurityContextState,
        },
        selection::data_device::{
            set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState,
            ServerDndGrabHandler,
        },
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::{
                decoration::{XdgDecorationHandler, XdgDecorationState},
                ToplevelSurface, XdgShellState, XdgToplevelSurfaceData,
            },
        },
        shm::{ShmHandler, ShmState},
        socket::ListeningSocketSource,
        tablet_manager::TabletSeatTrait,
        text_input::TextInputManagerState,
        viewporter::ViewporterState,
        virtual_keyboard::VirtualKeyboardManagerState,
        xdg_activation::{
            XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
        },
        xwayland_keyboard_grab::{XWaylandKeyboardGrabHandler, XWaylandKeyboardGrabState},
    },
    xwayland::{X11Wm, XWayland, XWaylandEvent},
};
use std::{
    os::unix::io::OwnedFd,
    sync::{Arc, Mutex},
    time::Duration,
};
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
    pub space: Space<WindowElement>,
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

delegate_compositor!(State);

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl ClientDndGrabHandler for State {
    fn started(
        &mut self,
        _source: Option<WlDataSource>,
        icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        self.dnd_icon = icon;
    }

    fn dropped(&mut self, _seat: Seat<Self>) {
        self.dnd_icon = None;
    }
}
impl ServerDndGrabHandler for State {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {
        unreachable!("Anvil doesn't do server-side grabs");
    }
}
delegate_data_device!(State);

delegate_output!(State);

impl SelectionHandler for State {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(err) = xwm.new_selection(ty, source.map(|source| source.mime_types())) {
                warn!(?err, ?ty, "Failed to set Xwayland selection");
            }
        }
    }

    fn send_selection(
        &mut self,
        ty: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        _user_data: &(),
    ) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(err) = xwm.send_selection(ty, mime_type, fd, self.loop_handle.clone()) {
                warn!(?err, "Failed to send primary (X11 -> Wayland)");
            }
        }
    }
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}
delegate_primary_selection!(State);

impl DataControlHandler for State {
    fn data_control_state(&self) -> &DataControlState {
        &self.data_control_state
    }
}
delegate_data_control!(State);

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}
delegate_shm!(State);

impl SeatHandler for State {
    type KeyboardFocus = FocusTarget;
    type PointerFocus = FocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, target: Option<&FocusTarget>) {
        let dh = &self.display_handle;

        let focus = target
            .and_then(WaylandFocus::wl_surface)
            .and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, focus.clone());
        set_primary_focus(dh, seat, focus);
    }
    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }
}
delegate_seat!(State);

delegate_tablet_manager!(State);

delegate_text_input_manager!(State);

impl InputMethodHandler for State {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(err) = self.popups.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {}", err);
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|parent| parent.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, smithay::utils::Logical> {
        self.space
            .elements()
            .find_map(|window| {
                (window.wl_surface().as_ref() == Some(parent)).then(|| window.geometry())
            })
            .unwrap_or_default()
    }
}
delegate_input_method_manager!(State);

impl KeyboardShortcutsInhibitHandler for State {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.keyboard_shortcuts_inhibit_state
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        // Just grant the wish for everyone
        inhibitor.activate();
    }
}

delegate_keyboard_shortcuts_inhibit!(State);

delegate_virtual_keyboard_manager!(State);

delegate_pointer_gestures!(State);

delegate_relative_pointer!(State);

impl PointerConstraintsHandler for State {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>) {
        warn!("pointer constraint {:?} ", surface);
        // XXX region
        if pointer
            .current_focus()
            .and_then(|x| x.wl_surface())
            .as_ref()
            == Some(surface)
        {
            warn!("ok");
            with_pointer_constraint(surface, pointer, |constraint| {
                constraint.unwrap().activate();
            });
        }
        warn!("done")
    }
}
delegate_pointer_constraints!(State);

delegate_viewporter!(State);

impl XdgActivationHandler for State {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        if let Some((serial, seat)) = data.serial {
            let keyboard = self.seat.get_keyboard().unwrap();
            Seat::from_resource(&seat) == Some(self.seat.clone())
                && keyboard
                    .last_enter()
                    .map(|last_enter| serial.is_no_older_than(&last_enter))
                    .unwrap_or(false)
        } else {
            false
        }
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        if token_data.timestamp.elapsed().as_secs() < 10 {
            // Just grant the wish
            let w = self
                .space
                .elements()
                .find(|window| window.wl_surface().map(|s| s == surface).unwrap_or(false))
                .cloned();
            if let Some(window) = w {
                self.space.raise_element(&window, true);
            }
        }
    }
}
delegate_xdg_activation!(State);

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        use xdg_decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ClientSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        use xdg_decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(match mode {
                DecorationMode::ServerSide => Mode::ServerSide,
                _ => Mode::ClientSide,
            });
        });

        let initial_configure_sent = with_states(toplevel.wl_surface(), |states| {
            if let Ok(data) = states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .try_lock()
            {
                data.initial_configure_sent
            } else {
                warn!("Unable to lock XdgToplevelSurfaceData in request mode");
                true
            }
        });
        if initial_configure_sent {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        use xdg_decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ClientSide);
        });
        let initial_configure_sent = with_states(toplevel.wl_surface(), |states| {
            if let Ok(data) = states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .try_lock()
            {
                data.initial_configure_sent
            } else {
                warn!("Unable to lock XdgToplevelSurfaceData in unset mode");
                true
            }
        });
        if initial_configure_sent {
            toplevel.send_pending_configure();
        }
    }
}
delegate_xdg_decoration!(State);

delegate_xdg_shell!(State);
delegate_layer_shell!(State);
delegate_presentation!(State);

impl FractionalScaleHandler for State {
    fn new_fractional_scale(
        &mut self,
        surface: smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) {
        // Here we can set the initial fractional scale
        //
        // First we look if the surface already has a primary scan-out output, if not
        // we test if the surface is a subsurface and try to use the primary scan-out output
        // of the root surface. If the root also has no primary scan-out output we just try
        // to use the first output of the toplevel.
        // If the surface is the root we also try to use the first output of the toplevel.
        //
        // If all the above tests do not lead to a output we just use the first output
        // of the space (which in case of anvil will also be the output a toplevel will
        // initially be placed on)
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }

        with_states(&surface, |states| {
            let primary_scanout_output = surface_primary_scanout_output(&surface, states)
                .or_else(|| {
                    if root != surface {
                        with_states(&root, |states| {
                            surface_primary_scanout_output(&root, states).or_else(|| {
                                self.window_for_surface(&root).and_then(|window| {
                                    self.space.outputs_for_element(&window).first().cloned()
                                })
                            })
                        })
                    } else {
                        self.window_for_surface(&root).and_then(|window| {
                            self.space.outputs_for_element(&window).first().cloned()
                        })
                    }
                })
                .or_else(|| self.space.outputs().next().cloned());
            if let Some(output) = primary_scanout_output {
                with_fractional_scale(states, |fractional_scale| {
                    fractional_scale.set_preferred_scale(output.current_scale().fractional_scale());
                });
            }
        });
    }
}
delegate_fractional_scale!(State);

impl SecurityContextHandler for State {
    fn context_created(
        &mut self,
        source: SecurityContextListenerSource,
        security_context: SecurityContext,
    ) {
        self.loop_handle
            .insert_source(source, move |client_stream, _, state| {
                let client_state = ClientState {
                    security_context: Some(security_context.clone()),
                    ..ClientState::default()
                };
                if let Err(err) = state
                    .display_handle
                    .insert_client(client_stream, Arc::new(client_state))
                {
                    warn!("Error adding wayland client: {}", err);
                };
            })
            .expect("Failed to init wayland socket source");
    }
}
delegate_security_context!(State);

impl XWaylandKeyboardGrabHandler for State {
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<FocusTarget> {
        let elem = self
            .space
            .elements()
            .find(|elem| elem.wl_surface().as_ref() == Some(surface))?;
        Some(FocusTarget::Window(elem.clone()))
    }
}
delegate_xwayland_keyboard_grab!(State);

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
    space: &Space<WindowElement>,
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

#[cfg_attr(feature = "profiling", profiling::function)]
pub fn take_presentation_feedback(
    output: &Output,
    space: &Space<WindowElement>,
    render_element_states: &RenderElementStates,
) -> OutputPresentationFeedback {
    let mut output_presentation_feedback = OutputPresentationFeedback::new(output);

    space.elements().for_each(|window| {
        if space.outputs_for_element(window).contains(output) {
            window.take_presentation_feedback(
                &mut output_presentation_feedback,
                surface_primary_scanout_output,
                |surface, _| {
                    surface_presentation_feedback_flags_from_states(surface, render_element_states)
                },
            );
        }
    });
    let map = smithay::desktop::layer_map_for_output(output);
    for layer_surface in map.layers() {
        layer_surface.take_presentation_feedback(
            &mut output_presentation_feedback,
            surface_primary_scanout_output,
            |surface, _| {
                surface_presentation_feedback_flags_from_states(surface, render_element_states)
            },
        );
    }

    output_presentation_feedback
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

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        self.backend_data.dmabuf_state()
    }

    fn dmabuf_imported(&mut self, global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
        self.backend_data.dmabuf_imported(global, dmabuf, notifier)
    }
}

delegate_dmabuf!(State);
