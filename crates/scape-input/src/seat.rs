use anyhow::Context;
use calloop::LoopHandle;
use scape_shared::RendererMessage;
use smithay::backend::session::{libseat::LibSeatSession, Event as SessionEvent};
use tracing::{error, info};

use crate::InputState;

pub(crate) fn start_seat_session(
    loop_handle: LoopHandle<'static, InputState>,
) -> anyhow::Result<LibSeatSession> {
    let (session, notifier) = LibSeatSession::new().context("Unable to create lib seat session")?;

    loop_handle
        .insert_source(notifier, move |event, &mut (), state| match event {
            SessionEvent::PauseSession => {
                info!("Pausing session");
                state.comms.renderer(RendererMessage::SeatSessionPaused);
                state.libinput_context.suspend();
            }
            SessionEvent::ActivateSession => {
                info!("Resuming session");
                state.comms.renderer(RendererMessage::SeatSessionResumed);
                if state.libinput_context.resume().is_err() {
                    error!("Failed to resume libinput context");
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Unable to insert lib seat session source: {}", e))?;

    Ok(session)
}
