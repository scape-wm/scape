pub use smithay::{
    backend::input::KeyState,
    desktop::{LayerSurface, PopupKind},
    input::{
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{AxisFrame, ButtonEvent, MotionEvent, PointerTarget, RelativeMotionEvent},
        Seat,
    },
    reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface, Resource},
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
};
use smithay::{
    desktop::{Window, WindowSurface},
    input::pointer::{
        GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
        GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
        GestureSwipeUpdateEvent,
    },
    wayland::session_lock::LockSurface,
};
use smithay::{input::touch::TouchTarget, xwayland::X11Surface};

use crate::{
    application_window::{ApplicationWindow, SSD},
    State,
};

#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardFocusTarget {
    Window(Window),
    LayerSurface(LayerSurface),
    Popup(PopupKind),
    LockScreen(LockSurface),
}

impl IsAlive for KeyboardFocusTarget {
    fn alive(&self) -> bool {
        match self {
            KeyboardFocusTarget::Window(w) => w.alive(),
            KeyboardFocusTarget::LayerSurface(l) => l.alive(),
            KeyboardFocusTarget::Popup(p) => p.alive(),
            KeyboardFocusTarget::LockScreen(l) => l.alive(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PointerFocusTarget {
    WlSurface(WlSurface),
    X11Surface(X11Surface),
    SSD(SSD),
}

impl IsAlive for PointerFocusTarget {
    fn alive(&self) -> bool {
        match self {
            PointerFocusTarget::WlSurface(w) => w.alive(),
            PointerFocusTarget::X11Surface(w) => w.alive(),
            PointerFocusTarget::SSD(x) => x.alive(),
        }
    }
}

impl From<PointerFocusTarget> for WlSurface {
    fn from(target: PointerFocusTarget) -> Self {
        target.wl_surface().unwrap()
    }
}

impl PointerTarget<State> for PointerFocusTarget {
    fn enter(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::enter(w, seat, data, event),
            PointerFocusTarget::X11Surface(w) => PointerTarget::enter(w, seat, data, event),
            PointerFocusTarget::SSD(w) => PointerTarget::enter(w, seat, data, event),
        }
    }

    fn motion(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::motion(w, seat, data, event),
            PointerFocusTarget::X11Surface(w) => PointerTarget::motion(w, seat, data, event),
            PointerFocusTarget::SSD(w) => PointerTarget::motion(w, seat, data, event),
        }
    }

    fn relative_motion(&self, seat: &Seat<State>, data: &mut State, event: &RelativeMotionEvent) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::relative_motion(w, seat, data, event)
            }
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::relative_motion(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::relative_motion(w, seat, data, event),
        }
    }

    fn button(&self, seat: &Seat<State>, data: &mut State, event: &ButtonEvent) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::button(w, seat, data, event),
            PointerFocusTarget::X11Surface(w) => PointerTarget::button(w, seat, data, event),
            PointerFocusTarget::SSD(w) => PointerTarget::button(w, seat, data, event),
        }
    }
    fn axis(&self, seat: &Seat<State>, data: &mut State, frame: AxisFrame) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::axis(w, seat, data, frame),
            PointerFocusTarget::X11Surface(w) => PointerTarget::axis(w, seat, data, frame),
            PointerFocusTarget::SSD(w) => PointerTarget::axis(w, seat, data, frame),
        }
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::frame(w, seat, data),
            PointerFocusTarget::X11Surface(w) => PointerTarget::frame(w, seat, data),
            PointerFocusTarget::SSD(w) => PointerTarget::frame(w, seat, data),
        }
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial, time: u32) {
        match self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::leave(w, seat, data, serial, time),
            PointerFocusTarget::X11Surface(w) => PointerTarget::leave(w, seat, data, serial, time),
            PointerFocusTarget::SSD(w) => PointerTarget::leave(w, seat, data, serial, time),
        }
    }

    fn gesture_swipe_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeBeginEvent,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_swipe_begin(w, seat, data, event)
            }
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_swipe_begin(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_swipe_begin(w, seat, data, event),
        }
    }

    fn gesture_swipe_update(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeUpdateEvent,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_swipe_update(w, seat, data, event)
            }
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_swipe_update(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_swipe_update(w, seat, data, event),
        }
    }

    fn gesture_swipe_end(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeEndEvent,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_swipe_end(w, seat, data, event)
            }
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_swipe_end(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_swipe_end(w, seat, data, event),
        }
    }

    fn gesture_pinch_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchBeginEvent,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_pinch_begin(w, seat, data, event)
            }
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_pinch_begin(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_pinch_begin(w, seat, data, event),
        }
    }

    fn gesture_pinch_update(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchUpdateEvent,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_pinch_update(w, seat, data, event)
            }
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_pinch_update(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_pinch_update(w, seat, data, event),
        }
    }

    fn gesture_pinch_end(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchEndEvent,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_pinch_end(w, seat, data, event)
            }
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_pinch_end(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_pinch_end(w, seat, data, event),
        }
    }

    fn gesture_hold_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureHoldBeginEvent,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_hold_begin(w, seat, data, event)
            }
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_hold_begin(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_hold_begin(w, seat, data, event),
        }
    }

    fn gesture_hold_end(&self, seat: &Seat<State>, data: &mut State, event: &GestureHoldEndEvent) {
        match self {
            PointerFocusTarget::WlSurface(w) => {
                PointerTarget::gesture_hold_end(w, seat, data, event)
            }
            PointerFocusTarget::X11Surface(w) => {
                PointerTarget::gesture_hold_end(w, seat, data, event)
            }
            PointerFocusTarget::SSD(w) => PointerTarget::gesture_hold_end(w, seat, data, event),
        }
    }
}

