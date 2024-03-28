use std::collections::HashMap;

use smithay::{
    delegate_session_lock,
    output::Output,
    reexports::wayland_server::{protocol::wl_output::WlOutput, Resource},
    utils::Size,
    wayland::session_lock::{LockSurface, SessionLockHandler, SessionLocker},
};
use tracing::info;

use crate::{state::SessionLock, State};

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut smithay::wayland::session_lock::SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, locker: SessionLocker) {
        // Reject lock if session lock exists and is still valid
        if let Some(session_lock) = self.session_lock.as_ref() {
            if self
                .display_handle
                .get_client(session_lock.ext_session_lock.id())
                .is_ok()
            {
                return;
            }
        }

        let ext_session_lock = locker.ext_session_lock().clone();
        locker.lock();
        self.session_lock = Some(SessionLock {
            ext_session_lock,
            surfaces: HashMap::new(),
        });

        // for output in self.outputs {
        //     self.backend
        //         .schedule_render(&self.common.event_loop_handle, &output);
        // }
    }

    fn unlock(&mut self) {
        self.session_lock = None;

        // for output in self.common.shell.outputs() {
        //     self.backend
        //         .schedule_render(&self.common.event_loop_handle, &output);
        // }
    }

    fn new_surface(&mut self, lock_surface: LockSurface, wl_output: WlOutput) {
        if let Some(session_lock) = &mut self.session_lock {
            if let Some(output) = Output::from_resource(&wl_output) {
                lock_surface.with_pending_state(|states| {
                    let mode = output.preferred_mode().unwrap();
                    states.size = Some(Size::from((mode.size.w as u32, mode.size.h as u32)));
                });
                lock_surface.send_configure();
                session_lock
                    .surfaces
                    .insert(output.clone(), lock_surface.clone());
            }
        }
    }
}
delegate_session_lock!(State);
