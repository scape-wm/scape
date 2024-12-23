use smithay::{
    output::{Mode, PhysicalProperties, Scale},
    utils::{Logical, Point, Transform},
};

/// Represents an output
#[derive(Debug)]
pub struct Output {
    /// The unique name of the output
    pub name: String,
    /// The description of the output
    pub description: String,
    /// The physical properties of the output
    pub physical: PhysicalProperties,
    /// The location of the output on the workspace
    pub location: Point<i32, Logical>,
    /// The transform of the output
    pub transform: Transform,
    /// The scale of the output
    pub scale: Scale,
    /// All available modes of the output
    pub modes: Vec<Mode>,
    /// The current mode of the output
    pub current_mode: Option<Mode>,
    /// The preferred mode of the output
    pub preferred_mode: Option<Mode>,
}
