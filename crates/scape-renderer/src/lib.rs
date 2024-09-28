use calloop::channel::Channel;
use scape_shared::{Comms, GlobalArgs, RendererMessage};

pub fn run(comms: Comms, channel: Channel<RendererMessage>, args: &GlobalArgs) {}
