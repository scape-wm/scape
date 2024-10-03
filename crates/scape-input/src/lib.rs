//! The input module is responsible for handling keyboard, mouse and touch events from the user.

#![warn(missing_docs)]

use calloop::channel::Channel;
use scape_shared::{Comms, GlobalArgs, InputMessage};
use tracing::{span, Level};

/// Runs the input module, and only exits when it receives a shutdown signal.
pub fn run(
    _comms: Comms,
    _channel: Channel<InputMessage>,
    _args: &GlobalArgs,
) -> anyhow::Result<()> {
    let span = span!(Level::ERROR, "input");
    let _guard = span.enter();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(10000));
    }
}
