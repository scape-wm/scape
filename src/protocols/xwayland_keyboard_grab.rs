use crate::{focus::KeyboardFocusTarget, State};
use smithay::{
    delegate_xwayland_keyboard_grab, reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabHandler,
};

impl XWaylandKeyboardGrabHandler for State {
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<KeyboardFocusTarget> {
        let (window, _) = self.window_and_space_for_surface(surface)?;
        Some(window.into())
    }
}

delegate_xwayland_keyboard_grab!(State);
