use crate::{focus::FocusTarget, State};
use smithay::{
    delegate_xwayland_keyboard_grab, reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabHandler,
};

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
