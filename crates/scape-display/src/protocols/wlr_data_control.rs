use crate::State;
use smithay::{
    delegate_data_control,
    wayland::selection::wlr_data_control::{DataControlHandler, DataControlState},
};

impl DataControlHandler for State {
    fn data_control_state(&self) -> &DataControlState {
        &self.data_control_state
    }
}

delegate_data_control!(State);
