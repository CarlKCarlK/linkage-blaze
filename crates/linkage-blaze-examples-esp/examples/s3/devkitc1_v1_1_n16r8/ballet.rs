#![no_std]
#![no_main]
// The embedded `MOTION` capture is a heavy const; its evaluation happens here,
// where the generic `ballet::<CydEsp>` is instantiated, so the allow lives here.
#![allow(long_running_const_eval)]

use core::convert::Infallible;

use device_envoy_esp::cyd::{
    CydDisplayEsp, CydError, CydEsp, CydStaticEsp, DEFAULT_DISPLAY_SPI_HZ,
};
use device_envoy_esp::init_and_start;
use device_envoy_esp::{Error, button::PressedTo, button_watch};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_core::examples::ballet::{
    self, BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, ballet,
};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

button_watch! {
    BalletButtonWatch {
        pin: GPIO6,
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    info!("Starting CYD ballet loop on ESP32-S3 / esp32-s3-devkitc-1-v1.1-n16r8");

    static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
    let mut display = CydDisplayEsp::new(
        &CYD_STATIC, // statics
        p.SPI2,      // display_spi
        p.GPIO1,     // display_sck_pin
        p.GPIO2,     // display_mosi_pin
        p.GPIO3,     // display_miso_pin
        p.GPIO4,     // display_cs_pin
        p.GPIO5,     // display_dc_pin
        p.GPIO7,     // display_rst_pin
        p.GPIO8,     // display_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        ORIENTATION, // orientation
        BACKGROUND,  // background
        FOREGROUND,  // foreground
        &TOP_FONT,   // font
    )?;
    info!("CYD display initialized");

    let button = BalletButtonWatch::new(p.GPIO6, PressedTo::Ground, spawner).await?;
    Ok(ballet(&mut display, &*button).await?)
}

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    DeviceEnvoy(Error),
    CydEsp(CydError),
    Ballet(ballet::Error<CydError>),
}
