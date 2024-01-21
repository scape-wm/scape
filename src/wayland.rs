use crate::{
    args::GlobalArgs,
    render,
    state::BackendData,
    udev::{render, RENDER_SCHEDULE_COUNTER},
    State,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    EventLoop, LoopHandle,
};
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use std::{ffi::OsString, time::Duration};
use tracing::error;

pub fn run(args: &GlobalArgs) -> anyhow::Result<()> {
    let display = create_display()?;
    let mut event_loop = create_event_loop()?;
    let backend_data = create_backend_data(args, &mut event_loop, display.handle())?;

    let mut state = State::init(display, backend_data, &mut event_loop)?;

    start_xwayland(&mut state, event_loop.handle());
    run_loop(state, &mut event_loop, !args.winit_backend)
}

fn create_display() -> anyhow::Result<Display<State>> {
    tracing::info!("Creating new display");
    let display = Display::new().map_err(|e| {
        tracing::error!(
            "Unable to create display. libwayland-server.so is probably missing: {}",
            e
        );
        e
    })?;
    tracing::info!("Created display successfully");
    Ok(display)
}

fn create_event_loop() -> anyhow::Result<EventLoop<'static, State>> {
    tracing::info!("Creating new event loop");
    let event_loop = EventLoop::try_new().map_err(|e| {
        tracing::error!("Unable to create event loop: {}", e);
        e
    })?;
    tracing::info!("Created event loop successfully");
    Ok(event_loop)
}

fn create_backend_data(
    args: &GlobalArgs,
    event_loop: &mut EventLoop<State>,
    display_handle: DisplayHandle,
) -> anyhow::Result<BackendData> {
    if args.winit_backend {
        tracing::info!("Starting with winit backend");
        crate::winit::init_winit(display_handle, event_loop)
    } else {
        tracing::info!("Starting on a tty using udev");
        crate::udev::init_udev(event_loop)
    }
}

fn start_xwayland(state: &State, loop_handle: LoopHandle<State>) {
    if let Err(e) = state.xwayland.start(
        loop_handle.clone(),
        None,
        std::iter::empty::<(OsString, OsString)>(), // TODO: Add configuration option
        true,
        |_| {},
    ) {
        error!("Failed to start XWayland: {}", e);
    }
}

fn run_loop(
    mut state: State,
    event_loop: &mut EventLoop<State>,
    is_udev: bool,
) -> anyhow::Result<()> {
    tracing::info!("Starting main loop");
    event_loop.run(Some(Duration::from_millis(100)), &mut state, |state| {
        state.space.refresh();
        state.popups.cleanup();
        if let Err(e) = state.display_handle.flush_clients() {
            error!("Unable to flush clients: {e}");
        }

        // if !is_udev {
        //     return;
        // }
        //
        // let val = RENDER_SCHEDULE_COUNTER.load(std::sync::atomic::Ordering::SeqCst);
        // tracing::warn!("current val is {}", val);
        // if val <= 0 {
        //     state
        //         .loop_handle
        //         .insert_source(
        //             Timer::from_duration(Duration::from_secs(10)),
        //             move |_, _, state| {
        //                 if RENDER_SCHEDULE_COUNTER.load(std::sync::atomic::Ordering::SeqCst) <= 0 {
        //                     render(state, state.last_node.unwrap(), None);
        //                 }
        //                 TimeoutAction::Drop
        //             },
        //         )
        //         .unwrap();
        // }
    })?;

    Ok(())
}
