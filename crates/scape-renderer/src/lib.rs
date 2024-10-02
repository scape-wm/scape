use calloop::channel::Channel;
use scape_shared::{Comms, GlobalArgs, RendererMessage};
use tracing::{span, Level};

pub fn run(
    _comms: Comms,
    _channel: Channel<RendererMessage>,
    _args: &GlobalArgs,
) -> anyhow::Result<()> {
    let span = span!(Level::ERROR, "renderer");
    let _guard = span.enter();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(10000));
    }
}
