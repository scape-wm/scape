use calloop::channel::Sender;
use tracing::warn;

use crate::{DisplayMessage, InputMessage, MainMessage, RendererMessage};

#[derive(Clone)]
pub struct Comms {
    to_main: Sender<MainMessage>,
    to_display: Sender<DisplayMessage>,
    to_renderer: Sender<RendererMessage>,
    to_input: Sender<InputMessage>,
}

impl Comms {
    pub fn new(
        to_main: Sender<MainMessage>,
        to_display: Sender<DisplayMessage>,
        to_renderer: Sender<RendererMessage>,
        to_input: Sender<InputMessage>,
    ) -> Self {
        Comms {
            to_main,
            to_display,
            to_renderer,
            to_input,
        }
    }

    pub fn main(&self, message: MainMessage) {
        self.to_main
            .send(message)
            .expect("Lost connection to the main thread");
    }

    pub fn display(&self, message: DisplayMessage) {
        if let Err(e) = self.to_display.send(message) {
            warn!(err = %e, "Lost connection to display. Requesting shutdown");
            self.to_main
                .send(MainMessage::Shutdown)
                .expect("Lost connection to the main thread");
        }
    }

    pub fn renderer(&self, message: RendererMessage) {
        if let Err(e) = self.to_renderer.send(message) {
            warn!(err = %e, "Lost connection to renderer. Requesting shutdown");
            self.to_main
                .send(MainMessage::Shutdown)
                .expect("Lost connection to the main thread");
        }
    }

    pub fn input(&self, message: InputMessage) {
        if let Err(e) = self.to_input.send(message) {
            warn!(err = %e, "Lost connection to input. Requesting shutdown");
            self.to_main
                .send(MainMessage::Shutdown)
                .expect("Lost connection to the main thread");
        }
    }
}
