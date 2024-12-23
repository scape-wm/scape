use crate::{CallbackRef, Output};

/// Represents the messages that can be sent to the config thread
pub enum ConfigMessage {
    /// Requests the config thread to shut down
    Shutdown,
    /// Request to run the given callback
    RunCallback(CallbackRef),
    /// Notifies the config thread that the application has started
    Startup,
    /// Notifies the config thread that a connector has changed
    ConnectorChange(Vec<Output>),
}
