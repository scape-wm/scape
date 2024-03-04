use crate::State;

#[derive(Debug)]
enum Action {
    /// Quit the compositor
    Quit,
    /// Trigger a vt-switch
    VtSwitch(i32),
    /// Run a command
    Run { command: String },
    /// Scales output up/down
    ChangeScale { percentage_points: isize },
    /// Sets output scale
    SetScale { percentage: usize },
    /// Rotate output
    RotateOutput { output: usize, rotation: usize },
    /// Move window to zone
    MoveWindow { window: Option<usize>, zone: String },
    /// Do nothing more
    None,
}

impl State {
    fn execute(&mut self, action: Action) {
        match action {
            Action::Quit => self.stop_loop(),
            Action::VtSwitch(_) => todo!(),
            Action::Run { command } => todo!(),
            Action::ChangeScale { percentage_points } => todo!(),
            Action::SetScale { percentage } => todo!(),
            Action::RotateOutput { output, rotation } => todo!(),
            Action::MoveWindow { window, zone } => todo!(),
            Action::None => todo!(),
        }
    }
}
