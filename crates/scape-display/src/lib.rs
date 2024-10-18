pub mod action;
pub mod application_window;
pub mod command;
pub mod composition;
pub mod config;
pub mod config_watcher;
pub mod cursor;
pub mod dbus;
pub mod drawing;
pub mod egui;
pub mod egui_window;
pub mod focus;
pub mod grabs;
pub mod input_handler;
pub mod pipewire;
pub mod protocols;
pub mod render;
pub mod shell;
pub mod ssd;
pub mod state;
pub mod udev;
pub mod wayland;
pub mod winit;
pub mod workspace_window;
pub mod xwayland;

use calloop::channel::Channel;
use scape_shared::{Comms, DisplayMessage, GlobalArgs};
pub use state::{ClientState, State};
use tracing::{span, Level};

pub fn run(
    comms: Comms,
    channel: Channel<DisplayMessage>,
    args: &GlobalArgs,
) -> anyhow::Result<()> {
    let span = span!(Level::ERROR, "display");
    let _guard = span.enter();
    wayland::run(comms, channel, args)
}
