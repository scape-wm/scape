//! The input module is responsible for handling keyboard, mouse and touch events from the user.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use calloop::{LoopHandle, LoopSignal};
use input::start_input;
use scape_shared::{
    CallbackRef, Comms, GlobalArgs, InputMessage, MessageRunner, Mods, RendererMessage,
};
use seat::start_seat_session;
use xkbcommon::xkb::{self, Keycode, Keymap, Keysym};

mod input;
mod keyboard;
mod keymap;
mod seat;

/// Holds the state of the input module
pub struct InputState {
    comms: Comms,
    shutting_down: bool,
    loop_handle: LoopHandle<'static, InputState>,
    keyboards: Vec<Device>,
    keyboard_state: KeyboardState,
    seat_session: LibSeatSession,
    libinput_context: Libinput,
    tab_index: usize,
    keymaps: HashMap<Mods, HashMap<Keysym, CallbackRef>>,
    suppressed_keys: Vec<Keysym>,
}

impl MessageRunner for InputState {
    type Message = InputMessage;

    fn new(
        comms: Comms,
        loop_handle: LoopHandle<'static, InputState>,
        _args: &GlobalArgs,
    ) -> anyhow::Result<Self> {
        let keyboard_state = KeyboardState::new().context("Unable to create keyboard state")?;
        let seat_session =
            start_seat_session(loop_handle.clone()).context("Unable to start seat session")?;
        comms.renderer(RendererMessage::SeatSessionCreated {
            seat_name: seat_session.seat(),
        });
        let libinput_context = start_input(loop_handle.clone(), seat_session.clone())
            .context("Unable to start libinput")?;

        Ok(Self {
            comms,
            shutting_down: false,
            loop_handle,
            keyboards: Vec::new(),
            keyboard_state,
            seat_session,
            libinput_context,
            tab_index: 0,
            keymaps: HashMap::new(),
            suppressed_keys: Vec::new(),
        })
    }

    fn handle_message(&mut self, msg: InputMessage) -> anyhow::Result<()> {
        match msg {
            InputMessage::Shutdown => {
                self.shutting_down = true;
            }
            InputMessage::Keymap {
                key_name,
                mods,
                callback,
            } => {
                self.keymap(key_name, mods, callback);
            }
            InputMessage::OpenFileInSessionForRenderer { path } => {
                let fd = self.seat_session.open(
                    &path,
                    OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
                )?;
                self.comms
                    .renderer(RendererMessage::FileOpenedInSession { path, fd })
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
    // TODO: Think of using a more efficient data structure for this,
    // since there are usually only a few keys pressed at a time
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
