use calloop::channel::Channel;
use scape_shared::{Comms, GlobalArgs, InputMessage};

pub fn run(comms: Comms, channel: Channel<InputMessage>, args: &GlobalArgs) {}
