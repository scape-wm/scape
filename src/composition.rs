use smithay::{
    desktop::{layer_map_for_output, Space},
    utils::{Logical, Point, Rectangle},
};
use tracing::warn;

use crate::shell::ApplicationWindow;

#[derive(Debug)]
pub struct Zone {
    pub geometry: Rectangle<i32, Logical>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WindowPosition {
    New,
    Mid,
    Left,
    Right,
}

pub fn place_window(
    space: &mut Space<ApplicationWindow>,
    pointer_location: Point<f64, Logical>,
    window: &ApplicationWindow,
    activate: bool,
    window_position: WindowPosition,
) -> Rectangle<i32, Logical> {
    let (size, position) = match window_position {
        WindowPosition::New => ((2560, 1440), (100, 100)),
        WindowPosition::Mid => ((2560, 1440), (2560 / 2 + 1, 0)),
        WindowPosition::Left => ((2560 / 2, 1440), (0, 0)),
        WindowPosition::Right => ((2560 / 2, 1440), (2560 + 2560 / 2 + 1, 0)),
    };
    let output = space
        .output_under(pointer_location)
        .next()
        .or_else(|| space.outputs().next())
        .cloned();
    let output_geometry = output
        .and_then(|o| {
            let geo = space.output_geometry(&o)?;
            let map = layer_map_for_output(&o);
            let zone = map.non_exclusive_zone();
            Some(Rectangle::from_loc_and_size(geo.loc + zone.loc, zone.size))
        })
        .unwrap_or_else(|| Rectangle::from_loc_and_size((0, 0), (800, 800)));

    // set the initial toplevel bounds
    match window {
        ApplicationWindow::Wayland(window) => {
            window.toplevel().with_pending_state(|state| {
                state.bounds = Some(output_geometry.size);
                // state.bounds = Some(size.into());
                state.size = Some(size.into());
            });
            if window_position != WindowPosition::New {
                window.toplevel().send_pending_configure();
            }
        }
        ApplicationWindow::X11(window) => {
            if window_position != WindowPosition::New {
                window
                    .configure(Some(Rectangle::from_loc_and_size(position, size)))
                    .unwrap();
            }
        }
    }

    space.map_element(window.clone(), position, activate);
    Rectangle::from_loc_and_size(position, size)
}
