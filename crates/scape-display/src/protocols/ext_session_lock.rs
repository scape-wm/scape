use std::collections::HashMap;

use smithay::{
    delegate_session_lock,
    output::Output,
    reexports::wayland_server::{protocol::wl_output::WlOutput, Resource},
    utils::{Size, SERIAL_COUNTER},
    wayland::session_lock::{LockSurface, SessionLockHandler, SessionLocker},
};

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

        let maybe_elem = self
            .spaces
            .values()
            .next()
            .unwrap()
            .elements()
            .next_back()
            .cloned();
        if let Some(elem) = maybe_elem {
            // TODO: Handle multiple spaces
            let keyboard = self.seat.as_ref().unwrap().get_keyboard().unwrap();
            let serial = SERIAL_COUNTER.next_serial();
            keyboard.set_focus(self, Some(elem.into()), serial);
        }
        // TODO: restore pointer grabs
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

                // set keyboard focus to new surface
                let keyboard = self.seat.as_ref().unwrap().get_keyboard().unwrap();
                let serial = SERIAL_COUNTER.next_serial();
                keyboard.set_focus(self, Some(lock_surface.into()), serial);

                // TODO: Unset pointer grab
                // let pointer = self.seat.as_ref().unwrap().get_pointer().unwrap();
                // pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), self.clock.now().into());
            }
        }
    }
}
delegate_session_lock!(State);
