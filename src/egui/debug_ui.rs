use egui::Context;
use smithay::utils::Rectangle;

use crate::{
    egui_window::{EguiAppState, EguiWindow},
    workspace_window::WorkspaceWindow,
    State,
};

use super::EguiState;

#[derive(Debug, PartialEq, Clone)]
pub struct DebugUi {
    age: isize,
    name: String,
}

impl DebugUi {
    pub fn new() -> Self {
        DebugUi {
            age: 0,
            name: "Test".to_string(),
        }
    }

    pub fn show(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=isize::MAX).text("age"));
            self.age += 1;
            if ui.button("Increment").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));
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
        if self.debug_ui.take().is_none() {
            self.debug_ui = Some((
                EguiState::new(Rectangle::from_loc_and_size((0, 0), (500, 500))),
                DebugUi::default(),
            ));
            if let Some(space_name) = self.spaces.keys().next().cloned() {
                self.place_window(
                    &space_name,
                    &WorkspaceWindow::from(EguiWindow::new(DebugUi::default())),
                    true,
                    None,
                    true,
                );
            }
        }
    }
}

impl From<DebugUi> for EguiAppState {
    fn from(debug_ui: DebugUi) -> Self {
        EguiAppState::DebugUi(debug_ui)
    }
}
