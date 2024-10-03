/// Represents the messages that can be sent to the renderer thread
pub enum RendererMessage {
    /// Requests the renderer thread to shut down
    Shutdown,
}
