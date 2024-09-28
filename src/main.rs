//! Wayland compositor for efficient workflows

#![warn(missing_docs)]

use anyhow::Context;
use calloop::{
    channel::{channel, Channel, Sender},
    EventLoop,
};
use scape_shared::{get_global_args, Comms, GlobalArgs, MainMessage};
use std::thread;
use tracing::warn;
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
        .compact()
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

#[derive(Default)]
struct MainData {
    shutting_down: bool,
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
                    data.shutting_down = true;
                }
            },
            calloop::channel::Event::Closed => (),
        })
        .unwrap();

    run_thread(
        comms.clone(),
        to_main.clone(),
        "input".to_string(),
        scape_input::run,
        input_channel,
        args,
    )
    .context("Unable to run input module")?;
    run_thread(
        comms.clone(),
        to_main.clone(),
        "renderer".to_string(),
        scape_renderer::run,
        renderer_channel,
        args,
    )
    .context("Unable to run renderer module")?;
    run_thread(
        comms.clone(),
        to_main.clone(),
        "display".to_string(),
        scape_display::run,
        display_channel,
        args,
    )
    .context("Unable to run display module")?;

    event_loop
        .run(None, &mut MainData::default(), |data| {
            if data.shutting_down {
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
) -> anyhow::Result<()>
where
    F: FnOnce(Comms, Channel<T>, &GlobalArgs) + Send + 'static,
    T: Send + 'static,
{
    thread::Builder::new()
        .name(name)
        .spawn(move || {
            runner(comms, channel, args);
            if let Err(err) = to_main.send(MainMessage::Shutdown) {
                warn!(%err, "Unable to send shutdown signal to main");
            }
        })
        .context("Unable to spawn thread")?;

    Ok(())
}
