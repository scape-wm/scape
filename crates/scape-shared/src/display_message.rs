/// Represents the messages that can be sent to the display thread
pub enum DisplayMessage {
    /// Requests the display thread to shut down
    Shutdown,
}
