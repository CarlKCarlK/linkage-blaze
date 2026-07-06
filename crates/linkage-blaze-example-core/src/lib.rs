#![no_std]

//! Platform-neutral example logic for the linkage-blaze CYD examples.
//!
//! The device abstraction itself lives in [`device_envoy_core::cyd`]; this crate
//! holds the generic examples ([`armatron`], [`skeleton_clock`], [`clock`], [`ballet`]) written
//! against the [`Cyd`](device_envoy_core::cyd::Cyd),
//! [`CydDisplay`](device_envoy_core::cyd::CydDisplay), and
//! [`CydTouch`](device_envoy_core::cyd::CydTouch) traits.

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
pub mod ui;
