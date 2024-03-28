use crate::application_window::ApplicationWindow;
use crate::composition::Zone;
use crate::config::Config;
use crate::input_handler::Mods;
use crate::udev::{schedule_initial_render, UdevOutputId};
use crate::xwayland::XWaylandState;
use crate::{udev::UdevData, winit::WinitData};
use anyhow::{anyhow, Result};
use calloop::generic::Generic;
use calloop::{EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction};
use mlua::Function as LuaFunction;
use smithay::backend::drm::DrmNode;
use smithay::input::keyboard::{Keysym, LedState};
use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1;
use smithay::utils::Logical;
use smithay::wayland::dmabuf::ImportNotifier;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::selection::wlr_data_control::DataControlState;
use smithay::wayland::session_lock::LockSurface;
use smithay::wayland::session_lock::SessionLockManagerState;
use smithay::wayland::tablet_manager::TabletManagerState;
use smithay::wayland::xdg_foreign::XdgForeignState;
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
    reexports::wayland_server::{
        backend::{ClientData, ClientId, DisconnectReason},
        protocol::wl_surface::{self, WlSurface},
        Display, DisplayHandle,
    },
    utils::{Clock, Monotonic, Point},
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
    },
};
use std::collections::{HashMap, HashSet};
use std::{sync::Arc, time::Duration};
use tracing::{error, info, warn};

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

#[derive(Debug, Default)]
pub struct ReadyState {
    backend_ready: bool,
    xwayland_ready: bool,
    on_ready_called: bool,
}

#[derive(Debug)]
pub struct SessionLock {
    pub ext_session_lock: ExtSessionLockV1,
    pub surfaces: HashMap<Output, LockSurface>,
}

#[derive(Debug)]
pub struct ActiveSpace(pub String);

#[derive(Debug)]
pub struct State {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Self>,
    pub loop_signal: LoopSignal,

    pub backend_data: BackendData,

    // desktop
    pub popups: PopupManager,
    pub outputs: HashMap<String, Output>,
    pub spaces: HashMap<String, Space<ApplicationWindow>>,
    pub started_outputs: HashSet<Output>,
    pub zones: HashMap<String, Zone>,
    pub default_zone: Option<String>,

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
    pub session_lock_state: SessionLockManagerState,
    pub xdg_foreign_state: XdgForeignState,
    pub session_lock: Option<SessionLock>,

    pub dnd_icon: Option<WlSurface>,

    // input-related fields
    pub suppressed_keys: Vec<Keysym>,
    pub cursor_status: CursorImageStatus,
    pub seat: Option<Seat<State>>,
    pub clock: Clock<Monotonic>,
    pub pointer: Option<PointerHandle<State>>,

    pub xwayland_state: Option<XWaylandState>,

    #[cfg(feature = "debug")]
    pub renderdoc: Option<renderdoc::RenderDoc<renderdoc::V141>>,

    pub show_window_preview: bool,
    pub session_paused: bool,
    pub last_node: Option<DrmNode>,

    pub config: Config,

    pub socket_name: Option<String>,

    pub ready_state: ReadyState,

    pub key_maps: HashMap<Mods, HashMap<Keysym, LuaFunction<'static>>>,
    pub tab_index: usize,
}

impl State {
    pub fn stop_loop(&mut self) {
        info!("Stopping loop");
        self.loop_signal.stop();
    }
}

