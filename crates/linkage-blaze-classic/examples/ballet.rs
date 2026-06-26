#![no_std]
#![no_main]
// The embedded `MOTION` capture is a heavy const; its evaluation happens here,
// where the generic `ballet::<CydEsp>` is instantiated, so the allow lives here.
#![allow(long_running_const_eval)]

use core::convert::Infallible;

use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use esp_backtrace as _;
use linkage_blaze_cyd::{CydEsp, CydError, CydStaticEsp, Orientation};
use linkage_blaze_example_core::ballet::{BACKGROUND, FOREGROUND, ballet};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
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

    info!("Starting CYD ballet loop");

    static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
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
        // todo000 are there 4 orientations?
        Orientation::Portrait,
        BACKGROUND,
        FOREGROUND,
        &FONT_6X10,
    )?;
    info!("CYD display initialized");

    // Hand off to the device-agnostic render loop.
    Ok(ballet(&mut cyd).await?)
}
