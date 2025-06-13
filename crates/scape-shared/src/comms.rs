use calloop::channel::Sender;
use log::warn;

use crate::{ConfigMessage, DisplayMessage, InputMessage, MainMessage, RendererMessage};

/// Holds the channels for general communication and sending messages to the different threads.
/// Also, provides some convenience methods for interacting with other threads.
#[derive(Clone)]
pub struct Comms {
    to_main: Sender<MainMessage>,
    to_display: Sender<DisplayMessage>,
    to_renderer: Sender<RendererMessage>,
    to_input: Sender<InputMessage>,
    to_config: Sender<ConfigMessage>,
}

impl std::fmt::Debug for Comms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Comms").finish()
    }
}

impl Comms {
    /// Creates a new instance of `Comms` with the given channels.
    pub fn new(
        to_main: Sender<MainMessage>,
        to_display: Sender<DisplayMessage>,
        to_renderer: Sender<RendererMessage>,
        to_input: Sender<InputMessage>,
        to_config: Sender<ConfigMessage>,
    ) -> Self {
        Comms {
            to_main,
            to_display,
            to_renderer,
            to_input,
            to_config,
        }
    }

    /// Sends a message to the main thread.
    ///
    /// # Example
    /// ```
    /// # use calloop::channel::channel;
    /// # use scape_shared::{Comms, MainMessage};
    /// # let (to_main, main_channel) = channel();
    /// # let (to_display, _) = channel();
    /// # let (to_renderer, _) = channel();
    /// # let (to_input, _) = channel();
    /// # let (to_config, _) = channel();
    /// # let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
    /// comms.main(MainMessage::Shutdown);
    /// assert!(matches!(main_channel.recv().unwrap(), MainMessage::Shutdown));
    /// ```
    pub fn main(&self, message: MainMessage) {
        self.to_main
            .send(message)
            .expect("Lost connection to the main thread");
    }

    /// Sends a message to the display thread.
    ///
    /// # Example
    /// ```
    /// # use calloop::channel::channel;
    /// # use scape_shared::{Comms, DisplayMessage};
    /// # let (to_main, _) = channel();
    /// # let (to_display, display_channel) = channel();
    /// # let (to_renderer, _) = channel();
    /// # let (to_input, _) = channel();
    /// # let (to_config, _) = channel();
    /// # let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
    /// comms.display(DisplayMessage::Shutdown);
    /// assert!(matches!(display_channel.recv().unwrap(), DisplayMessage::Shutdown));
    /// ```
    pub fn display(&self, message: DisplayMessage) {
        if let Err(e) = self.to_display.send(message) {
            warn!("Lost connection to display ({e}). Requesting shutdown");
            self.to_main
                .send(MainMessage::Shutdown)
                .expect("Lost connection to the main thread");
        }
    }

    /// Sends a message to the renderer thread.
    ///
    /// # Example
    /// ```
    /// # use calloop::channel::channel;
    /// # use scape_shared::{Comms, RendererMessage};
    /// # let (to_main, _) = channel();
    /// # let (to_display, _) = channel();
    /// # let (to_renderer, renderer_channel) = channel();
    /// # let (to_input, _) = channel();
    /// # let (to_config, _) = channel();
    /// # let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
    /// comms.renderer(RendererMessage::Shutdown);
    /// assert!(matches!(renderer_channel.recv().unwrap(), RendererMessage::Shutdown));
    /// ```
    pub fn renderer(&self, message: RendererMessage) {
        if let Err(e) = self.to_renderer.send(message) {
            warn!("Lost connection to renderer ({e}). Requesting shutdown");
            self.to_main
                .send(MainMessage::Shutdown)
                .expect("Lost connection to the main thread");
        }
    }

    /// Sends a message to the input thread.
    ///
    /// # Example
    /// ```
    /// # use calloop::channel::channel;
    /// # use scape_shared::{Comms, InputMessage};
    /// # let (to_main, _) = channel();
    /// # let (to_display, _) = channel();
    /// # let (to_renderer, _) = channel();
    /// # let (to_input, input_channel) = channel();
    /// # let (to_config, _) = channel();
    /// # let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
    /// comms.input(InputMessage::Shutdown);
    /// assert!(matches!(input_channel.recv().unwrap(), InputMessage::Shutdown));
    /// ```
    pub fn input(&self, message: InputMessage) {
        if let Err(e) = self.to_input.send(message) {
            warn!("Lost connection to input ({e}). Requesting shutdown");
            self.to_main
                .send(MainMessage::Shutdown)
                .expect("Lost connection to the main thread");
        }
    }

