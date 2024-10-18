use crate::{focus::PointerFocusTarget, State};
use mlua::Function as LuaFunction;
use scape_shared::Action;
use smithay::backend::input::GestureSwipeUpdateEvent;
use smithay::backend::input::{GesturePinchUpdateEvent, TouchEvent};
use smithay::input::pointer;
use smithay::input::touch::{DownEvent, UpEvent};
use smithay::{
    backend::input::{
        self, AbsolutePositionEvent, Axis, AxisSource, Device, DeviceCapability, Event,
        GestureBeginEvent, GestureEndEvent, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
        PointerAxisEvent, PointerButtonEvent, PointerMotionEvent, ProximityState,
        TabletToolButtonEvent, TabletToolEvent, TabletToolProximityEvent, TabletToolTipEvent,
        TabletToolTipState,
    },
    desktop::{layer_map_for_output, WindowSurfaceType},
    input::{
        keyboard::{keysyms as xkb, FilterResult, Keysym, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
            GesturePinchBeginEvent, GesturePinchEndEvent, GestureSwipeBeginEvent,
            GestureSwipeEndEvent, MotionEvent, RelativeMotionEvent,
        },
    },
    output::Output,
    reexports::wayland_server::{protocol::wl_pointer, DisplayHandle},
    utils::{Logical, Point, Serial, SERIAL_COUNTER as SCOUNTER},
    wayland::{
        compositor::with_states,
        input_method::InputMethodSeat,
        keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitorSeat,
        pointer_constraints::{with_pointer_constraint, PointerConstraint},
        seat::WaylandFocus,
        shell::wlr_layer::{KeyboardInteractivity, Layer as WlrLayer, LayerSurfaceCachedState},
        tablet_manager::{TabletDescriptor, TabletSeatTrait},
    },
};
use std::convert::TryInto;
use tracing::debug;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "shift" key
    pub shift: bool,
    /// The "Caps lock" key
    pub caps_lock: bool,
    /// The "logo" key
    ///
    /// Also known as the "windows" key on most keyboards
    pub logo: bool,
}

impl From<ModifiersState> for Mods {
    fn from(value: ModifiersState) -> Self {
        Self {
            ctrl: value.ctrl,
            alt: value.alt,
            shift: value.shift,
            caps_lock: value.caps_lock,
            logo: value.logo,
        }
    }
}

impl State {
    pub fn map_key(&mut self, key: Keysym, mods: Mods, callback: LuaFunction<'static>) {
        self.key_maps.entry(mods).or_default().insert(key, callback);
    }

    pub fn clear_key_map(&mut self) {
        self.key_maps.clear();
    }

    // fn process_common_key_action(&mut self, action: KeyAction) {
    //     match action {
    //         KeyAction::None => (),
    //
    //         KeyAction::Quit => {
    //             info!("Quitting.");
    //             self.loop_signal.stop();
    //         }
    //
    //         KeyAction::Run(cmd) => {
    //             info!(cmd, "Starting program");
    //
    //             // if let Err(e) = Command::new(&cmd)
    //             //     .envs(
    //             //         Some(self.socket_name.clone())
    //             //             .map(|v| ("WAYLAND_DISPLAY", v))
    //             //             .into_iter()
    //             //             .chain(self.xdisplay.map(|v| ("DISPLAY", format!(":{}", v)))),
    //             //     )
    //             //     .spawn()
    //             // {
    //             //     error!(cmd, err = %e, "Failed to start program");
    //             // }
    //         }
    //
    //         // KeyAction::TogglePreview => {
    //         //     self.show_window_preview = !self.show_window_preview;
    //         // }
    //         //
    //         // KeyAction::ToggleDecorations => {
    //         //     for element in self.space.elements() {
    //         //         #[allow(irrefutable_let_patterns)]
    //         //         if let ApplicationWindow::Wayland(window) = element {
    //         //             let toplevel = window.toplevel();
    //         //             let mode_changed = toplevel.with_pending_state(|state| {
    //         //                 if let Some(current_mode) = state.decoration_mode {
    //         //                     let new_mode = if current_mode
    //         //                         == zxdg_toplevel_decoration_v1::Mode::ClientSide
    //         //                     {
    //         //                         zxdg_toplevel_decoration_v1::Mode::ServerSide
    //         //                     } else {
    //         //                         zxdg_toplevel_decoration_v1::Mode::ClientSide
    //         //                     };
    //         //                     state.decoration_mode = Some(new_mode);
    //         //                     true
    //         //                 } else {
    //         //                     false
    //         //                 }
    //         //             });
    //         //             if mode_changed && toplevel.is_initial_configure_sent() {
    //         //                 toplevel.send_pending_configure();
    //         //             }
    //         //         }
    //         //     }
    //         // }
    //         // KeyAction::MoveWindow(window_position) => {
    //         //     let pointer_location = self.pointer_location();
    //         //     if let Some((window, _)) = self.space.element_under(pointer_location) {
    //         //         let window = window.clone();
    //         //         place_window(
    //         //             &mut self.space,
    //         //             pointer_location,
    //         //             &window,
    //         //             true,
    //         //             window_position,
    //         //         );
    //         //     }
    //         // }
    //         _ => unreachable!(
    //             "Common key action handler encountered backend specific action {:?}",
    //             action
    //         ),
    //     }
    // }

