pub mod action;
pub mod application_window;
pub mod command;
pub mod composition;
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

use calloop::{channel::Channel, LoopHandle};
use scape_shared::{Comms, DisplayMessage, GlobalArgs, MessageRunner};
pub use state::{ClientState, State};
use tracing::{span, Level};

/// Holds the state of the display module
pub struct DisplayState {
    comms: Comms,
    shutting_down: bool,
    loop_handle: LoopHandle<'static, DisplayState>,
}

impl MessageRunner for DisplayState {
    type Message = DisplayMessage;

    fn new(
        comms: Comms,
        loop_handle: LoopHandle<'static, Self>,
        args: &GlobalArgs,
    ) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Ok(DisplayState {
            comms,
            shutting_down: false,
            loop_handle,
        })
    }

    fn handle_message(&mut self, message: Self::Message) -> anyhow::Result<()> {
        match message {
            DisplayMessage::Shutdown => {
                self.shutting_down = true;
            }
            DisplayMessage::KeyboardInput {
                keycode,
                key_state,
                modifiers_changed,
                time,
            } => (),
            DisplayMessage::Action(_) => (),
            DisplayMessage::SetZones(_) => (),
            DisplayMessage::MoveCurrentWindowToZone(_) => (),
            DisplayMessage::VtSwitch(_) => (),
            DisplayMessage::FocusOrSpawn {
                app_id,
                command,
                args,
            } => (),
            DisplayMessage::CloseCurrentWindow => (),
            DisplayMessage::AddWindowRule(_) => (),
            DisplayMessage::ToggleDebugUi => (),
            DisplayMessage::StartVideoStream => (),
            DisplayMessage::SetLayout { spaces } => (),
        }
        Ok(())
    }

    fn on_dispatch_wait(&mut self, signal: &calloop::LoopSignal) {
        if self.shutting_down {
            signal.stop();
        }
    }
}

pub fn run(
    comms: Comms,
    channel: Channel<DisplayMessage>,
    args: &GlobalArgs,
) -> anyhow::Result<()> {
    let span = span!(Level::ERROR, "display");
    let _guard = span.enter();
    wayland::run(comms, channel, args)
}
