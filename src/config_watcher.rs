use std::path::PathBuf;

use calloop::{
    channel::{sync_channel, Channel, ChannelError},
    EventSource,
};
use notify::{
    event::ModifyKind, recommended_watcher, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use tracing::error;

pub struct ConfigWatcher {
    channel: Channel<PathBuf>,
    _watcher: RecommendedWatcher,
}

impl ConfigWatcher {
    pub fn new(path: PathBuf) -> Self {
        let (sender, channel) = sync_channel(64);

        let mut watcher =
            recommended_watcher(move |event_res: Result<notify::Event, notify::Error>| {
                match &event_res {
                    Ok(event) => {
                        match &event.kind {
                            EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_)) => {
                                // No change to file contents
                                return;
                            }
                            _ => {}
                        }

                        for path in &event.paths {
                            sender.send(path.to_owned()).unwrap();
                        }
                    }
                    Err(err) => {
                        error!(?err, "File watcher had error")
                    }
                }
            })
            .unwrap();

        if let Err(err) = watcher.watch(path.as_path(), RecursiveMode::NonRecursive) {
            error!(?err, "Unable to setup config file change watcher");
        }

        Self {
            channel,
            _watcher: watcher,
        }
    }
}

impl EventSource for ConfigWatcher {
    type Event = PathBuf;
    type Metadata = ();
    type Ret = ();
    type Error = ChannelError;

    fn process_events<F>(
        &mut self,
        readiness: calloop::Readiness,
        token: calloop::Token,
        mut callback: F,
    ) -> Result<calloop::PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.channel
            .process_events(readiness, token, |event, ()| match event {
                calloop::channel::Event::Msg(msg) => callback(msg, &mut ()),
                calloop::channel::Event::Closed => {}
            })
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> Result<(), calloop::Error> {
        self.channel.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> Result<(), calloop::Error> {
        self.channel.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> Result<(), calloop::Error> {
        self.channel.unregister(poll)
    }
}