    fn keyboard_key_to_action<B: InputBackend>(
        &mut self,
        evt: B::KeyboardKeyEvent,
    ) -> Option<Action> {
        let space = &self
            .spaces // FIXME: handle multiple spaces
            .iter()
            .next()
            .unwrap()
            .1;

        let keycode = evt.key_code();
        let evt_state = evt.state();
        debug!(?keycode, ?evt_state, "key");
        let serial = SCOUNTER.next_serial();
        let time = Event::time_msec(&evt);
        let mut suppressed_keys = self.suppressed_keys.clone();
        let seat = self.seat.as_ref()?;
        let keyboard = seat.get_keyboard().unwrap();

        for layer in self.layer_shell_state.layer_surfaces().rev() {
            let data = with_states(layer.wl_surface(), |states| {
                *states
                    .cached_state
                    .get::<LayerSurfaceCachedState>()
                    .current()
            });
            if data.keyboard_interactivity == KeyboardInteractivity::Exclusive
                && (data.layer == WlrLayer::Top || data.layer == WlrLayer::Overlay)
            {
                let surface = space.outputs().find_map(|o| {
                    let map = layer_map_for_output(o);
                    let cloned = map.layers().find(|l| l.layer_surface() == &layer).cloned();
                    cloned
                });
                if let Some(surface) = surface {
                    keyboard.set_focus(self, Some(surface.into()), serial);
                    keyboard.input::<(), _>(self, keycode, evt_state, serial, time, |_, _, _| {
                        FilterResult::Forward
                    });
                    return None;
                };
            }
        }

        let inhibited = space
            .element_under(self.pointer_location())
            .and_then(|(window, _)| {
                let surface = window.wl_surface()?;
                self.seat
                    .as_ref()?
                    .keyboard_shortcuts_inhibitor_for_surface(&surface)
            })
            .map(|inhibitor| inhibitor.is_active())
            .unwrap_or(false);

        let action = keyboard.input(
            self,
            keycode,
            evt_state,
            serial,
            time,
            |state, modifiers, handle| {
                let keysym = handle.modified_sym();

                debug!(
                    ?evt_state,
                    mods = ?modifiers,
                    keysym = ::xkbcommon::xkb::keysym_get_name(keysym),
                    "keysym"
                );

                if !modifiers.alt {
                    state.tab_index = 0;
                }

                // If the key is pressed and triggered a action
                // we will not forward the key to the client.
                // Additionally add the key to the suppressed keys
                // so that we can decide on a release if the key
                // should be forwarded to the client or not.
                if let KeyState::Pressed = evt_state {
                    if !inhibited {
                        let action = state.process_keyboard_shortcut(*modifiers, keysym);

                        if action.is_some() {
                            suppressed_keys.push(keysym);
                        }

                        action
                            .map(FilterResult::Intercept)
                            .unwrap_or(FilterResult::Forward)
                    } else {
                        FilterResult::Forward
                    }
                } else {
                    let suppressed = suppressed_keys.contains(&keysym);
                    if suppressed {
                        suppressed_keys.retain(|k| *k != keysym);
                        FilterResult::Intercept(Action::None)
                    } else {
                        FilterResult::Forward
                    }
                }
            },
        );

        self.suppressed_keys = suppressed_keys;
        match action {
            None | Some(Action::None) => None,
            _ => action,
        }
    }

