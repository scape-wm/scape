use crate::{
    egui_window::{EguiAppState, EguiWindow},
    workspace_window::WorkspaceWindow,
    State,
};
use egui::Context;
use smithay::desktop::space::SpaceElement;

#[derive(Debug, PartialEq, Clone)]
struct Space {
    name: String,
    windows: Vec<Window>,
}

#[derive(Debug, PartialEq, Clone)]
struct Window {
    name: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, PartialEq, Clone)]
pub struct DebugState {
    spaces: Vec<Space>,
}

impl From<&State> for DebugState {
    fn from(value: &State) -> Self {
        let spaces = value
            .spaces
            .iter()
            .map(|(name, space)| Space {
                name: name.to_string(),
                windows: space
                    .elements()
                    .map(|window| {
                        let geometry = window.geometry();

                        Window {
                            name: window.app_id(),
                            x: geometry.loc.x,
                            y: geometry.loc.y,
                            width: geometry.size.w,
                            height: geometry.size.h,
                        }
                    })
                    .collect(),
            })
            .collect();

        DebugState { spaces }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DebugUi {
    debug_state: Option<DebugState>,
}

impl DebugUi {
    pub fn new() -> Self {
        DebugUi { debug_state: None }
    }

    pub fn update(&mut self, debug_state: DebugState) -> bool {
        let new_state = Some(debug_state);
        if self.debug_state != new_state {
            self.debug_state = new_state;
            true
        } else {
            false
        }
    }

    pub fn show(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Debug UI");
            ui.separator();

            if let Some(debug_state) = &self.debug_state {
                for space in &debug_state.spaces {
                    ui.heading(&space.name);
                    for window in &space.windows {
                        ui.label(window.name.to_string());
                        ui.label(format!(
                            "({}, {}, {}, {})",
                            window.x, window.y, window.width, window.height
                        ));
                    }
                }
            }
        });
    }
}

impl Default for DebugUi {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn toggle_debug_ui(&mut self) {
        match self.debug_ui.take() {
            Some(window) => {
                if let Some(space) = self.spaces.values_mut().next() {
                    space.unmap_elem(&WorkspaceWindow::from(window));
                }
            }
            None => {
                let window = EguiWindow::new(DebugUi::default());
                self.debug_ui = Some(window.clone());
                if let Some(space_name) = self.spaces.keys().next().cloned() {
                    self.place_window(
                        &space_name,
                        &WorkspaceWindow::from(window),
                        true,
                        None,
                        true,
                    );
                }
            }
        }
    }
}

impl From<DebugUi> for EguiAppState {
    fn from(debug_ui: DebugUi) -> Self {
        EguiAppState::DebugUi(debug_ui)
    }
}
