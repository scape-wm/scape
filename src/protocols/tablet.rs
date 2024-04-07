use crate::State;
use smithay::{
    backend::input::TabletToolDescriptor, delegate_tablet_manager,
    input::pointer::CursorImageStatus, wayland::tablet_manager::TabletSeatHandler,
};

impl TabletSeatHandler for State {
    fn tablet_tool_image(&mut self, _tool: &TabletToolDescriptor, image: CursorImageStatus) {
        // TODO: add tablet to seat and handle cursor event
        // TODO: tablet tools should have their own cursors
        self.cursor_status = image;
    }
}

delegate_tablet_manager!(State);
