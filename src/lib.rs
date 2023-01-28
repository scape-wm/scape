#![cfg_attr(
    not(any(feature = "winit", feature = "udev")),
    allow(dead_code, unused_imports)
)]

#[macro_use]
extern crate slog;

#[cfg(any(feature = "udev", feature = "xwayland"))]
pub mod cursor;
pub mod drawing;
pub mod focus;
pub mod input_handler;
pub mod render;
pub mod shell;
pub mod state;
#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

pub use state::{AnvilState, CalloopData, ClientState};
