use smithay::backend::session::libseat::LibSeatSession;

use crate::{CallbackRef, Mods};

/// Represents the messages that can be sent to the input thread
pub enum InputMessage {
    /// Requests the input thread to shut down
    Shutdown,
    /// Notifies the input thread that a new seat session has been created
    SeatSessionCreated {
        /// The seat session
        session: LibSeatSession,
    },
    /// Notifies the input thread that a seat session has been suspended
    SeatSessionSuspended,
    /// Notifies the input thread that a seat session has been resumed
    SeatSessionResumed,
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
