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

/// Sets up logging with tracing. If the `log_file` is `Some`, the log messages will be written to
/// the file. Otherwise, they will be written to the standard output.
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
    // Get a static reference to the global args, so that they can be sent across threads
    let args = Box::leak(Box::new(get_global_args()));

    setup_logging(args.log_file.as_deref());
    #[cfg(feature = "profiling")]
    setup_profiling();

    run_app(args)
}

/// Represents the data for the main thread
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
    /// Creates a new instance of `MainData`
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

/// Starts the application by creating the needed channels and starting the necessary threads. The
/// main thread will wait for the other threads to finish before exiting.
fn run_app(args: &'static GlobalArgs) -> anyhow::Result<()> {
    // Create the channels for communication between the threads
    let (to_main, main_channel) = channel();
    let (to_display, display_channel) = channel();
    let (to_renderer, renderer_channel) = channel();
    let (to_input, input_channel) = channel();
    let comms = Comms::new(to_main.clone(), to_display, to_renderer, to_input);

    let mut event_loop = EventLoop::<MainData>::try_new().context("Unable to create event loop")?;
    let signal = event_loop.get_signal();
    let loop_handle = event_loop.handle();

    if let Err(e) = loop_handle
        .insert_source(main_channel, |event, _, data| match event {
            calloop::channel::Event::Msg(msg) => match msg {
                MainMessage::Shutdown => {
                    if !data.shutting_down {
                        data.shutting_down = true;
                        // Notify the other threads that the application is shutting down
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
        }) {
        anyhow::bail!("Unable to insert main channel into event loop: {}", e);
    }

    // Spawn the input thread
    let input_join_handle = run_thread(
        comms.clone(),
        to_main.clone(),
        String::from("input"),
        scape_input::run,
        input_channel,
        args,
    )
    .context("Unable to run input module")?;
    // Spawn the renderer thread
    let display_join_handle = run_thread(
        comms.clone(),
        to_main.clone(),
        String::from("renderer"),
        scape_renderer::run,
        renderer_channel,
        args,
    )
    .context("Unable to run renderer module")?;
    // Spawn the display thread
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

    // Run the main loop
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

/// Spawns a new thread and runs the given function in it, returning a handle to the newly created
/// thread. The spawned thread is wrapped in a panic handler to gracefully handle any panics that
/// might occur.
fn run_thread<F, T>(
    comms: Comms,
    to_main: Sender<MainMessage>,
    name: String,
    runner: F,
    channel: Channel<T>,
    args: &'static GlobalArgs,
) -> anyhow::Result<JoinHandle<()>>
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
                Ok(Ok(r)) => {
                    info!(result = ?r, "Thread exited normally");
                }
                Ok(Err(err)) => {
                    error!(result = ?err, "Thread exited with an error");
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

            // The thread should only exit if the main thread has already sent a shutdown signal,
            // but in case something is wrong, we send a shutdown signal to the main thread anyway.
            if let Err(err) = to_main.send(MainMessage::Shutdown) {
                warn!(%err, "Unable to send shutdown signal to main");
            }
        })
        .context("Unable to spawn thread")?;

    Ok(join_handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_thread_sends_shutdown_signal() {
        let (to_main, main_channel) = channel();
        let (to_display, display_channel) = channel();
        let (to_renderer, _) = channel();
        let (to_input, _) = channel();
        let comms = Comms::new(to_main.clone(), to_display, to_renderer, to_input);
        let args = Box::leak(Box::new(GlobalArgs::default()));

        let join_handle = run_thread(
            comms,
            to_main,
            String::from("test_thread"),
            |_, _, _| Ok(()),
            display_channel, // pick any channel, does not matter for this test
            args,
        );

        // Wait for the thread to finish
        join_handle.unwrap().join().unwrap();

        // Check if the main channel has received the shutdown signal
        assert!(matches!(
            main_channel.recv().unwrap(),
            MainMessage::Shutdown
        ));
        // No other messages should be received
        assert!(main_channel.try_recv().is_err());
    }

    #[test]
    fn run_thread_sends_shutdown_signal_on_panic() {
        let (to_main, main_channel) = channel();
        let (to_display, display_channel) = channel();
        let (to_renderer, _) = channel();
        let (to_input, _) = channel();
        let comms = Comms::new(to_main.clone(), to_display, to_renderer, to_input);
        let args = Box::leak(Box::new(GlobalArgs::default()));

        let join_handle = run_thread(
            comms,
            to_main,
            String::from("test_thread"),
            |_, _, _| panic!("Test panic"),
            display_channel, // pick any channel, does not matter for this test
            args,
        );

        // Wait for the thread to finish
        join_handle.unwrap().join().unwrap();

        // Check if the main channel has received the shutdown signal
        assert!(matches!(
            main_channel.recv().unwrap(),
            MainMessage::Shutdown
        ));
        // No other messages should be received
        assert!(main_channel.try_recv().is_err());
    }
}