    /// Sends a message to the config thread.
    ///
    /// # Example
    /// ```
    /// # use calloop::channel::channel;
    /// # use scape_shared::{Comms, ConfigMessage};
    /// # let (to_main, _) = channel();
    /// # let (to_display, _) = channel();
    /// # let (to_renderer, _) = channel();
    /// # let (to_input, _) = channel();
    /// # let (to_config, config_channel) = channel();
    /// # let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
    /// comms.input(ConfigMessage::Shutdown);
    /// assert!(matches!(config_channel.recv().unwrap(), ConfigMessage::Shutdown));
    /// ```
    pub fn config(&self, message: ConfigMessage) {
        if let Err(e) = self.to_config.send(message) {
            warn!("Lost connection to config ({e}). Requesting shutdown");
            self.to_main
                .send(MainMessage::Shutdown)
                .expect("Lost connection to the main thread");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calloop::channel::channel;

    #[test]
    #[should_panic]
    fn to_main_panics_on_lost_connection() {
        let (to_main, main_channel) = channel();
        let (to_display, _) = channel();
        let (to_renderer, _) = channel();
        let (to_input, _) = channel();
        let (to_config, _) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channel to the main thread
        drop(main_channel);

        comms.main(MainMessage::Shutdown);
    }

    #[test]
    fn to_display_sends_shutdown_to_main_on_lost_connection_to_display() {
        let (to_main, main_channel) = channel();
        let (to_display, display_channel) = channel();
        let (to_renderer, _) = channel();
        let (to_input, _) = channel();
        let (to_config, _) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channel to the display thread
        drop(display_channel);

        comms.display(DisplayMessage::Shutdown);
        assert!(matches!(
            main_channel.recv().unwrap(),
            MainMessage::Shutdown
        ));
    }

    #[test]
    #[should_panic]
    fn to_display_panics_on_lost_connection_to_display_and_main() {
        let (to_main, main_channel) = channel();
        let (to_display, display_channel) = channel();
        let (to_renderer, _) = channel();
        let (to_input, _) = channel();
        let (to_config, _) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channels to the display and main threads
        drop(display_channel);
        drop(main_channel);

        comms.display(DisplayMessage::Shutdown);
    }

    #[test]
    fn to_renderer_sends_shutdown_to_main_on_lost_connection_to_renderer() {
        let (to_main, main_channel) = channel();
        let (to_display, _) = channel();
        let (to_renderer, renderer_channel) = channel();
        let (to_input, _) = channel();
        let (to_config, _) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channel to the renderer thread
        drop(renderer_channel);

        comms.renderer(RendererMessage::Shutdown);
        assert!(matches!(
            main_channel.recv().unwrap(),
            MainMessage::Shutdown
        ));
    }

    #[test]
    #[should_panic]
    fn to_renderer_panics_on_lost_connection_to_renderer_and_main() {
        let (to_main, main_channel) = channel();
        let (to_display, _) = channel();
        let (to_renderer, renderer_channel) = channel();
        let (to_input, _) = channel();
        let (to_config, _) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channels to the renderer and main threads
        drop(renderer_channel);
        drop(main_channel);

        comms.renderer(RendererMessage::Shutdown);
    }

    #[test]
    fn to_input_sends_shutdown_to_main_on_lost_connection_to_input() {
        let (to_main, main_channel) = channel();
        let (to_display, _) = channel();
        let (to_renderer, _) = channel();
        let (to_input, input_channel) = channel();
        let (to_config, _) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channel to the input thread
        drop(input_channel);

        comms.input(InputMessage::Shutdown);
        assert!(matches!(
            main_channel.recv().unwrap(),
            MainMessage::Shutdown
        ));
    }

    #[test]
    #[should_panic]
    fn to_input_panics_on_lost_connection_to_input_and_main() {
        let (to_main, main_channel) = channel();
        let (to_display, _) = channel();
        let (to_renderer, _) = channel();
        let (to_input, input_channel) = channel();
        let (to_config, _) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channels to the input and main threads
        drop(input_channel);
        drop(main_channel);

        comms.input(InputMessage::Shutdown);
    }

    #[test]
    fn to_config_sends_shutdown_to_main_on_lost_connection_to_config() {
        let (to_main, main_channel) = channel();
        let (to_display, _) = channel();
        let (to_renderer, _) = channel();
        let (to_input, _) = channel();
        let (to_config, config_channel) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channel to the config thread
        drop(config_channel);

        comms.config(ConfigMessage::Shutdown);
        assert!(matches!(
            main_channel.recv().unwrap(),
            MainMessage::Shutdown
        ));
    }

    #[test]
    #[should_panic]
    fn to_config_panics_on_lost_connection_to_config_and_main() {
        let (to_main, main_channel) = channel();
        let (to_display, _) = channel();
        let (to_renderer, _) = channel();
        let (to_input, _) = channel();
        let (to_config, config_channel) = channel();
        let comms = Comms::new(to_main, to_display, to_renderer, to_input, to_config);
        // Close the channels to the config and main threads
        drop(config_channel);
        drop(main_channel);

        comms.config(ConfigMessage::Shutdown);
    }
}
