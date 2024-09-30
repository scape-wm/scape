use smithay::{
    delegate_drm_syncobj,
    wayland::drm_syncobj::{DrmSyncobjHandler, DrmSyncobjState},
};

use crate::State;

impl DrmSyncobjHandler for State {
    fn drm_syncobj_state(&mut self) -> &mut DrmSyncobjState {
        // TODO: Check when this is called. Can this panic?
        self.backend_data.syncobj_state().as_mut().unwrap()
    }
}

delegate_drm_syncobj!(State);
