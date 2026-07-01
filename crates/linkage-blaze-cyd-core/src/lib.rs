#![no_std]

//! Platform-neutral core of the CYD device abstraction.
//!
//! See [`cyd`] for the [`Cyd`] device trait and its [`CydFrame`].

mod cyd;
mod contiguous_pixels;
mod orientation;
mod tga;
pub mod tiling;
mod touch_event;

pub use cyd::{
    CopySizeError, Cyd, CydFlushError, CydFrame, CydInfallibleError, RegionPixels, Tiles,
};
pub use contiguous_pixels::ContiguousPixels;
pub use orientation::Orientation;
pub use tga::{Image565, Image565Mask};
pub use touch_event::TouchInputEvent;

/// Native panel width in pixels (landscape). The CYD panel is fixed hardware.
pub const SCREEN_WIDTH: usize = 320;
/// Native panel height in pixels (landscape). The CYD panel is fixed hardware.
pub const SCREEN_HEIGHT: usize = 240;
/// Total panel pixel count (`SCREEN_WIDTH * SCREEN_HEIGHT`).
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