    fn on_pointer_button<B: InputBackend>(&mut self, evt: B::PointerButtonEvent) {
        let serial = SCOUNTER.next_serial();
        let button = evt.button_code();
        let state = wl_pointer::ButtonState::from(evt.state());
        if wl_pointer::ButtonState::Pressed == state {
            self.update_keyboard_focus(self.pointer_location(), serial);
        };
        let Some(pointer) = self.pointer.clone() else {
            return;
        };
        pointer.button(
            self,
            &ButtonEvent {
                button,
                state: state.try_into().unwrap(),
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);
    }

    fn update_keyboard_focus(&mut self, pointer_location: Point<f64, Logical>, serial: Serial) {
        let Some(seat) = &self.seat else {
            return;
        };
        let pointer = seat.get_pointer().unwrap();
        let touch = seat.get_touch();
        let keyboard = seat.get_keyboard().unwrap();
        let input_method = seat.input_method();
        // change the keyboard focus unless the pointer or keyboard is grabbed
        // We test for any matching surface type here but always use the root
        // (in case of a window the toplevel) surface for the focus.
        // So for example if a user clicks on a subsurface or popup the toplevel
        // will receive the keyboard focus. Directly assigning the focus to the
        // matching surface leads to issues with clients dismissing popups and
        // subsurface menus (for example firefox-wayland).
        // see here for a discussion about that issue:
        // https://gitlab.freedesktop.org/wayland/wayland/-/issues/294
        if !pointer.is_grabbed()
            && (!keyboard.is_grabbed() || input_method.keyboard_grabbed())
            && !touch.map(|touch| touch.is_grabbed()).unwrap_or(false)
        {
            let output = self
                .spaces // FIXME: handle multiple spaces
                .iter()
                .next()
                .unwrap()
                .1
                .output_under(pointer_location)
                .next()
                .cloned();
            if let Some(output) = output.as_ref() {
                let output_geo = self
                    .spaces // FIXME: handle multiple spaces
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .output_geometry(output)
                    .unwrap();

                let layers = layer_map_for_output(output);
                if let Some(layer) = layers
                    .layer_under(WlrLayer::Overlay, pointer_location)
                    .or_else(|| layers.layer_under(WlrLayer::Top, pointer_location))
                {
                    if layer.can_receive_keyboard_focus() {
                        if let Some((_, _)) = layer.surface_under(
                            pointer_location
                                - output_geo.loc.to_f64()
                                - layers.layer_geometry(layer).unwrap().loc.to_f64(),
                            WindowSurfaceType::ALL,
                        ) {
                            keyboard.set_focus(self, Some(layer.clone().into()), serial);
                            return;
                        }
                    }
                }
            }

            if let Some((window, _)) = self
                .spaces // FIXME: handle multiple spaces
                .iter()
                .next()
                .unwrap()
                .1
                .element_under(pointer_location)
                .map(|(w, p)| (w.clone(), p))
            {
                if let Some(surface) = window.x11_surface() {
                    // users should not be able to focus override redirect windows
                    if surface.is_override_redirect() {
                        return;
                    }
                }
                self.spaces // FIXME: handle multiple spaces
                    .iter_mut()
                    .next()
                    .unwrap()
                    .1
                    .raise_element(&window, true);
                if let Some(surface) = window.x11_surface() {
                    let Some(ref mut xwayland_state) = &mut self.xwayland_state else {
                        return;
                    };
                    xwayland_state
                        .wm
                        .as_mut()
                        .unwrap()
                        .raise_window(surface)
                        .unwrap();
                }
                keyboard.set_focus(self, Some(window.into()), serial);
                return;
            }

            if let Some(output) = output.as_ref() {
                let output_geo = self
                    .spaces // FIXME: handle multiple spaces
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .output_geometry(output)
                    .unwrap();
                let layers = layer_map_for_output(output);
                if let Some(layer) = layers
                    .layer_under(WlrLayer::Bottom, pointer_location)
                    .or_else(|| layers.layer_under(WlrLayer::Background, pointer_location))
                {
                    if layer.can_receive_keyboard_focus() {
                        if let Some((_, _)) = layer.surface_under(
                            pointer_location
                                - output_geo.loc.to_f64()
                                - layers.layer_geometry(layer).unwrap().loc.to_f64(),
                            WindowSurfaceType::ALL,
                        ) {
                            keyboard.set_focus(self, Some(layer.clone().into()), serial);
                        }
                    }
                }
            };
        }
    }

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
        let space = &self
            .spaces // FIXME: handle multiple spaces
            .iter()
            .next()
            .unwrap()
            .1;
        let output = space.outputs().find(|o| {
            let geometry = space.output_geometry(o).unwrap();
            geometry.contains(pos.to_i32_round())
        })?;
        let output_geo = space.output_geometry(output).unwrap();
        let layers = layer_map_for_output(output);

        let mut under = None;
        if let Some(focus) = layers
            .layer_under(WlrLayer::Overlay, pos)
            .or_else(|| layers.layer_under(WlrLayer::Top, pos))
            .and_then(|layer| {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(surface, loc)| {
                        (
                            PointerFocusTarget::from(surface),
                            loc + layer_loc + output_geo.loc,
                        )
                    })
            })
        {
            under = Some(focus)
        } else if let Some(focus) = space.element_under(pos).and_then(|(window, loc)| {
            window
                .surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)
                .map(|(surface, surf_loc)| (surface, surf_loc + loc))
        }) {
            under = Some(focus);
        } else if let Some(focus) = layers
            .layer_under(WlrLayer::Bottom, pos)
            .or_else(|| layers.layer_under(WlrLayer::Background, pos))
            .and_then(|layer| {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(surface, loc)| {
                        (
                            PointerFocusTarget::from(surface),
                            loc + layer_loc + output_geo.loc,
                        )
                    })
            })
        {
            under = Some(focus)
        };

        under.map(|(s, l)| (s, l.to_f64()))
    }

    fn on_pointer_axis<B: InputBackend>(&mut self, evt: B::PointerAxisEvent) {
        let horizontal_amount = evt.amount(input::Axis::Horizontal).unwrap_or_else(|| {
            evt.amount_v120(input::Axis::Horizontal).unwrap_or(0.0) * 3.0 / 120.
        });
        let vertical_amount = evt
            .amount(input::Axis::Vertical)
            .unwrap_or_else(|| evt.amount_v120(input::Axis::Vertical).unwrap_or(0.0) * 3.0 / 120.);
        let horizontal_amount_discrete = evt.amount_v120(input::Axis::Horizontal);
        let vertical_amount_discrete = evt.amount_v120(input::Axis::Vertical);

        {
            let mut frame = AxisFrame::new(evt.time_msec()).source(evt.source());
            if horizontal_amount != 0.0 {
                frame = frame
                    .relative_direction(Axis::Horizontal, evt.relative_direction(Axis::Horizontal));
                frame = frame.value(Axis::Horizontal, horizontal_amount);
                if let Some(discrete) = horizontal_amount_discrete {
                    frame = frame.v120(Axis::Horizontal, discrete as i32);
                }
            } else if evt.source() == AxisSource::Finger {
                frame = frame.stop(Axis::Horizontal);
            }
            if vertical_amount != 0.0 {
                frame = frame
                    .relative_direction(Axis::Vertical, evt.relative_direction(Axis::Vertical));
                frame = frame.value(Axis::Vertical, vertical_amount);
                if let Some(discrete) = vertical_amount_discrete {
                    frame = frame.v120(Axis::Vertical, discrete as i32);
                }
            } else if evt.source() == AxisSource::Finger {
                frame = frame.stop(Axis::Vertical);
            }
            if evt.source() == AxisSource::Finger {
                if evt.amount(Axis::Horizontal) == Some(0.0) {
                    frame = frame.stop(Axis::Horizontal);
                }
                if evt.amount(Axis::Vertical) == Some(0.0) {
                    frame = frame.stop(Axis::Vertical);
                }
            }
            let pointer = self.pointer.clone().unwrap();
            pointer.axis(self, frame);
            pointer.frame(self);
        }
    }
}

