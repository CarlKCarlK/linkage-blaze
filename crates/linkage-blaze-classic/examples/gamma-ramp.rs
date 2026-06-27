#![no_std]
#![no_main]

//! Display-calibration test pattern for the CYD panel.
//!
//! Paints the gray/red/green/blue gamma ramp from
//! [`gamma_ramp`](linkage_blaze_example_core::gamma_ramp) and holds it on
//! screen. Photograph it (alongside the WASM `gamma.html` reference for a
//! camera-canceled comparison) and sample each patch to fit the panel's gamma.
//! No WiFi or clock is needed.

use core::convert::Infallible;

use device_envoy_esp::{Error, init_and_start};
use embedded_graphics::pixelcolor::{Rgb888, RgbColor};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_cyd::{CydEsp, CydError, CydStaticEsp, DEFAULT_FONT, Orientation};
use linkage_blaze_example_core::gamma_ramp::{self, gamma_ramp};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    DeviceEnvoy(Error),
    CydEsp(CydError),
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD gamma-ramp calibration pattern");

    // One patch at a time, so the draw buffer only needs a single cell.
    static CYD_STATIC: CydStaticEsp<{ gamma_ramp::CELL_PIXELS }> = CydEsp::new_static();
    let mut cyd = CydEsp::new_display_only(
        &CYD_STATIC,
        p.SPI2,
        p.GPIO14,
        p.GPIO13,
        p.GPIO12,
        p.GPIO15,
        p.GPIO2,
        p.GPIO4,
        p.GPIO21,
        Orientation::Portrait,
        Rgb888::BLACK,
        Rgb888::WHITE,
        &DEFAULT_FONT,
    )?;
    info!("CYD display initialized; painting ramp");

    Ok(gamma_ramp(&mut cyd).await?)
}