impl KeyboardTarget<State> for KeyboardFocusTarget {
    fn enter(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => {
                    KeyboardTarget::enter(w.wl_surface(), seat, data, keys, serial)
                }
                WindowSurface::X11(s) => KeyboardTarget::enter(s, seat, data, keys, serial),
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                KeyboardTarget::enter(l.wl_surface(), seat, data, keys, serial)
            }
            KeyboardFocusTarget::Popup(p) => {
                KeyboardTarget::enter(p.wl_surface(), seat, data, keys, serial)
            }
            KeyboardFocusTarget::LockScreen(l) => {
                KeyboardTarget::enter(l.wl_surface(), seat, data, keys, serial)
            }
        }
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => {
                    KeyboardTarget::leave(w.wl_surface(), seat, data, serial)
                }
                WindowSurface::X11(s) => KeyboardTarget::leave(s, seat, data, serial),
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                KeyboardTarget::leave(l.wl_surface(), seat, data, serial)
            }
            KeyboardFocusTarget::Popup(p) => {
                KeyboardTarget::leave(p.wl_surface(), seat, data, serial)
            }
            KeyboardFocusTarget::LockScreen(l) => {
                KeyboardTarget::leave(l.wl_surface(), seat, data, serial)
            }
        }
    }

    fn key(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: u32,
    ) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => {
                    KeyboardTarget::key(w.wl_surface(), seat, data, key, state, serial, time)
                }
                WindowSurface::X11(s) => {
                    KeyboardTarget::key(s, seat, data, key, state, serial, time)
                }
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                KeyboardTarget::key(l.wl_surface(), seat, data, key, state, serial, time)
            }
            KeyboardFocusTarget::Popup(p) => {
                KeyboardTarget::key(p.wl_surface(), seat, data, key, state, serial, time)
            }
            KeyboardFocusTarget::LockScreen(l) => {
                KeyboardTarget::key(l.wl_surface(), seat, data, key, state, serial, time)
            }
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => {
                    KeyboardTarget::modifiers(w.wl_surface(), seat, data, modifiers, serial)
                }
                WindowSurface::X11(s) => {
                    KeyboardTarget::modifiers(s, seat, data, modifiers, serial)
                }
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                KeyboardTarget::modifiers(l.wl_surface(), seat, data, modifiers, serial)
            }
            KeyboardFocusTarget::Popup(p) => {
                KeyboardTarget::modifiers(p.wl_surface(), seat, data, modifiers, serial)
            }
            KeyboardFocusTarget::LockScreen(l) => {
                KeyboardTarget::modifiers(l.wl_surface(), seat, data, modifiers, serial)
            }
        }
    }
}

