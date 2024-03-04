use crate::{ClientState, State};
use smithay::{
    delegate_security_context,
    wayland::security_context::{
        SecurityContext, SecurityContextHandler, SecurityContextListenerSource,
    },
};
use std::sync::Arc;
use tracing::warn;

impl SecurityContextHandler for State {
    fn context_created(
        &mut self,
        source: SecurityContextListenerSource,
        security_context: SecurityContext,
    ) {
        self.loop_handle
            .insert_source(source, move |client_stream, _, state| {
                let client_state = ClientState {
                    security_context: Some(security_context.clone()),
                    ..ClientState::default()
                };
                if let Err(err) = state
                    .display_handle
                    .insert_client(client_stream, Arc::new(client_state))
                {
                    warn!("Error adding wayland client: {}", err);
                };
            })
            .expect("Failed to init wayland socket source");
    }
}

delegate_security_context!(State);
