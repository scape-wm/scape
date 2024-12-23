//! The input module is responsible for handling keyboard, mouse and touch events from the user.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use anyhow::bail;
use calloop::{LoopHandle, LoopSignal};
use scape_shared::{CallbackRef, Comms, GlobalArgs, InputMessage, MessageRunner};
use smithay::{
    backend::{
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session},
    },
    input::keyboard::{LedMapping, LedState, ModifiersState},
    reexports::input::{Device, DeviceCapability, Libinput},
};
use xkbcommon::xkb::{self, Keycode, Keymap, Keysym};

mod keyboard;

/// Holds the state of the input module
pub struct InputState {
    comms: Comms,
    shutting_down: bool,
    loop_handle: LoopHandle<'static, InputState>,
    keyboards: Vec<Device>,
    keyboard_state: KeyboardState,
    libinput_context: Option<Libinput>,
    tab_index: usize,
    key_maps: HashMap<ModifiersState, HashMap<Keysym, CallbackRef>>,
    suppressed_keys: Vec<Keysym>,
}

impl MessageRunner for InputState {
    type Message = InputMessage;

    fn new(
        comms: Comms,
        loop_handle: LoopHandle<'static, InputState>,
        _args: &GlobalArgs,
    ) -> anyhow::Result<Self> {
        let keyboard_state = KeyboardState::new()?;

        Ok(Self {
            comms,
            shutting_down: false,
            loop_handle,
            keyboards: Vec::new(),
            keyboard_state,
            libinput_context: None,
            tab_index: 0,
            key_maps: HashMap::new(),
            suppressed_keys: Vec::new(),
        })
    }

    fn handle_message(&mut self, msg: InputMessage) -> anyhow::Result<()> {
        match msg {
            InputMessage::Shutdown => {
                self.shutting_down = true;
            }
            InputMessage::SeatSessionCreated { session } => {
                // A new seat session has been created, we can now initialize the input
                let seat_name = session.seat();
                let mut libinput_context = Libinput::new_with_udev::<
                    LibinputSessionInterface<LibSeatSession>,
                >(session.into());
                if libinput_context.udev_assign_seat(&seat_name).is_err() {
                    bail!("Failed to assign seat to libinput context");
                }

                let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
                self.libinput_context = Some(libinput_context);
                self.loop_handle
                    .insert_source(libinput_backend, move |event, _, state| {
                        state.handle_input_event(event)
                    })
                    .unwrap();
            }
            InputMessage::SeatSessionSuspended => {
                if let Some(libinput_context) = &self.libinput_context {
                    libinput_context.suspend();
                }
            }
            InputMessage::SeatSessionResumed => {
                if let Some(libinput_context) = &mut self.libinput_context {
                    if libinput_context.resume().is_err() {
                        anyhow::bail!("Failed to resume libinput context");
                    }
                }
            }
        }
        Ok(())
    }

    fn on_dispatch_wait(&mut self, signal: &LoopSignal) {
        if self.shutting_down {
            signal.stop();
        }
    }
}

impl InputState {
    fn handle_input_event(&mut self, event: InputEvent<LibinputInputBackend>) {
        match event {
            InputEvent::DeviceAdded { mut device } => {
                if device.has_capability(DeviceCapability::Keyboard) {
                    device.led_update(self.keyboard_state.led_state.into());
                    self.keyboards.push(device);
                }
            }
            InputEvent::DeviceRemoved { device } => {
                if device.has_capability(DeviceCapability::Keyboard) {
                    self.keyboards.retain(|item| item != &device);
                }
            }
            InputEvent::Keyboard { event } => {
                self.handle_keyboard_event::<LibinputInputBackend>(event)
            }
            InputEvent::PointerMotion { event } => todo!(),
            InputEvent::PointerMotionAbsolute { event } => todo!(),
            InputEvent::PointerButton { event } => todo!(),
            InputEvent::PointerAxis { event } => todo!(),
            InputEvent::GestureSwipeBegin { event } => todo!(),
            InputEvent::GestureSwipeUpdate { event } => todo!(),
            InputEvent::GestureSwipeEnd { event } => todo!(),
            InputEvent::GesturePinchBegin { event } => todo!(),
            InputEvent::GesturePinchUpdate { event } => todo!(),
            InputEvent::GesturePinchEnd { event } => todo!(),
            InputEvent::GestureHoldBegin { event } => todo!(),
            InputEvent::GestureHoldEnd { event } => todo!(),
            InputEvent::TouchDown { event } => todo!(),
            InputEvent::TouchMotion { event } => todo!(),
            InputEvent::TouchUp { event } => todo!(),
            InputEvent::TouchCancel { event } => todo!(),
            InputEvent::TouchFrame { event } => todo!(),
            InputEvent::TabletToolAxis { event } => todo!(),
            InputEvent::TabletToolProximity { event } => todo!(),
            InputEvent::TabletToolTip { event } => todo!(),
            InputEvent::TabletToolButton { event } => todo!(),
            InputEvent::SwitchToggle { event } => todo!(),
            InputEvent::Special(_) => todo!(),
        }
    }
}

struct KeyboardState {
    xkb_state: xkb::State,
    led_mapping: LedMapping,
    led_state: LedState,
    pressed_keys: HashSet<Keycode>,
    mods_state: ModifiersState,
}

impl KeyboardState {
    fn new() -> anyhow::Result<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = Keymap::new_from_names(
            &context,
            "",
            "",
            "de",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or(anyhow::anyhow!("Failed to create xkb state"))?;
        let xkb_state = xkb::State::new(&keymap);
        let led_mapping = LedMapping::from_keymap(&keymap);
        let led_state = LedState::from_state(&xkb_state, &led_mapping);

        Ok(Self {
            xkb_state,
            led_mapping,
            led_state,
            pressed_keys: HashSet::new(),
            mods_state: ModifiersState::default(),
        })
    }
}
