#![no_std]

//! Platform-neutral example logic for the linkage-blaze CYD examples.
//!
//! The device abstraction itself lives in [`device_envoy_core::cyd`]; this crate
//! holds the generic examples ([`armatron`], [`skeleton_clock`], [`clock`], [`ballet`]) written
//! against the owned CYD parts:
//! [`CydDisplay`](device_envoy_core::cyd::CydDisplay) and
//! [`CydTouch`](device_envoy_core::cyd::CydTouch).

#[cfg(feature = "armatron")]
#[path = "armatron/main.rs"]
pub mod armatron;
#[cfg(feature = "ballet")]
pub mod ballet;
#[cfg(feature = "clock")]
pub mod clock;
#[cfg(feature = "skeleton-clock")]
pub mod skeleton_clock;
pub mod ui;
