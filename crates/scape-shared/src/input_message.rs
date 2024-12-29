use crate::{CallbackRef, Mods};

/// Represents the messages that can be sent to the input thread
pub enum InputMessage {
    /// Requests the input thread to shut down
    Shutdown,
    /// Registers a new keyboard mapping
    Keymap {
        /// The name of the key that is mapped
        key_name: String,
        /// The modifiers that are required to trigger the key
        mods: Mods,
        /// The callback that is called when the key is pressed
        callback: CallbackRef,
    },
}
