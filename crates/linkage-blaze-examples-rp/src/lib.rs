#![cfg_attr(target_os = "none", no_std)]

//! Raspberry Pi Pico entrypoints for the shared linkage-blaze CYD examples.

#[cfg(all(target_os = "none", not(any(feature = "pico1", feature = "pico2"))))]
compile_error!("Must enable exactly one board feature: 'pico1' or 'pico2'");

#[cfg(all(target_os = "none", feature = "pico1", feature = "pico2"))]
compile_error!("Cannot enable both 'pico1' and 'pico2' features simultaneously");

#[cfg(all(target_os = "none", not(feature = "arm")))]
compile_error!("Must enable architecture feature: 'arm'");
