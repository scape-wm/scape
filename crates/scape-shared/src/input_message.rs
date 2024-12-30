use std::path::PathBuf;

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
    /// Request to open the file at the given path in the current session, and return the fd to the
    /// renderer thread
    OpenFileInSessionForRenderer {
        /// The path to open
        path: PathBuf,
    },
}
