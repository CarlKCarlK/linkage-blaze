#![no_std]

//! Platform-neutral example logic for the linkage-blaze CYD examples.
//!
//! The device abstraction itself lives in [`linkage_blaze_cyd_core`]; this crate
//! holds the generic examples ([`skeleton_clock`], [`ballet`]) written against
//! the [`Cyd`](linkage_blaze_cyd_core::Cyd) trait.

pub mod ballet;
pub mod gamma_ramp;
#[cfg(feature = "skeleton-clock")]
pub mod skeleton_clock;