impl State {
    pub fn process_input_event_windowed<B: InputBackend>(
        &mut self,
        dh: &DisplayHandle,
        event: InputEvent<B>,
        output_name: &str,
    ) {
        match event {
            InputEvent::Keyboard { event } => {
                if let Some(action) = self.keyboard_key_to_action::<B>(event) {
                    self.execute(action)
                }
            }

            InputEvent::PointerMotionAbsolute { event } => {
                let output = self
                    .outputs
                    .values()
                    .find(|o| o.name() == output_name)
                    .unwrap()
                    .clone();
                self.on_pointer_move_absolute_windowed::<B>(dh, event, &output)
            }
            InputEvent::PointerButton { event } => self.on_pointer_button::<B>(event),
            InputEvent::PointerAxis { event } => self.on_pointer_axis::<B>(event),
            _ => (), // other events are not handled in anvil (yet)
        }
    }

    fn on_pointer_move_absolute_windowed<B: InputBackend>(
        &mut self,
        _dh: &DisplayHandle,
        evt: B::PointerMotionAbsoluteEvent,
        output: &Output,
    ) {
        let space = &self
            .spaces // FIXME: handle multiple spaces
            .iter()
            .next()
            .unwrap()
            .1;
        let output_geo = space.output_geometry(output).unwrap();

        let pos = evt.position_transformed(output_geo.size) + output_geo.loc.to_f64();
        let serial = SCOUNTER.next_serial();

        let pointer = self.pointer.clone().unwrap();
        let under = self.surface_under(pos);
        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: pos,
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);
    }

    pub fn release_all_keys(&mut self) {
        let keyboard = self.seat.as_ref().unwrap().get_keyboard().unwrap();
        for keycode in keyboard.pressed_keys() {
            keyboard.input(
                self,
                keycode,
                KeyState::Released,
                SCOUNTER.next_serial(),
                self.clock.now().as_millis(),
                |_, _, _| FilterResult::Forward::<bool>,
            );
        }
    }
}

