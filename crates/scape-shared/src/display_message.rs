use smithay::backend::input::{KeyState, Keycode};

use crate::Action;

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
}
