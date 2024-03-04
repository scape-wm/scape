use crate::State;
use smithay::{
    delegate_keyboard_shortcuts_inhibit,
    wayland::keyboard_shortcuts_inhibit::{
        KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
    },
};

impl KeyboardShortcutsInhibitHandler for State {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.keyboard_shortcuts_inhibit_state
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        // Just grant the wish for everyone
        inhibitor.activate();
    }
}

delegate_keyboard_shortcuts_inhibit!(State);
