use std::process::Command;

use mlua::Function as LuaFunction;
use tracing::{error, info, warn};

use crate::{workspace_window::WorkspaceWindow, State};

#[derive(Debug)]
pub enum Action {
    /// Quit the compositor
    Quit,
    /// Trigger a vt-switch
    VtSwitch(i32),
    /// Spawn a command
    Spawn { command: String, args: Vec<String> },
    /// Focus or spawn a command
    FocusOrSpawn { app_id: String, command: String },
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
    /// Tab through windows
    Tab { index: usize },
    /// Close current window
    Close,
    /// Do nothing more
    None,
}

impl State {
    pub fn execute(&mut self, action: Action) {
        info!(?action, "Executing action");
        if self.session_lock.is_some() && !matches!(action, Action::VtSwitch(_)) {
            warn!("No action is executed, since session is locked");
            return;
        }
        match action {
            Action::Quit => {
                self.stop_loop();
                self.config.stop();
                self.clear_key_map();
            }
            Action::VtSwitch(vt) => {
                info!(to = vt, "Trying to switch vt");
                if let Err(err) = self.backend_data.switch_vt(vt) {
                    error!(vt, "Error switching vt: {}", err);
                }
            }
            Action::Spawn { command, args } => self.spawn(&command, &args),
            Action::ChangeScale {
                percentage_points: _,
            } => todo!(),
            Action::SetScale { percentage: _ } => todo!(),
            Action::RotateOutput {
                output: _,
                rotation: _,
            } => todo!(),
            Action::MoveWindow { window: _, zone } => {
                let (space_name, _) = self.spaces.iter().next().unwrap();
                let keyboard = self.seat.as_ref().unwrap().get_keyboard().unwrap();
                if let Some(focus) = keyboard.current_focus() {
                    if let Ok(window) = WorkspaceWindow::try_from(focus) {
                        self.place_window(
                            &space_name.to_owned(),
                            &window,
                            false,
                            Some(&zone),
                            true,
                        );
                    }
                }
            }
            Action::Close => {
                let (_, space) = self.spaces.iter_mut().next().unwrap();
                if let Some(window) = space.elements().last().cloned() {
                    if window.close() {
                        space.unmap_elem(&window);
                    }
                }
            }
            Action::Tab { index } => {
                let (space_name, space) = self.spaces.iter().next().unwrap();
                let maybe_window = space.elements().rev().nth(index).cloned();
                if let Some(window) = maybe_window {
                    self.focus_window(window, &space_name.to_owned());
                }
            }
            Action::Callback(callback) => callback.call(()).unwrap(),
            Action::FocusOrSpawn { app_id, command } => {
                if !self.focus_window_by_app_id(app_id) {
                    self.execute(Action::Spawn {
                        command,
                        args: Vec::new(),
                    });
                }
            }
            Action::None => {}
        }
    }

    fn spawn(&self, command: &str, args: &[String]) {
        info!(command, "Starting program");

        if let Err(e) = Command::new(command)
            .args(args)
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
