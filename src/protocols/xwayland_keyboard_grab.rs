use crate::{focus::FocusTarget, State};
use smithay::{
    delegate_xwayland_keyboard_grab, reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabHandler,
};

impl XWaylandKeyboardGrabHandler for State {
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<FocusTarget> {
        let (window, _) = self.window_and_space_for_surface(surface)?;
        Some(FocusTarget::Window(window))
    }
}

delegate_xwayland_keyboard_grab!(State);
