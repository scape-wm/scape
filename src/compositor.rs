use crate::{args::GlobalArgs, state::BackendData, CalloopData, ScapeState};
use calloop::{EventLoop, LoopHandle};
use smithay::reexports::wayland_server::Display;
use std::{ffi::OsString, time::Duration};

pub fn run(args: &GlobalArgs) -> anyhow::Result<()> {
    let mut display = create_display()?;
    let mut event_loop = create_event_loop()?;
    let backend_data = create_backend_data(args, &mut event_loop, &mut display)?;

    let mut state = ScapeState::init(&mut display, backend_data, &mut event_loop)?;

    start_xwayland(&mut state, event_loop.handle());
    run_loop(state, &mut event_loop, display)
}

fn create_display() -> anyhow::Result<Display<ScapeState>> {
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

fn create_event_loop() -> anyhow::Result<EventLoop<'static, CalloopData>> {
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
    event_loop: &mut EventLoop<CalloopData>,
    display: &mut Display<ScapeState>,
) -> anyhow::Result<BackendData> {
    if args.winit_backend {
        tracing::info!("Starting with winit backend");
        crate::winit::init_winit(event_loop, display)
    } else {
        tracing::info!("Starting on a tty using udev");
        crate::udev::init_udev(event_loop, display)
    }
}

fn start_xwayland(state: &ScapeState, loop_handle: LoopHandle<CalloopData>) {
    if let Err(e) = state.xwayland.start(
        loop_handle.clone(),
        None,
        std::iter::empty::<(OsString, OsString)>(), // TODO: Add configuration option
        true,
        |_| {},
    ) {
        tracing::error!("Failed to start XWayland: {}", e);
    }
}

fn run_loop(
    state: ScapeState,
    event_loop: &mut EventLoop<CalloopData>,
    display: Display<ScapeState>,
) -> anyhow::Result<()> {
    let mut calloop_data = CalloopData { state, display };
    tracing::info!("Starting main loop");
    event_loop.run(
        Some(Duration::from_millis(100)),
        &mut calloop_data,
        |data| {
            data.state.space.refresh();
            data.state.popups.cleanup();
            data.display.flush_clients().unwrap();
        },
    )?;

    Ok(())
}
