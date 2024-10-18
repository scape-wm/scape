use smithay::backend::session::libseat::LibSeatSession;

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
}
