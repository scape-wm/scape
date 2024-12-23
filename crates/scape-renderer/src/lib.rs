//! The renderer module is responsible for rendering various UI elements based on the current state.

#![warn(missing_docs)]

use calloop::{LoopHandle, LoopSignal};
use scape_shared::{Comms, GlobalArgs, MessageRunner, RendererMessage};

/// Holds the state of the renderer module
pub struct RendererState {
    comms: Comms,
    shutting_down: bool,
    loop_handle: LoopHandle<'static, RendererState>,
}

impl MessageRunner for RendererState {
    type Message = RendererMessage;

    fn new(
        comms: Comms,
        loop_handle: LoopHandle<'static, RendererState>,
        _args: &GlobalArgs,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            comms,
            shutting_down: false,
            loop_handle,
        })
    }

    fn handle_message(&mut self, message: RendererMessage) -> anyhow::Result<()> {
        match message {
            RendererMessage::Shutdown => {
                self.shutting_down = true;
            }
        }

        Ok(())
    }

    fn on_dispatch_wait(&mut self, signal: &LoopSignal) {
        if self.shutting_down {
            signal.stop();
        }
    }
}
