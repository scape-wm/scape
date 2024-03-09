use std::os::fd::OwnedFd;

use smithay::{
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    input::{keyboard::LedState, pointer::CursorImageStatus, Seat, SeatHandler, SeatState},
    reexports::wayland_server::{
        protocol::{wl_data_source::WlDataSource, wl_surface::WlSurface},
        Resource,
    },
    wayland::{
        output::OutputHandler,
        seat::WaylandFocus,
        selection::{
            data_device::{
                set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState,
                ServerDndGrabHandler,
            },
            primary_selection::set_primary_focus,
            SelectionHandler, SelectionSource, SelectionTarget,
        },
        shm::{ShmHandler, ShmState},
    },
};
use tracing::warn;

use crate::{focus::FocusTarget, State};

delegate_compositor!(State);

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for State {
    fn started(
        &mut self,
        _source: Option<WlDataSource>,
        icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        self.dnd_icon = icon;
    }

    fn dropped(&mut self, _seat: Seat<Self>) {
        self.dnd_icon = None;
    }
}

impl ServerDndGrabHandler for State {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {
        warn!("Server dnd grab handler not supported");
    }
}

delegate_data_device!(State);

impl OutputHandler for State {}

delegate_output!(State);

impl SelectionHandler for State {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        let Some(ref mut xwayland_state) = &mut self.xwayland_state else {
            return;
        };
        if let Some(xwm) = xwayland_state.wm.as_mut() {
            if let Err(err) = xwm.new_selection(ty, source.map(|source| source.mime_types())) {
                warn!(?err, ?ty, "Failed to set Xwayland selection");
            }
        }
    }

    fn send_selection(
        &mut self,
        ty: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        _user_data: &(),
    ) {
        let Some(ref mut xwayland_state) = &mut self.xwayland_state else {
            return;
        };
        if let Some(xwm) = xwayland_state.wm.as_mut() {
            if let Err(err) = xwm.send_selection(ty, mime_type, fd, self.loop_handle.clone()) {
                warn!(?err, "Failed to send primary (X11 -> Wayland)");
            }
        }
    }
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_shm!(State);

impl SeatHandler for State {
    type KeyboardFocus = FocusTarget;
    type PointerFocus = FocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, target: Option<&FocusTarget>) {
        let dh = &self.display_handle;

        let focus = target
            .and_then(WaylandFocus::wl_surface)
            .and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, focus.clone());
        set_primary_focus(dh, seat, focus);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, led_state: LedState) {
        self.backend_data.update_led_state(led_state)
    }
}

delegate_seat!(State);
