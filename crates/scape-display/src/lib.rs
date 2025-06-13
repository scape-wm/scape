use calloop::{channel::Channel, LoopHandle};
use scape_shared::{Comms, DisplayMessage, GlobalArgs, MessageRunner};
// pub use state::{ClientState, State};
use log::{info, warn};
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// pub mod action;
// pub mod application_window;
// pub mod command;
// pub mod composition;
// pub mod cursor;
// pub mod dbus;
// pub mod drawing;
// pub mod egui;
// pub mod egui_window;
// pub mod focus;
// pub mod grabs;
// pub mod input_handler;
// pub mod pipewire;
// pub mod protocols;
// pub mod render;
// pub mod shell;
// pub mod ssd;
// pub mod state;
// pub mod udev;
mod wayland;
// pub mod winit;
// pub mod workspace_window;
// pub mod xwayland;

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
        let state = DisplayState {
            comms,
            shutting_down: false,
            loop_handle,
        };
        state.start_display();

        Ok(state)
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
