use scape_shared::{Action, DisplayMessage};
use smithay::{
    backend::input::{Event, InputBackend, KeyState, KeyboardKeyEvent},
    input::keyboard::ModifiersState,
};
use xkbcommon::xkb::{self, Keysym};

use crate::InputData;

impl InputData {
    pub(crate) fn handle_keyboard_event<B: InputBackend>(&mut self, event: B::KeyboardKeyEvent) {
        let keyboard_state = &mut self.keyboard_state;

        let event_state = event.state();
        let key_code = event.key_code();
        let direction = match event_state {
            KeyState::Pressed => {
                keyboard_state.pressed_keys.insert(key_code);
                xkb::KeyDirection::Down
            }
            KeyState::Released => {
                keyboard_state.pressed_keys.remove(&key_code);
                xkb::KeyDirection::Up
            }
        };

        let changed_state_components = keyboard_state.xkb_state.update_key(key_code, direction);
        let modifiers_changed = changed_state_components != 0;
        if modifiers_changed {
            keyboard_state
                .mods_state
                .update_with(&keyboard_state.xkb_state);
        }

        let leds_changed = keyboard_state
            .led_state
            .update_with(&keyboard_state.xkb_state, &keyboard_state.led_mapping);
        if leds_changed {
            for keyboard in &mut self.keyboards {
                keyboard.led_update(keyboard_state.led_state.into());
            }
        }

        let modifiers = keyboard_state.mods_state;
        let keysym = keyboard_state.xkb_state.key_get_one_sym(key_code);
        if let Some(action) = self.keyboard_shortcut(modifiers, keysym) {
            self.suppressed_keys.push(keysym);
            self.comms.display(DisplayMessage::Action(action));
            return;
        }

        if event_state == KeyState::Released && self.suppressed_keys.contains(&keysym) {
            self.suppressed_keys.retain(|k| *k != keysym);
            return;
        }

        self.comms.display(DisplayMessage::KeyboardInput {
            keycode: key_code,
            key_state: event_state,
            modifiers_changed,
            time: Event::time_msec(&event),
        });
    }

    /// Check for keyboard shortcuts and return the corresponding action
    fn keyboard_shortcut(&mut self, modifiers: ModifiersState, keysym: Keysym) -> Option<Action> {
        if !modifiers.alt {
            self.tab_index = 0;
        }

        // TODO: Check for keyboard inhibitors
        // let inhibited = space
        //     .element_under(self.pointer_location())
        //     .and_then(|(window, _)| {
        //         let surface = window.wl_surface()?;
        //         self.seat
        //             .as_ref()?
        //             .keyboard_shortcuts_inhibitor_for_surface(&surface)
        //     })
        //     .map(|inhibitor| inhibitor.is_active())
        //     .unwrap_or(false);

        if modifiers.ctrl && modifiers.alt && keysym == Keysym::BackSpace {
            // ctrl+alt+backspace = quit
            Some(Action::Quit)
        } else if (xkb::keysyms::KEY_XF86Switch_VT_1..=xkb::keysyms::KEY_XF86Switch_VT_12)
            .contains(&keysym.raw())
        {
            // VTSwitch
            Some(Action::VtSwitch(
                (keysym.raw() - xkb::keysyms::KEY_XF86Switch_VT_1 + 1) as i32,
            ))
        } else if modifiers.alt && keysym == Keysym::Tab {
            self.tab_index += 1;
            Some(Action::Tab {
                index: self.tab_index,
            })
        } else {
            let maps = self.key_maps.get(&modifiers)?;
            let callback = maps.get(&keysym)?;
            Some(Action::Callback(*callback))
        }
    }
}
