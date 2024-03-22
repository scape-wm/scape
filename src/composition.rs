use crate::{application_window::ApplicationWindow, config::ConfigZone, State};
use smithay::{
    desktop::{layer_map_for_output, Space},
    utils::{Logical, Point, Rectangle},
};

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
}