impl TouchTarget<State> for PointerFocusTarget {
    fn down(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &smithay::input::touch::DownEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::down(w, seat, data, event, seq),
            PointerFocusTarget::X11Surface(w) => TouchTarget::down(w, seat, data, event, seq),
            PointerFocusTarget::SSD(w) => TouchTarget::down(w, seat, data, event, seq),
        }
    }

    fn up(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &smithay::input::touch::UpEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::up(w, seat, data, event, seq),
            PointerFocusTarget::X11Surface(w) => TouchTarget::up(w, seat, data, event, seq),
            PointerFocusTarget::SSD(w) => TouchTarget::up(w, seat, data, event, seq),
        }
    }

    fn motion(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &smithay::input::touch::MotionEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::motion(w, seat, data, event, seq),
            PointerFocusTarget::X11Surface(w) => TouchTarget::motion(w, seat, data, event, seq),
            PointerFocusTarget::SSD(w) => TouchTarget::motion(w, seat, data, event, seq),
        }
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State, seq: Serial) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::frame(w, seat, data, seq),
            PointerFocusTarget::X11Surface(w) => TouchTarget::frame(w, seat, data, seq),
            PointerFocusTarget::SSD(w) => TouchTarget::frame(w, seat, data, seq),
        }
    }

    fn cancel(&self, seat: &Seat<State>, data: &mut State, seq: Serial) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::cancel(w, seat, data, seq),
            PointerFocusTarget::X11Surface(w) => TouchTarget::cancel(w, seat, data, seq),
            PointerFocusTarget::SSD(w) => TouchTarget::cancel(w, seat, data, seq),
        }
    }

    fn shape(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &smithay::input::touch::ShapeEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::shape(w, seat, data, event, seq),
            PointerFocusTarget::X11Surface(w) => TouchTarget::shape(w, seat, data, event, seq),
            PointerFocusTarget::SSD(w) => TouchTarget::shape(w, seat, data, event, seq),
        }
    }

    fn orientation(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &smithay::input::touch::OrientationEvent,
        seq: Serial,
    ) {
        match self {
            PointerFocusTarget::WlSurface(w) => TouchTarget::orientation(w, seat, data, event, seq),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => {
                TouchTarget::orientation(w, seat, data, event, seq)
            }
            PointerFocusTarget::SSD(w) => TouchTarget::orientation(w, seat, data, event, seq),
        }
    }
}

impl WaylandFocus for PointerFocusTarget {
    fn wl_surface(&self) -> Option<WlSurface> {
        match self {
            PointerFocusTarget::WlSurface(w) => w.wl_surface(),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => w.wl_surface(),
            PointerFocusTarget::SSD(_) => None,
        }
    }
    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            PointerFocusTarget::WlSurface(w) => w.same_client_as(object_id),
            #[cfg(feature = "xwayland")]
            PointerFocusTarget::X11Surface(w) => w.same_client_as(object_id),
            PointerFocusTarget::SSD(w) => w
                .wl_surface()
                .map(|surface| surface.same_client_as(object_id))
                .unwrap_or(false),
        }
    }
}

impl WaylandFocus for KeyboardFocusTarget {
    fn wl_surface(&self) -> Option<WlSurface> {
        match self {
            KeyboardFocusTarget::Window(w) => w.wl_surface(),
            KeyboardFocusTarget::LayerSurface(l) => Some(l.wl_surface().clone()),
            KeyboardFocusTarget::Popup(p) => Some(p.wl_surface().clone()),
            KeyboardFocusTarget::LockScreen(l) => Some(l.wl_surface().clone()),
        }
    }
}

impl From<WlSurface> for PointerFocusTarget {
    fn from(value: WlSurface) -> Self {
        PointerFocusTarget::WlSurface(value)
    }
}

impl From<&WlSurface> for PointerFocusTarget {
    fn from(value: &WlSurface) -> Self {
        PointerFocusTarget::from(value.clone())
    }
}

impl From<PopupKind> for PointerFocusTarget {
    fn from(value: PopupKind) -> Self {
        PointerFocusTarget::from(value.wl_surface())
    }
}

#[cfg(feature = "xwayland")]
impl From<X11Surface> for PointerFocusTarget {
    fn from(value: X11Surface) -> Self {
        PointerFocusTarget::X11Surface(value)
    }
}

#[cfg(feature = "xwayland")]
impl From<&X11Surface> for PointerFocusTarget {
    fn from(value: &X11Surface) -> Self {
        PointerFocusTarget::from(value.clone())
    }
}

impl From<ApplicationWindow> for KeyboardFocusTarget {
    fn from(w: ApplicationWindow) -> Self {
        KeyboardFocusTarget::Window(w.0.clone())
    }
}

impl From<LayerSurface> for KeyboardFocusTarget {
    fn from(l: LayerSurface) -> Self {
        KeyboardFocusTarget::LayerSurface(l)
    }
}

impl From<PopupKind> for KeyboardFocusTarget {
    fn from(p: PopupKind) -> Self {
        KeyboardFocusTarget::Popup(p)
    }
}

impl From<LockSurface> for KeyboardFocusTarget {
    fn from(l: LockSurface) -> Self {
        KeyboardFocusTarget::LockScreen(l)
    }
}

impl From<KeyboardFocusTarget> for PointerFocusTarget {
    fn from(value: KeyboardFocusTarget) -> Self {
        match value {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(w) => PointerFocusTarget::from(w.wl_surface()),
                WindowSurface::X11(s) => PointerFocusTarget::from(s),
            },
            KeyboardFocusTarget::LayerSurface(surface) => {
                PointerFocusTarget::from(surface.wl_surface())
            }
            KeyboardFocusTarget::Popup(popup) => PointerFocusTarget::from(popup.wl_surface()),
            KeyboardFocusTarget::LockScreen(popup) => PointerFocusTarget::from(popup.wl_surface()),
        }
    }
}
