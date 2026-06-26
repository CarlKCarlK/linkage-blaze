#![no_std]

//! Platform-neutral core for the linkage-blaze CYD examples.
//!
//! See [`cyd_surface`] for the device abstraction and [`skeleton_clock`] for the
//! first generic example built on it.

pub mod ballet;
mod cyd_surface;
pub mod skeleton_clock;
pub mod tiling;
mod translated;

pub use cyd_surface::{CydFrameOps, CydSurface};
pub use translated::TranslatedDrawTarget;
