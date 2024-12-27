use smithay::utils::{Logical, Rectangle};

#[derive(Debug)]
/// Represents a zone in logical compositor space. A zone is a rectangular area that is used for window placement.
pub struct Zone {
    /// The name of the zone
    pub name: String,
    /// The geometry of the zone
    pub geometry: Rectangle<i32, Logical>,
    /// Whether the zone is the default zone
    pub default: bool,
}

impl Zone {
    /// Creates a new instance from the given name, offset, size and default flag
    pub fn new(name: String, x: i32, y: i32, width: i32, height: i32, default: bool) -> Self {
        Self {
            name,
            geometry: Rectangle::from_loc_and_size((x, y), (width, height)),
            default,
        }
    }
}
