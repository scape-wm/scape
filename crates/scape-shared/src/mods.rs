/// Represents the keyboard modifiers
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Mods {
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "shift" key
    pub shift: bool,
    /// The "logo" key
    pub logo: bool,
}