impl State {
    pub fn process_input_event<B: InputBackend>(&mut self, event: InputEvent<B>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                if let Some(action) = self.keyboard_key_to_action::<B>(event) {
                    self.execute(action)
                }
            }
            InputEvent::PointerMotion { event, .. } => self.on_pointer_move::<B>(event),
            InputEvent::PointerMotionAbsolute { event, .. } => {
                self.on_pointer_move_absolute::<B>(event)
            }
            InputEvent::PointerButton { event, .. } => self.on_pointer_button::<B>(event),
            InputEvent::PointerAxis { event, .. } => self.on_pointer_axis::<B>(event),
            InputEvent::TabletToolAxis { event, .. } => self.on_tablet_tool_axis::<B>(event),
            InputEvent::TabletToolProximity { event, .. } => {
                self.on_tablet_tool_proximity::<B>(event)
            }
            InputEvent::TabletToolTip { event, .. } => self.on_tablet_tool_tip::<B>(event),
            InputEvent::TabletToolButton { event, .. } => self.on_tablet_button::<B>(event),
            InputEvent::GestureSwipeBegin { event, .. } => self.on_gesture_swipe_begin::<B>(event),
            InputEvent::GestureSwipeUpdate { event, .. } => {
                self.on_gesture_swipe_update::<B>(event)
            }
            InputEvent::GestureSwipeEnd { event, .. } => self.on_gesture_swipe_end::<B>(event),
            InputEvent::GesturePinchBegin { event, .. } => self.on_gesture_pinch_begin::<B>(event),
            InputEvent::GesturePinchUpdate { event, .. } => {
                self.on_gesture_pinch_update::<B>(event)
            }
            InputEvent::GesturePinchEnd { event, .. } => self.on_gesture_pinch_end::<B>(event),
            InputEvent::GestureHoldBegin { event, .. } => self.on_gesture_hold_begin::<B>(event),
            InputEvent::GestureHoldEnd { event, .. } => self.on_gesture_hold_end::<B>(event),

            InputEvent::TouchDown { event } => self.on_touch_down::<B>(event),
            InputEvent::TouchUp { event } => self.on_touch_up::<B>(event),
            InputEvent::TouchMotion { event } => self.on_touch_motion::<B>(event),
            InputEvent::TouchFrame { event } => self.on_touch_frame::<B>(event),
            InputEvent::TouchCancel { event } => self.on_touch_cancel::<B>(event),

