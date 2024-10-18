use std::{process::Command, sync::atomic::Ordering};

use scape_shared::Action;
use tracing::{error, info, warn};

use crate::{
    dbus::portals::screen_cast::NODE_ID, pipewire::Pipewire, workspace_window::WorkspaceWindow,
    State,
};

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
            Action::StartVideoStream => {
                if self.pipewire.is_none() {
                    match Pipewire::new(self.loop_handle.clone()) {
                        Ok(pipewire) => self.pipewire = Some(pipewire),
                        Err(err) => {
                            error!("Failed to initialize pipewire: {}", err);
                            return;
                        }
                    }
                }

                let Some(gbm_device) = self.backend_data.gbm_device() else {
                    error!("No gbm device available");
                    return;
                };

                match self
                    .pipewire
                    .as_ref()
                    .unwrap()
                    .start_video_stream(gbm_device)
                {
                    Ok(stream) => {
                        info!("Pipewire video stream started");
                        NODE_ID.store(stream.node_id(), Ordering::SeqCst);
                        self.video_streams.push(stream);
                    }
                    Err(err) => error!(?err, "Failed to start pipewire video stream"),
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
                            .and_then(|v| v.display_number)
                            .map(|v| ("DISPLAY", format!(":{}", v))),
                    ),
            )
            .spawn()
        {
            error!(command, err = %e, "Failed to start program");
        }
    }
}
