//! Wayland compositor for efficient workflows

#![warn(missing_docs)]

use anyhow::Context;
use calloop::{
    channel::{channel, Channel, Sender},
    timer::{TimeoutAction, Timer},
    EventLoop, LoopHandle,
};
use scape_shared::{
    get_global_args, Comms, DisplayMessage, GlobalArgs, InputMessage, MainMessage, RendererMessage,
};
use std::{
    panic::UnwindSafe,
    thread::{self, JoinHandle},
    time::Duration,
};
use tracing::{error, info, span, warn, Level};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

#[cfg(feature = "profile-with-tracy")]
#[global_allocator]
static GLOBAL: profiling::tracy_client::ProfiledAllocator<std::alloc::System> =
    profiling::tracy_client::ProfiledAllocator::new(std::alloc::System, 10);

#[cfg(feature = "profiling")]
fn setup_profiling() {
    #[cfg(feature = "profile-with-tracy")]
    profiling::tracy_client::Client::start();

    profiling::register_thread!("Main Thread");
}

fn setup_logging(log_file: Option<&str>) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let builder = EnvFilter::builder();
        #[cfg(feature = "debug")]
        let builder = builder.with_default_directive(LevelFilter::DEBUG.into());
        #[cfg(not(feature = "debug"))]
        let builder = builder.with_default_directive(LevelFilter::INFO.into());
        builder.parse_lossy("")
    });

    let log_builder = tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(env_filter);

    if let Some(log_file) = log_file {
        log_builder
            .with_writer(
                std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(log_file)
                    .unwrap(),
            )
            .init();
    } else {
        log_builder.init();
    }
}

fn main() -> anyhow::Result<()> {
    let args = Box::leak(Box::new(get_global_args()));

    setup_logging(args.log_file.as_deref());
    #[cfg(feature = "profiling")]
    setup_profiling();

    start_app(args)
}

struct MainData {
    loop_handle: LoopHandle<'static, MainData>,
    comms: Comms,
    input_join_handle: JoinHandle<()>,
    display_join_handle: JoinHandle<()>,
    renderer_join_handle: JoinHandle<()>,
    shutting_down: bool,
    force_shutting_down: bool,
}

impl MainData {
    fn new(
        loop_handle: LoopHandle<'static, MainData>,
        comms: Comms,
        input_join_handle: JoinHandle<()>,
        display_join_handle: JoinHandle<()>,
        renderer_join_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            loop_handle,
            comms,
            input_join_handle,
            display_join_handle,
            renderer_join_handle,
            shutting_down: false,
            force_shutting_down: false,
        }
    }
}

fn start_app(args: &'static GlobalArgs) -> anyhow::Result<()> {
    let (to_main, main_channel) = channel();
    let (to_display, display_channel) = channel();
    let (to_renderer, renderer_channel) = channel();
    let (to_input, input_channel) = channel();
    let comms = Comms::new(to_main.clone(), to_display, to_renderer, to_input);

    let mut event_loop = EventLoop::<MainData>::try_new().context("Unable to create event loop")?;
    let signal = event_loop.get_signal();
    let loop_handle = event_loop.handle();

    loop_handle
        .insert_source(main_channel, |event, _, data| match event {
            calloop::channel::Event::Msg(msg) => match msg {
                MainMessage::Shutdown => {
                    if !data.shutting_down {
                        data.shutting_down = true;
                        data.comms.input(InputMessage::Shutdown);
                        data.comms.display(DisplayMessage::Shutdown);
                        data.comms.renderer(RendererMessage::Shutdown);
                        // Force shutdown after some time
                        if let Err(e) = data.loop_handle.insert_source(
                            Timer::from_duration(Duration::from_millis(1000)),
                            |_, _, data| {
                                info!("Force shutdown timeout reached. Shutting down now");
                                data.force_shutting_down = true;
                                TimeoutAction::Drop
                            },
                        ) {
                            warn!(err = ?e, "Unable to insert timer to force shutdown. Shutting down now");
                            data.force_shutting_down = true;
                        }
                    }
                }
            },
            calloop::channel::Event::Closed => (),
        })
        .unwrap();

    let input_join_handle = run_thread(
        comms.clone(),
        to_main.clone(),
        String::from("input"),
        scape_input::run,
        input_channel,
        args,
    )
    .context("Unable to run input module")?;
    let display_join_handle = run_thread(
        comms.clone(),
        to_main.clone(),
        String::from("renderer"),
        scape_renderer::run,
        renderer_channel,
        args,
    )
    .context("Unable to run renderer module")?;
    let renderer_join_handle = run_thread(
        comms.clone(),
        to_main.clone(),
        String::from("display"),
        scape_display::run,
        display_channel,
        args,
    )
    .context("Unable to run display module")?;

    let mut data = MainData::new(
        loop_handle,
        comms,
        input_join_handle,
        display_join_handle,
        renderer_join_handle,
    );
    event_loop
        .run(None, &mut data, |data| {
            if data.shutting_down
                && data.input_join_handle.is_finished()
                && data.display_join_handle.is_finished()
                && data.renderer_join_handle.is_finished()
                || data.force_shutting_down
            {
                signal.stop();
            }
        })
        .context("Unable to run main loop")?;

    Ok(())
}

fn run_thread<F, T>(
    comms: Comms,
    to_main: Sender<MainMessage>,
    name: String,
    runner: F,
    channel: Channel<T>,
    args: &'static GlobalArgs,
) -> anyhow::Result<thread::JoinHandle<()>>
where
    F: FnOnce(Comms, Channel<T>, &GlobalArgs) -> anyhow::Result<()> + Send + UnwindSafe + 'static,
    T: Send + 'static,
{
    let join_handle = thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            let result = std::panic::catch_unwind(move || runner(comms, channel, args));
            let span = span!(Level::INFO, "scape", thread_name = name);
            let _guard = span.enter();
            match result {
                Ok(r) => {
                    error!(result = ?r, "Thread exited without panic");
                }
                Err(err) => {
                    if let Some(err) = err.downcast_ref::<&str>() {
                        error!(?err, "Thread panicked");
                    } else if let Some(err) = err.downcast_ref::<String>() {
                        error!(?err, "Thread panicked");
                    } else {
                        error!("Thread panicked");
                    }
                }
            }
            info!("Sending shutdown signal to main, because thread is about to exit");

            if let Err(err) = to_main.send(MainMessage::Shutdown) {
                warn!(%err, "Unable to send shutdown signal to main");
            }
        })
        .context("Unable to spawn thread")?;

    Ok(join_handle)
}
