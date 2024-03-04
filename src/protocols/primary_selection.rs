use crate::State;
use smithay::{
    delegate_primary_selection,
    wayland::selection::primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
};

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

delegate_primary_selection!(State);
