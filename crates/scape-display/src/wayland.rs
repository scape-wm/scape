use crate::{
    dbus, egui::debug_ui::DebugState, state::BackendData, workspace_window::WorkspaceWindow, State,
};
use anyhow::Context;
use calloop::{channel::Channel, EventLoop};
use scape_shared::{Comms, ConfigMessage, DisplayMessage, GlobalArgs, MainMessage};
use smithay::{
    reexports::wayland_server::{Display, DisplayHandle},
    utils::SERIAL_COUNTER,
};
use std::{thread, time::Duration};
use tracing::error;

pub fn run(
    comms: Comms,
    channel: Channel<DisplayMessage>,
    args: &GlobalArgs,
) -> anyhow::Result<()> {
    let display = create_display()?;
    let mut event_loop = create_event_loop()?;
    let backend_data = create_backend_data(args, &mut event_loop, display.handle(), &comms)?;

    let mut state = State::new(&display, &mut event_loop, comms)?;
    state.load_config(args)?;
    state.init(display, backend_data)?;

    event_loop
        .handle()
        .insert_source(channel, |event, _, state| match event {
            calloop::channel::Event::Msg(msg) => handle_display_message(state, msg),
            calloop::channel::Event::Closed => state.comms.main(MainMessage::Shutdown),
        });

    // thread running dbus services
    thread::spawn(move || {
        let _ = dbus::run_dbus_services();
    });

    run_loop(state, &mut event_loop)
}

fn create_display() -> anyhow::Result<Display<State>> {
    tracing::info!("Creating new display");
    let display = Display::new()
        .context("Unable to create display. libwayland-server.so is probably missing")?;
    tracing::info!("Created display successfully");
    Ok(display)
}

fn create_event_loop() -> anyhow::Result<EventLoop<'static, State>> {
    tracing::info!("Creating new event loop");
    let event_loop = EventLoop::try_new().context("Unable to create event loop")?;
    tracing::info!("Created event loop successfully");
    Ok(event_loop)
}

fn create_backend_data(
    args: &GlobalArgs,
    event_loop: &mut EventLoop<'static, State>,
    display_handle: DisplayHandle,
    comms: &Comms,
) -> anyhow::Result<BackendData> {
    if args.winit_backend {
        tracing::info!("Starting with winit backend");
        crate::winit::init_winit(display_handle, event_loop)
    } else {
        tracing::info!("Starting on a tty using udev");
        crate::udev::init_udev(event_loop, comms)
    }
}

fn handle_display_message(state: &mut State, message: DisplayMessage) {
    match message {
        DisplayMessage::Shutdown => {
            state.loop_signal.stop();
            state.loop_signal.wakeup();
        }
        DisplayMessage::KeyboardInput {
            keycode,
            key_state,
            modifiers_changed,
            time,
        } => {
            state
                .seat
                .as_mut()
                .unwrap()
                .get_keyboard()
                .unwrap()
                .input_forward(
                    state,
                    keycode,
                    key_state,
                    SERIAL_COUNTER.next_serial(),
                    time,
                    modifiers_changed,
                );
        }
        DisplayMessage::Action(action) => {
            state.execute(action);
        }
        DisplayMessage::SetZones(zones) => {
            state.set_zones(zones);
        }
        DisplayMessage::MoveCurrentWindowToZone(zone_name) => {
            // TODO: Handle multiple spaces
            let (space_name, _) = state.spaces.iter().next().unwrap();
            let keyboard = state.seat.as_ref().unwrap().get_keyboard().unwrap();
            if let Some(focus) = keyboard.current_focus() {
                if let Ok(window) = WorkspaceWindow::try_from(focus) {
                    state.place_window(
                        &space_name.to_owned(),
                        &window,
                        false,
                        Some(&zone_name),
                        true,
                    );
                }
            }
        }
        DisplayMessage::VtSwitch(vt) => {
            if let Err(err) = state.backend_data.switch_vt(vt) {
                error!(vt, "Error switching vt: {}", err);
            }
        }
        DisplayMessage::FocusOrSpawn {
            app_id,
            command,
            args,
        } => {
            if !state.focus_window_by_app_id(app_id) {
                state.comms.config(ConfigMessage::Spawn(command, args));
            }
        }
        DisplayMessage::CloseCurrentWindow => {
            // TODO: Handle multiple spaces
            let (_, space) = state.spaces.iter_mut().next().unwrap();
            if let Some(window) = space.elements().last().cloned() {
                if window.close() {
                    space.unmap_elem(&window);
                }
            }
        }
        DisplayMessage::AddWindowRule(window_rule) => {
            state.add_window_rule(window_rule);
        }
        DisplayMessage::ToggleDebugUi => {
            state.toggle_debug_ui();
        }
        DisplayMessage::StartVideoStream => {
            state.start_video_stream();
        }
        DisplayMessage::SetLayout { spaces } => {
            state.set_layout(spaces);
        }
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
