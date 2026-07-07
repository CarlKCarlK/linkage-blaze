#![no_std]
#![no_main]
// The embedded `MOTION` capture is a heavy const; its evaluation happens here,
// where the generic `ballet::<CydEsp>` is instantiated, so the allow lives here.
#![allow(long_running_const_eval)]

use core::convert::Infallible;

use device_envoy_esp::cyd::{CydDisplayEsp, CydError, CydEsp, CydStaticEsp};
use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_example_core::ballet::{
    self, BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, ballet,
};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    CydEsp(CydError),
    Ballet(ballet::Error<CydError>),
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    info!("Starting CYD ballet loop on ESP32-C2 / esp8684-devkitm-1-v1.0");

    static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
    let mut display = CydDisplayEsp::new(
        &CYD_STATIC, // statics
        p.SPI2,      // display_spi
        p.GPIO3,     // display_sck_pin
        p.GPIO4,     // display_mosi_pin
        p.GPIO5,     // display_miso_pin
        p.GPIO6,     // display_cs_pin
        p.GPIO7,     // display_dc_pin
        p.GPIO8,     // display_rst_pin
        p.GPIO9,     // display_backlight_pin
        ORIENTATION, // orientation
        BACKGROUND,  // background
        FOREGROUND,  // foreground
        &TOP_FONT,   // font
    )?;
    info!("CYD display initialized");

    Ok(ballet(&mut display).await?)
}
