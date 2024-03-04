use crate::State;
use smithay::{
    delegate_xdg_decoration,
    reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
    wayland::{
        compositor::with_states,
        shell::xdg::{decoration::XdgDecorationHandler, ToplevelSurface, XdgToplevelSurfaceData},
    },
};
use tracing::warn;

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ClientSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(match mode {
                DecorationMode::ServerSide => DecorationMode::ServerSide,
                _ => DecorationMode::ClientSide,
            });
        });

        let initial_configure_sent = with_states(toplevel.wl_surface(), |states| {
            if let Ok(data) = states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .try_lock()
            {
                data.initial_configure_sent
            } else {
                warn!("Unable to lock XdgToplevelSurfaceData in request mode");
                true
            }
        });
        if initial_configure_sent {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ClientSide);
        });
        let initial_configure_sent = with_states(toplevel.wl_surface(), |states| {
            if let Ok(data) = states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .try_lock()
            {
                data.initial_configure_sent
            } else {
                warn!("Unable to lock XdgToplevelSurfaceData in unset mode");
                true
            }
        });
        if initial_configure_sent {
            toplevel.send_pending_configure();
        }
    }
}

delegate_xdg_decoration!(State);
