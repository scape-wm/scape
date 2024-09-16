use crate::{args::GlobalArgs, dbus, egui::debug_ui::DebugState, state::BackendData, State};
use calloop::EventLoop;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use std::{thread, time::Duration};
use tracing::error;

pub fn run(args: &GlobalArgs) -> anyhow::Result<()> {
    let display = create_display()?;
    let mut event_loop = create_event_loop()?;
    let backend_data = create_backend_data(args, &mut event_loop, display.handle())?;

    let mut state = State::new(&display, &mut event_loop)?;
    state.load_config(args)?;
    state.init(display, backend_data)?;

    thread::spawn(move || {
        let _ = dbus::run_dbus_services();
    });

    run_loop(state, &mut event_loop)
}

fn create_display() -> anyhow::Result<Display<State>> {
    tracing::info!("Creating new display");
    let display = Display::new().inspect_err(|e| {
        tracing::error!(
            err = %e, "Unable to create display. libwayland-server.so is probably missing",
        );
    })?;
    tracing::info!("Created display successfully");
    Ok(display)
}

fn create_event_loop() -> anyhow::Result<EventLoop<'static, State>> {
    tracing::info!("Creating new event loop");
    let event_loop = EventLoop::try_new().inspect_err(|e| {
        tracing::error!(err = %e, "Unable to create event loop");
    })?;
    tracing::info!("Created event loop successfully");
    Ok(event_loop)
}

fn create_backend_data(
    args: &GlobalArgs,
    event_loop: &mut EventLoop<'static, State>,
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

fn run_loop(mut state: State, event_loop: &mut EventLoop<State>) -> anyhow::Result<()> {
    tracing::info!("Starting main loop");
    event_loop.run(Some(Duration::from_millis(100)), &mut state, |state| {
        // TODO: Only refresh spaces that are active
        for space in state.spaces.values_mut() {
            space.refresh();
        }
        state.popups.cleanup();
        if let Err(e) = state.display_handle.flush_clients() {
            error!(err = %e, "Unable to flush clients");
        }

        if let Some(debug_ui) = &state.debug_ui {
            let needs_redraw = debug_ui
                .to_owned()
                .update_debug_ui(DebugState::from(&*state));

            if needs_redraw {
                state.backend_data.schedule_render();
            }
        }
    })?;

    Ok(())
}
