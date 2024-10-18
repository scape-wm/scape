//! Shared types that are used throughout scape

#![warn(missing_docs)]

mod action;
mod args;
mod comms;
mod display_message;
mod input_message;
mod main_message;
mod renderer_message;

pub use action::Action;
pub use action::CallbackRef;
pub use args::get_global_args;
pub use args::GlobalArgs;
pub use comms::Comms;
pub use display_message::DisplayMessage;
pub use input_message::InputMessage;
pub use main_message::MainMessage;
pub use renderer_message::RendererMessage;