            InputEvent::DeviceAdded { device } => {
                if device.has_capability(DeviceCapability::TabletTool) {
                    self.seat
                        .as_ref()
                        .unwrap()
                        .tablet_seat()
                        .add_tablet::<Self>(&self.display_handle, &TabletDescriptor::from(&device));
                }
                if device.has_capability(DeviceCapability::Touch)
                    && self.seat.as_ref().unwrap().get_touch().is_none()
                {
                    self.seat.as_mut().unwrap().add_touch();
                }
            }
            InputEvent::DeviceRemoved { device } => {
                if device.has_capability(DeviceCapability::TabletTool) {
                    let tablet_seat = self.seat.as_ref().unwrap().tablet_seat();

                    tablet_seat.remove_tablet(&TabletDescriptor::from(&device));

                    // If there are no tablets in seat we can remove all tools
                    if tablet_seat.count_tablets() == 0 {
                        tablet_seat.clear_tools();
                    }
                }
            }
            _ => {
                // other events are not handled in anvil (yet)
            }
        }
    }

    fn on_pointer_move<B: InputBackend>(&mut self, evt: B::PointerMotionEvent) {
        // TODO: Can we do this better?
        self.backend_data.schedule_render();

        let mut pointer_location = self.pointer_location();

        let serial = SCOUNTER.next_serial();

        let pointer = self.pointer.clone().unwrap();
        let under = self.surface_under(pointer_location);

        let mut pointer_locked = false;
        let mut pointer_confined = false;
        let mut confine_region = None;
        if let Some((surface, surface_loc)) = under
            .as_ref()
            .and_then(|(target, l)| Some((target.wl_surface()?, l)))
        {
            with_pointer_constraint(&surface, &pointer, |constraint| match constraint {
                Some(constraint) if constraint.is_active() => {
                    // Constraint does not apply if not within region
                    if !constraint.region().map_or(true, |x| {
                        x.contains((pointer_location - *surface_loc).to_i32_round())
                    }) {
                        return;
                    }
                    match &*constraint {
                        PointerConstraint::Locked(_locked) => {
                            pointer_locked = true;
                        }
                        PointerConstraint::Confined(confine) => {
                            pointer_confined = true;
                            confine_region = confine.region().cloned();
                        }
                    }
                }
                _ => {}
            });
        }

        pointer.relative_motion(
            self,
            under.clone(),
            &RelativeMotionEvent {
                delta: evt.delta(),
                delta_unaccel: evt.delta_unaccel(),
                utime: evt.time(),
            },
        );

        // If pointer is locked, only emit relative motion
        if pointer_locked {
            pointer.frame(self);
            return;
        }

        pointer_location += evt.delta();

        // clamp to screen limits
        // this event is never generated by winit
        pointer_location = self.clamp_coords(pointer_location);

        let new_under = self.surface_under(pointer_location);

        // If confined, don't move pointer if it would go outside surface or region
        if pointer_confined {
            if let Some((surface, surface_loc)) = &under {
                if new_under.as_ref().and_then(|(under, _)| under.wl_surface())
                    != surface.wl_surface()
                {
                    pointer.frame(self);
                    return;
                }
                if let Some(region) = confine_region {
                    if !region.contains((pointer_location - *surface_loc).to_i32_round()) {
                        pointer.frame(self);
                        return;
                    }
                }
            }
        }

        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: pointer_location,
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);

        // If pointer is now in a constraint region, activate it
        // TODO Anywhere else pointer is moved needs to do this
        if let Some((under, surface_location)) =
            new_under.and_then(|(target, loc)| Some((target.wl_surface()?.into_owned(), loc)))
        {
            with_pointer_constraint(&under, &pointer, |constraint| match constraint {
                Some(constraint) if !constraint.is_active() => {
                    let point = (pointer_location - surface_location).to_i32_round();
                    if constraint
                        .region()
                        .map_or(true, |region| region.contains(point))
                    {
                        constraint.activate();
                    }
                }
                _ => {}
            });
        }
    }

    fn on_pointer_move_absolute<B: InputBackend>(&mut self, evt: B::PointerMotionAbsoluteEvent) {
        // TODO: Can we do this better?
        self.backend_data.schedule_render();

        let serial = SCOUNTER.next_serial();

        let space = &self
            .spaces // FIXME: handle multiple spaces
            .iter()
            .next()
            .unwrap()
            .1;

        let max_x = space
            .outputs()
            .fold(0, |acc, o| acc + space.output_geometry(o).unwrap().size.w);

        let max_h_output = space
            .outputs()
            .max_by_key(|o| space.output_geometry(o).unwrap().size.h)
            .unwrap();

        let max_y = space.output_geometry(max_h_output).unwrap().size.h;

        let mut pointer_location = (evt.x_transformed(max_x), evt.y_transformed(max_y)).into();

        // clamp to screen limits
        pointer_location = self.clamp_coords(pointer_location);

        let pointer = self.pointer.clone().unwrap();
        let under = self.surface_under(pointer_location);

        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: pointer_location,
                serial,
                time: evt.time_msec(),
            },
        );
        pointer.frame(self);
    }

    fn on_tablet_tool_axis<B: InputBackend>(&mut self, evt: B::TabletToolAxisEvent) {
        let tablet_seat = self.seat.as_ref().unwrap().tablet_seat();

        let space = &self
            .spaces // FIXME: handle multiple spaces
            .iter()
            .next()
            .unwrap()
            .1;

        let output_geometry = space
            .outputs()
            .next()
            .map(|o| space.output_geometry(o).unwrap());

        if let Some(rect) = output_geometry {
            let pointer_location = evt.position_transformed(rect.size) + rect.loc.to_f64();

            let pointer = self.pointer.clone().unwrap();
            let under = self.surface_under(pointer_location);
            let tablet = tablet_seat.get_tablet(&TabletDescriptor::from(&evt.device()));
            let tool = tablet_seat.get_tool(&evt.tool());

            pointer.motion(
                self,
                under.clone(),
                &MotionEvent {
                    location: pointer_location,
                    serial: SCOUNTER.next_serial(),
                    time: evt.time_msec(),
                },
            );

            if let (Some(tablet), Some(tool)) = (tablet, tool) {
                if evt.pressure_has_changed() {
                    tool.pressure(evt.pressure());
                }
                if evt.distance_has_changed() {
                    tool.distance(evt.distance());
                }
                if evt.tilt_has_changed() {
                    tool.tilt(evt.tilt());
                }
                if evt.slider_has_changed() {
                    tool.slider_position(evt.slider_position());
                }
                if evt.rotation_has_changed() {
                    tool.rotation(evt.rotation());
                }
                if evt.wheel_has_changed() {
                    tool.wheel(evt.wheel_delta(), evt.wheel_delta_discrete());
                }

                tool.motion(
                    pointer_location,
                    under.and_then(|(f, loc)| f.wl_surface().map(|s| (s.into_owned(), loc))),
                    &tablet,
                    SCOUNTER.next_serial(),
                    evt.time_msec(),
                );
            }
            pointer.frame(self);
        }
    }

    fn on_tablet_tool_proximity<B: InputBackend>(&mut self, evt: B::TabletToolProximityEvent) {
        let tablet_seat = self.seat.as_ref().unwrap().tablet_seat();

        let space = &self
            .spaces // FIXME: handle multiple spaces
            .iter()
            .next()
            .unwrap()
            .1;

        let output_geometry = space
            .outputs()
            .next()
            .map(|o| space.output_geometry(o).unwrap());

        if let Some(rect) = output_geometry {
            let tool = evt.tool();
            tablet_seat.add_tool::<Self>(self, &self.display_handle.clone(), &tool);

            let pointer_location = evt.position_transformed(rect.size) + rect.loc.to_f64();

            let pointer = self.pointer.clone().unwrap();
            let under = self.surface_under(pointer_location);
            let tablet = tablet_seat.get_tablet(&TabletDescriptor::from(&evt.device()));
            let tool = tablet_seat.get_tool(&tool);

            pointer.motion(
                self,
                under.clone(),
                &MotionEvent {
                    location: pointer_location,
                    serial: SCOUNTER.next_serial(),
                    time: evt.time_msec(),
                },
            );
            pointer.frame(self);

            if let (Some(under), Some(tablet), Some(tool)) = (
                under.and_then(|(f, loc)| f.wl_surface().map(|s| (s.into_owned(), loc))),
                tablet,
                tool,
            ) {
                match evt.state() {
                    ProximityState::In => tool.proximity_in(
                        pointer_location,
                        under,
                        &tablet,
                        SCOUNTER.next_serial(),
                        evt.time_msec(),
                    ),
                    ProximityState::Out => tool.proximity_out(evt.time_msec()),
                }
            }
        }
    }

    fn on_tablet_tool_tip<B: InputBackend>(&mut self, evt: B::TabletToolTipEvent) {
        let tool = self
            .seat
            .as_ref()
            .unwrap()
            .tablet_seat()
            .get_tool(&evt.tool());

        if let Some(tool) = tool {
            match evt.tip_state() {
                TabletToolTipState::Down => {
                    let serial = SCOUNTER.next_serial();
                    tool.tip_down(serial, evt.time_msec());

                    // change the keyboard focus
                    self.update_keyboard_focus(self.pointer_location(), serial);
                }
                TabletToolTipState::Up => {
                    tool.tip_up(evt.time_msec());
                }
            }
        }
    }

    fn on_tablet_button<B: InputBackend>(&mut self, evt: B::TabletToolButtonEvent) {
        let tool = self
            .seat
            .as_ref()
            .unwrap()
            .tablet_seat()
            .get_tool(&evt.tool());

        if let Some(tool) = tool {
            tool.button(
                evt.button(),
                evt.button_state(),
                SCOUNTER.next_serial(),
                evt.time_msec(),
            );
        }
    }

    fn on_gesture_swipe_begin<B: InputBackend>(&mut self, evt: B::GestureSwipeBeginEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_swipe_begin(
            self,
            &GestureSwipeBeginEvent {
                serial,
                time: evt.time_msec(),
                fingers: evt.fingers(),
            },
        );
    }

    fn on_gesture_swipe_update<B: InputBackend>(&mut self, evt: B::GestureSwipeUpdateEvent) {
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_swipe_update(
            self,
            &pointer::GestureSwipeUpdateEvent {
                time: evt.time_msec(),
                delta: evt.delta(),
            },
        );
    }

    fn on_gesture_swipe_end<B: InputBackend>(&mut self, evt: B::GestureSwipeEndEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_swipe_end(
            self,
            &GestureSwipeEndEvent {
                serial,
                time: evt.time_msec(),
                cancelled: evt.cancelled(),
            },
        );
    }

    fn on_gesture_pinch_begin<B: InputBackend>(&mut self, evt: B::GesturePinchBeginEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_pinch_begin(
            self,
            &GesturePinchBeginEvent {
                serial,
                time: evt.time_msec(),
                fingers: evt.fingers(),
            },
        );
    }

    fn on_gesture_pinch_update<B: InputBackend>(&mut self, evt: B::GesturePinchUpdateEvent) {
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_pinch_update(
            self,
            &pointer::GesturePinchUpdateEvent {
                time: evt.time_msec(),
                delta: evt.delta(),
                scale: evt.scale(),
                rotation: evt.rotation(),
            },
        );
    }

    fn on_gesture_pinch_end<B: InputBackend>(&mut self, evt: B::GesturePinchEndEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_pinch_end(
            self,
            &GesturePinchEndEvent {
                serial,
                time: evt.time_msec(),
                cancelled: evt.cancelled(),
            },
        );
    }

    fn on_gesture_hold_begin<B: InputBackend>(&mut self, evt: B::GestureHoldBeginEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_hold_begin(
            self,
            &GestureHoldBeginEvent {
                serial,
                time: evt.time_msec(),
                fingers: evt.fingers(),
            },
        );
    }

    fn on_gesture_hold_end<B: InputBackend>(&mut self, evt: B::GestureHoldEndEvent) {
        let serial = SCOUNTER.next_serial();
        let pointer = self.pointer.clone().unwrap();
        pointer.gesture_hold_end(
            self,
            &GestureHoldEndEvent {
                serial,
                time: evt.time_msec(),
                cancelled: evt.cancelled(),
            },
        );
    }

    fn touch_location_transformed<B: InputBackend, E: AbsolutePositionEvent<B>>(
        &self,
        evt: &E,
    ) -> Option<Point<f64, Logical>> {
        let output = self
            .outputs
            .values()
            .find(|output| output.name().starts_with("eDP"))
            .or_else(|| self.outputs.values().next());

        let output = output?;

        // TODO: Handle multiple spaces
        let output_geometry = self
            .spaces
            .values()
            .next()
            .unwrap()
            .output_geometry(output)?;

        let transform = output.current_transform();
        let size = transform.invert().transform_size(output_geometry.size);
        Some(
            transform.transform_point_in(evt.position_transformed(size), &size.to_f64())
                + output_geometry.loc.to_f64(),
        )
    }

    fn on_touch_down<B: InputBackend>(&mut self, evt: B::TouchDownEvent) {
        let Some(handle) = self.seat.as_ref().unwrap().get_touch() else {
            return;
        };

        let Some(touch_location) = self.touch_location_transformed(&evt) else {
            return;
        };

        let serial = SCOUNTER.next_serial();
        self.update_keyboard_focus(touch_location, serial);

        let under = self.surface_under(touch_location);
        handle.down(
            self,
            under,
            &DownEvent {
                slot: evt.slot(),
                location: touch_location,
                serial,
                time: evt.time_msec(),
            },
        );
    }
    fn on_touch_up<B: InputBackend>(&mut self, evt: B::TouchUpEvent) {
        let Some(handle) = self.seat.as_ref().unwrap().get_touch() else {
            return;
        };
        let serial = SCOUNTER.next_serial();
        handle.up(
            self,
            &UpEvent {
                slot: evt.slot(),
                serial,
                time: evt.time_msec(),
            },
        )
    }
    fn on_touch_motion<B: InputBackend>(&mut self, evt: B::TouchMotionEvent) {
        let Some(handle) = self.seat.as_ref().unwrap().get_touch() else {
            return;
        };
        let Some(touch_location) = self.touch_location_transformed(&evt) else {
            return;
        };

        let under = self.surface_under(touch_location);
        handle.motion(
            self,
            under,
            &smithay::input::touch::MotionEvent {
                slot: evt.slot(),
                location: touch_location,
                time: evt.time_msec(),
            },
        );
    }
    fn on_touch_frame<B: InputBackend>(&mut self, _evt: B::TouchFrameEvent) {
        let Some(handle) = self.seat.as_ref().unwrap().get_touch() else {
            return;
        };
        handle.frame(self);
    }
    fn on_touch_cancel<B: InputBackend>(&mut self, _evt: B::TouchCancelEvent) {
        let Some(handle) = self.seat.as_ref().unwrap().get_touch() else {
            return;
        };
        handle.cancel(self);
    }

    fn clamp_coords(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        let space = &self
            .spaces // FIXME: handle multiple spaces
            .iter()
            .next()
            .unwrap()
            .1;

        if space.outputs().next().is_none() {
            return pos;
        }

        let (pos_x, pos_y) = pos.into();
        let max_x = space
            .outputs()
            .fold(0, |acc, o| acc + space.output_geometry(o).unwrap().size.w);
        let clamped_x = pos_x.clamp(0.0, max_x as f64);
        let max_y = space
            .outputs()
            .find(|o| {
                let geo = space.output_geometry(o).unwrap();
                geo.contains((clamped_x as i32, 0))
            })
            .map(|o| space.output_geometry(o).unwrap().size.h);

        if let Some(max_y) = max_y {
            let clamped_y = pos_y.clamp(0.0, max_y as f64);
            (clamped_x, clamped_y).into()
        } else {
            (clamped_x, pos_y).into()
        }
    }
}

