//! Shared types that are used throughout scape

#![warn(missing_docs)]

use calloop::{LoopHandle, LoopSignal};

mod action;
mod args;
mod comms;
mod config_message;
mod display_message;
mod input_message;
mod main_message;
mod mods;
mod output;
mod renderer_message;
mod window_rule;
mod zone;

pub use action::Action;
pub use action::CallbackRef;
pub use args::get_global_args;
pub use args::GlobalArgs;
pub use comms::Comms;
pub use config_message::ConfigMessage;
pub use display_message::DisplayMessage;
pub use input_message::InputMessage;
pub use main_message::MainMessage;
pub use mods::Mods;
pub use output::Output;
pub use renderer_message::RendererMessage;
pub use window_rule::WindowRule;
pub use zone::Zone;

/// A trait for running a message loop
pub trait MessageRunner {
    /// The message type that this runner handles
    type Message;
    /// Creates a new instance of the runner
    fn new(
        comms: Comms,
        loop_handle: LoopHandle<'static, Self>,
        args: &GlobalArgs,
    ) -> anyhow::Result<Self>
    where
        Self: Sized;
    /// Handle a message
    fn handle_message(&mut self, message: Self::Message) -> anyhow::Result<()>;
    /// Called when the loop is waiting for a new message. The provided [`LoopSignal`] can be used to stop
    /// the loop.
    fn on_dispatch_wait(&mut self, signal: &LoopSignal);
}