impl State {
    pub fn new(
        display: &Display<State>,
        event_loop: &mut EventLoop<'static, State>,
    ) -> anyhow::Result<State> {
        let display_handle = display.handle();
        let loop_handle = event_loop.handle();
        let loop_signal = event_loop.get_signal();

        let clock = Clock::new();

        // init globals
        let compositor_state = CompositorState::new::<Self>(&display_handle);
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&display_handle);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display_handle);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&display_handle);
        let data_control_state = DataControlState::new::<Self, _>(
            &display_handle,
            Some(&primary_selection_state),
            |_| true,
        );
        let seat_state = SeatState::new();
        let shm_state = ShmState::new::<Self>(&display_handle, vec![]);
        let viewporter_state = ViewporterState::new::<Self>(&display_handle);
        let xdg_activation_state = XdgActivationState::new::<Self>(&display_handle);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&display_handle);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let presentation_state = PresentationState::new::<Self>(&display_handle, clock.id() as u32);
        let fractional_scale_manager_state =
            FractionalScaleManagerState::new::<Self>(&display_handle);
        let xdg_foreign_state = XdgForeignState::new::<Self>(&display_handle);
        let _text_input_manager_state = TextInputManagerState::new::<Self>(&display_handle);
        let _input_method_manager_state =
            InputMethodManagerState::new::<Self, _>(&display_handle, |_client| {
                // TODO: implement filtering based on the client
                true
            });
        let _virtual_keyboard_manager_state =
            VirtualKeyboardManagerState::new::<Self, _>(&display_handle, |_client| true);
        let _relative_pointer_manager_state =
            RelativePointerManagerState::new::<Self>(&display_handle);
        PointerConstraintsState::new::<Self>(&display_handle);
        let _pointer_gestures_state = PointerGesturesState::new::<Self>(&display_handle);
        let _tablet_manager_state = TabletManagerState::new::<Self>(&display_handle);
        SecurityContextState::new::<Self, _>(&display_handle, |client| {
            client
                .get_data::<ClientState>()
                .map_or(true, |client_state| client_state.security_context.is_none())
        });

        let keyboard_shortcuts_inhibit_state =
            KeyboardShortcutsInhibitState::new::<Self>(&display_handle);

        // TODO: implement filtering based on the client
        let session_lock_state = SessionLockManagerState::new::<Self, _>(&display_handle, |_| true);

        let cursor_status = CursorImageStatus::default_named();

        Ok(State {
            display_handle,
            loop_handle,
            loop_signal,
            backend_data: BackendData::None,
            popups: PopupManager::default(),
            compositor_state,
            data_device_state,
            layer_shell_state,
            output_manager_state,
            primary_selection_state,
            data_control_state,
            seat_state,
            keyboard_shortcuts_inhibit_state,
            session_lock_state,
            session_lock: None,
            shm_state,
            viewporter_state,
            xdg_activation_state,
            xdg_decoration_state,
            xdg_shell_state,
            presentation_state,
            fractional_scale_manager_state,
            xdg_foreign_state,
            dnd_icon: None,
            suppressed_keys: Vec::new(),
            cursor_status,
            seat: None,
            pointer: None,
            clock,
            xwayland_state: None,
            #[cfg(feature = "debug")]
            renderdoc: renderdoc::RenderDoc::new().ok(),
            show_window_preview: false,
            session_paused: false,
            last_node: None,
            config: Config::new(),
            socket_name: None,
            ready_state: ReadyState::default(),
            outputs: HashMap::new(),
            started_outputs: HashSet::new(),
            spaces: {
                let mut spaces = HashMap::new();
                spaces.insert(String::from("main"), Space::default());
                spaces
            },
            zones: HashMap::new(),
            default_zone: None,
            key_maps: HashMap::new(),
            tab_index: 0,
        })
    }

    pub fn init(
        &mut self,
        display: Display<State>,
        backend_data: BackendData,
    ) -> anyhow::Result<()> {
        info!("Initializing state");

        // init wayland clients
        let source = ListeningSocketSource::new_auto()?;
        let socket_name = source.socket_name().to_string_lossy().into_owned();
        self.loop_handle
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
        self.socket_name = Some(socket_name);

        self.loop_handle
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

        // init input
        let seat_name = backend_data.seat_name();
        let mut seat = self
            .seat_state
            .new_wl_seat(&self.display_handle, seat_name.clone());

        let pointer = seat.add_pointer();
        seat.add_keyboard(
            XkbConfig {
                layout: "de",
                ..Default::default()
            },
            400,
            20,
        )
        .expect("Failed to initialize the keyboard");

        self.seat = Some(seat);
        self.pointer = Some(pointer);

        // TODO: add tablet to seat and handle cursor event
        // let cursor_status2 = cursor_status.clone();
        // seat.tablet_seat()
        //     .on_cursor_surface(move |_tool, new_status| {
        //         // TODO: tablet tools should have their own cursors
        //         *cursor_status2.lock().unwrap() = new_status;
        //     });

        self.backend_data = backend_data;

        if let Err(e) = self.start_xwayland() {
            error!(err = %e, "Failed to start XWayland");
        }

        Ok(())
    }

    pub fn pointer_location(&self) -> Point<f64, Logical> {
        self.pointer.as_ref().unwrap().current_location()
    }

    pub fn check_readyness(&mut self) {
        if !self.ready_state.on_ready_called
            && self.ready_state.backend_ready
            && self.ready_state.xwayland_ready
        {
            self.ready_state.on_ready_called = true;
            self.on_startup();
        }
    }

    pub fn backend_ready(&mut self) {
        self.ready_state.backend_ready = true;
        self.check_readyness();
    }

    pub fn xwayland_ready(&mut self) {
        self.ready_state.xwayland_ready = true;
        self.check_readyness();
    }

    pub fn start_outputs(&mut self) {
        info!("Starting outputs");
        for output in self.outputs.values() {
            if self.started_outputs.contains(output) {
                return;
            }

            self.started_outputs.insert(output.to_owned());
            self.backend_data
                .start_output(output, self.loop_handle.clone());
        }

        self.loop_handle.insert_idle(State::backend_ready);
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
    None,
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
            _ => unreachable!("Requested winit_data, but is not winit backend data"),
        }
    }

    pub fn winit_mut(&mut self) -> &mut WinitData {
        match self {
            BackendData::Winit(winit_data) => winit_data,
            _ => unreachable!("Requested mut winit_data, but is not udev backend data"),
        }
    }

    pub fn seat_name(&self) -> String {
        match self {
            BackendData::Udev(ref udev_data) => udev_data.seat_name(),
            BackendData::Winit(ref winit_data) => winit_data.seat_name(),
            BackendData::None => unreachable!("Requested seat name, but no backend data is set"),
        }
    }

    pub fn reset_buffers(&mut self, output: &Output) {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.reset_buffers(output),
            BackendData::Winit(ref mut winit_data) => winit_data.reset_buffers(output),
            BackendData::None => {
                unreachable!("Requested to reset buffers, but no backend data is set")
            }
        }
    }

    pub fn early_import(&mut self, surface: &wl_surface::WlSurface) {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.early_import(surface),
            BackendData::Winit(ref mut winit_data) => winit_data.early_import(surface),
            BackendData::None => {
                unreachable!("Requested to early import, but no backend data is set")
            }
        }
    }

    pub fn dmabuf_state(&mut self) -> &mut DmabufState {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.dmabuf_state(),
            BackendData::Winit(ref mut winit_data) => winit_data.dmabuf_state(),
            BackendData::None => {
                unreachable!("Requested to get dmabuf state, but no backend data is set")
            }
        }
    }

    pub fn update_led_state(&mut self, led_state: LedState) {
        if let BackendData::Udev(ref mut udev_data) = self {
            udev_data.update_led_state(led_state)
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
            BackendData::None => {
                unreachable!("Requested dmabuf import notifier, but no backend data is set")
            }
        }
    }

    pub fn set_debug_flags(&mut self, flags: DebugFlags) {
        match self {
            BackendData::Udev(ref mut udev_data) => udev_data.set_debug_flags(flags),
            BackendData::Winit(_) => (),
            BackendData::None => {
                unreachable!("Requested set debug flags, but no backend data is set")
            }
        }
    }

    pub fn debug_flags(&self) -> DebugFlags {
        match self {
            BackendData::Udev(ref udev_data) => udev_data.debug_flags(),
            BackendData::Winit(_) => DebugFlags::empty(),
            BackendData::None => {
                unreachable!("Requested to get debug flags, but no backend data is set")
            }
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

    fn start_output(&mut self, output: &Output, loop_handle: LoopHandle<'static, State>) {
        info!(?output, "Starting output");
        match self {
            BackendData::Udev(ref mut udev_data) => {
                let UdevOutputId { device_id, crtc } =
                    output.user_data().get::<UdevOutputId>().unwrap();
                schedule_initial_render(udev_data, *device_id, *crtc, loop_handle);
            }
            _ => {}
        }
    }
}
