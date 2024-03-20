use crate::{state::ActiveSpace, State};
use smithay::{
    delegate_input_method_manager,
    desktop::{space::SpaceElement, PopupKind, PopupManager},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::Rectangle,
    wayland::{
        compositor::with_states,
        input_method::{InputMethodHandler, PopupSurface},
    },
};
use tracing::warn;

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
        let space_name = with_states(parent, |surface_data| {
            surface_data
                .data_map
                .get::<ActiveSpace>()
                .unwrap()
                .0
                .to_owned()
        });
        self.spaces[&space_name]
            .elements()
            .find_map(|window| {
                (window.wl_surface().as_ref() == Some(parent)).then(|| window.geometry())
            })
            .unwrap_or_default()
    }
}

delegate_input_method_manager!(State);
