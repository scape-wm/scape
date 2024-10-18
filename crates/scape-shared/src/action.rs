/// Represents the actions that a user can perform
#[derive(Debug)]
pub enum Action {
    /// Quit the compositor
    Quit,
    /// Trigger a vt-switch
    VtSwitch(i32),
    /// Spawn a command
    Spawn {
        /// The command (program) to run
        command: String,
        /// The arguments to pass to the command
        args: Vec<String>,
    },
    /// Focus application or spawn a command
    FocusOrSpawn {
        /// The application id of the application to focus
        app_id: String,
        /// The command (program) to run
        command: String,
    },
    /// Scales output up/down
    ChangeScale {
        /// The percentage points to scale the output up/down
        percentage_points: isize,
    },
    /// Sets output scale
    SetScale {
        /// The percentage to set the output scale to
        percentage: usize,
    },
    /// Rotate output
    RotateOutput {
        /// The output to rotate
        output: usize,
        /// The rotation to apply
        rotation: usize,
    },
    /// Move window to zone
    MoveWindow {
        /// The window to move
        window: Option<usize>,
        /// The zone to move the window to
        zone: String,
    },
    /// Run callback
    Callback(CallbackRef),
    /// Tab through windows
    Tab {
        /// The index of the window to focus
        index: usize,
    },
    /// Close current window
    Close,
    /// Start pipewire video stream
    StartVideoStream,
    /// Do nothing
    None,
}

/// The callback reference
#[derive(Debug, Clone, Copy)]
pub struct CallbackRef {
    /// The callback reference id
    pub callback_id: usize,
}
