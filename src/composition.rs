use crate::{application_window::ApplicationWindow, config::ConfigZone, state::ActiveSpace, State};
use smithay::{
    desktop::{layer_map_for_output, Space},
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER},
};
use tracing::info;

#[derive(Debug)]
pub struct Zone {
    pub name: String,
    pub geometry: Rectangle<i32, Logical>,
    pub default: bool,
}

impl From<ConfigZone> for Zone {
    fn from(value: ConfigZone) -> Self {
        Self {
            name: value.name,
            geometry: Rectangle::from_loc_and_size((value.x, value.y), (value.width, value.height)),
            default: value.default,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WindowPosition {
    New,
    Mid,
    Left,
    Right,
}

impl State {
    pub fn place_window(
        &mut self,
        space_name: &str,
        window: &ApplicationWindow,
        activate: bool,
        zone: Option<&str>,
        send_configure: bool,
    ) -> Rectangle<i32, Logical> {
        let pointer_location = self.pointer_location();
        let space = self.spaces.get_mut(space_name).unwrap();

        let (size, position) = if let Some(zone_name) = zone {
            let zone = &self.zones[zone_name];
            (zone.geometry.size, zone.geometry.loc)
        } else if let Some(default_zone_name) = &self.default_zone {
            let zone = &self.zones[default_zone_name];
            (zone.geometry.size, zone.geometry.loc)
        } else {
            ((2560, 1440).into(), (100, 100).into())
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
                if send_configure {
                    window.toplevel().send_pending_configure();
                }
            }
            ApplicationWindow::X11(window) => {
                if send_configure {
                    window
                        .configure(Some(Rectangle::from_loc_and_size(position, size)))
                        .unwrap();
                }
            }
        }

        space.map_element(window.clone(), position, activate);
        Rectangle::from_loc_and_size(position, size)
    }

    pub fn set_zones(&mut self, zones: Vec<Zone>) {
        self.zones.clear();
        for zone in zones {
            if zone.default {
                self.default_zone = Some(zone.name.clone());
            }
            self.zones.insert(zone.name.clone(), zone);
        }
    }

    pub fn focus_window_by_app_id(&mut self, app_id: String) -> bool {
        if let Some((space_name, space)) = self.spaces.iter().next() {
            let mut window_result = None;
            let mut last = false;
            for (i, window) in space.elements().rev().enumerate() {
                info!(app_id = %window.app_id(), "looking at window");
                if window.app_id() == app_id {
                    window_result = Some(window.clone());
                    if i == 0 {
                        last = true;
                    } else if last {
                        continue;
                    } else {
                        break;
                    }
                };
            }
            if let Some(window) = window_result {
                self.focus_window(window, &space_name.to_owned());
                return true;
            }
        }
        false
    }

    pub fn focus_window(&mut self, window: ApplicationWindow, space_name: &str) {
        let space = self.spaces.get_mut(space_name).unwrap();
        space.raise_element(&window, true);
        let keyboard = self.seat.as_ref().unwrap().get_keyboard().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        keyboard.set_focus(self, Some(window.into()), serial);
    }
}
