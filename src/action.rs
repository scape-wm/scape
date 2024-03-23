use std::process::Command;

use mlua::Function as LuaFunction;
use tracing::{error, info};

use crate::State;

#[derive(Debug)]
pub enum Action {
    /// Quit the compositor
    Quit,
    /// Trigger a vt-switch
    VtSwitch(i32),
    /// Spawn a command
    Spawn { command: String },
    /// Scales output up/down
    ChangeScale { percentage_points: isize },
    /// Sets output scale
    SetScale { percentage: usize },
    /// Rotate output
    RotateOutput { output: usize, rotation: usize },
    /// Move window to zone
    MoveWindow { window: Option<usize>, zone: String },
    /// Run Lua callback
    Callback(LuaFunction<'static>),
    /// Do nothing more
    None,
}

impl State {
    pub fn execute(&mut self, action: Action) {
        info!(?action, "Executing action");
        match action {
            Action::Quit => self.stop_loop(),
            Action::VtSwitch(_) => todo!(),
            Action::Spawn { command } => self.spawn(&command),
            Action::ChangeScale {
                percentage_points: _,
            } => todo!(),
            Action::SetScale { percentage: _ } => todo!(),
            Action::RotateOutput {
                output: _,
                rotation: _,
            } => todo!(),
            Action::MoveWindow { window: _, zone: _ } => todo!(),
            Action::Callback(callback) => callback.call(()).unwrap(),
            Action::None => {}
        }
    }

    fn spawn(&self, command: &str) {
        info!(command, "Starting program");

        if let Err(e) = Command::new(command)
            .envs(
                self.socket_name
                    .clone()
                    .map(|v| ("WAYLAND_DISPLAY", v))
                    .into_iter()
                    .chain(
                        self.xwayland_state
                            .as_ref()
                            .and_then(|v| v.display)
                            .map(|v| ("DISPLAY", format!(":{}", v))),
                    ),
            )
            .spawn()
        {
            error!(command, err = %e, "Failed to start program");
        }
    }
}
