#![no_std]

//! Platform-neutral example logic for the linkage-blaze CYD examples.
//!
//! The device abstraction itself lives in [`linkage_blaze_cyd_core`]; this crate
//! holds the generic examples ([`armatron`], [`skeleton_clock`], [`clock`], [`ballet`]) written
//! against the [`Cyd`](linkage_blaze_cyd_core::Cyd) trait.

#[cfg(feature = "armatron")]
#[path = "armatron/main.rs"]
pub mod armatron;
#[cfg(feature = "ballet")]
pub mod ballet;
#[cfg(feature = "clock")]
pub mod clock;
pub mod infallible;
#[cfg(feature = "skeleton-clock")]
pub mod skeleton_clock;
