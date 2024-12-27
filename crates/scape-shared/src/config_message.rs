use crate::{CallbackRef, Output};

/// Represents the messages that can be sent to the config thread
pub enum ConfigMessage {
    /// Requests the config thread to shut down
    Shutdown,
    /// Request to run the given callback
    RunCallback(CallbackRef),
    /// Forgets the callback, usually because it is no longer possible to run it, e.g. because the
    /// callback is no longer registered
    ForgetCallback(CallbackRef),
    /// Notifies the config thread that the application has started
    Startup,
    /// Notifies the config thread that a connector has changed
    ConnectorChange(Vec<Output>),
    /// Set extra environment variables, which are used for spawning processes
    ExtraEnv {
        /// The name of the environment variable
        name: String,
        /// The value of the environment variable
        value: String,
    },
}
