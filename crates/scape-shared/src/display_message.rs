use std::collections::HashMap;

use crate::{Action, Output, WindowRule, Zone};

/// Represents the messages that can be sent to the display thread
pub enum DisplayMessage {
    /// Requests the display thread to shut down
    Shutdown,
    /// A keyboard input event needs to be forwarded
    KeyboardInput {
        /// The keyboard code of the key that was pressed or released
        keycode: Keycode,
        /// The state of the key, it it was pressed or released
        key_state: KeyState,
        /// Whether the modifiers have changed with this input
        modifiers_changed: bool,
        /// The time in milliseconds, when the key was pressed or released
        time: u32,
    },
    /// An action needs to be executed
    Action(Action),
    /// Overwrite all zones known to the compositor
    SetZones(Vec<Zone>),
    /// Move the currently keyboard focused window to the given zone
    MoveCurrentWindowToZone(String),
    /// Trigger a vt-switch
    VtSwitch(i32),
    /// Focus the window with the given app_id or spawn a process with the given command
    FocusOrSpawn {
        /// The app_id of the window to focus
        app_id: String,
        /// The command to spawn
        command: String,
        /// The arguments to pass to the command
        args: Vec<String>,
    },
    /// Close the currently keyboard focused window
    CloseCurrentWindow,
    /// Add a window rule
    AddWindowRule(WindowRule),
    /// Toggle the debug UI
    ToggleDebugUi,
    /// Start a video stream
    StartVideoStream,
    /// Set the layout
    SetLayout {
        /// The spaces of the layout
        spaces: HashMap<String, Vec<Output>>,
    },
}