/// Possible results of a keyboard action
// #[derive(Debug)]
// enum KeyAction {
//     /// Quit the compositor
//     Quit,
//     /// Trigger a vt-switch
//     VtSwitch(i32),
//     /// run a command
//     Run(String),
//     /// Switch the current screen
//     Screen(usize),
//     ScaleUp,
//     ScaleDown,
//     TogglePreview,
//     RotateOutput,
//     ToggleTint,
//     ToggleDecorations,
//     MoveWindow(WindowPosition),
//     Action(Action),
//     /// Do nothing more
//     None,
// }

impl State {
    fn process_keyboard_shortcut(
        &mut self,
        modifiers: ModifiersState,
        keysym: Keysym,
    ) -> Option<Action> {
        if modifiers.ctrl && modifiers.alt && keysym == Keysym::BackSpace
            || modifiers.logo && keysym == Keysym::Q
        {
            // ctrl+alt+backspace = quit
            // logo + q = quit
            Some(Action::Quit)
        } else if (xkb::KEY_XF86Switch_VT_1..=xkb::KEY_XF86Switch_VT_12).contains(&keysym.raw()) {
            // VTSwitch
            Some(Action::VtSwitch(
                (keysym.raw() - xkb::KEY_XF86Switch_VT_1 + 1) as i32,
            ))
        } else if modifiers.alt && keysym == Keysym::Tab {
            self.tab_index += 1;
            Some(Action::Tab {
                index: self.tab_index,
            })
        } else {
            let maps = self.key_maps.get(&modifiers.into())?;
            let callback = maps.get(&keysym)?;
            Some(Action::Callback(callback.clone()))
        }
    }
}
