use std::{os::fd::OwnedFd, path::PathBuf};

/// Represents the messages that can be sent to the renderer thread
pub enum RendererMessage {
    /// Requests the renderer thread to shut down
    Shutdown,
    /// Seat session has been created
    SeatSessionCreated {
        /// The seat name
        seat_name: String,
    },
    /// The seat session has been paused
    SeatSessionPaused,
    /// The seat session has been resumed
    SeatSessionResumed,
    /// A file has been opened in the session
    FileOpenedInSession {
        /// The path that was opened
        path: PathBuf,
        /// The file descriptor
        fd: OwnedFd,
    },
}
