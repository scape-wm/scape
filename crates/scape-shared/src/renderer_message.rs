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
}
