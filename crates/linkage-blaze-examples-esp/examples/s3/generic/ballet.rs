#![no_std]
#![no_main]
// The embedded `MOTION` capture is a heavy const; its evaluation happens here,
// where the generic `ballet::<CydEsp>` is instantiated, so the allow lives here.
#![allow(long_running_const_eval)]

use core::convert::Infallible;

use device_envoy_esp::cyd::{
    CydDisplayEsp, CydEsp, CydStaticEsp, DEFAULT_DISPLAY_SPI_HZ, Error as CydError,
};
use device_envoy_esp::init_and_start;
use device_envoy_esp::{Error as DeviceEnvoyError, button::PressedTo, button_watch};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_core::examples::ballet::{
    self, BACKGROUND_COLOR, FOREGROUND_COLOR, ORIENTATION, TOP_FONT, run,
};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

button_watch! {
    ButtonWatch {
        pin: GPIO6,
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, Error> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    info!("Starting CYD ballet loop on ESP32-S3 / generic");

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
        ORIENTATION,      // orientation
        BACKGROUND_COLOR, // background_color
        FOREGROUND_COLOR, // foreground_color
        &TOP_FONT,        // font
    )?;
    info!("CYD display initialized");

    let button_watch = ButtonWatch::new(p.GPIO6, PressedTo::Ground, spawner).await?;
    Ok(run(&mut display, &*button_watch).await?)
}

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[derive(derive_more::From)]
enum Error {
    DeviceEnvoy(DeviceEnvoyError),
    Cyd(CydError),
    Ballet(ballet::Error<CydError>),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DeviceEnvoy(error) => formatter.debug_tuple("DeviceEnvoy").field(error).finish(),
            Self::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
            Self::Ballet(error) => formatter.debug_tuple("Ballet").field(error).finish(),
        }
    }
}
