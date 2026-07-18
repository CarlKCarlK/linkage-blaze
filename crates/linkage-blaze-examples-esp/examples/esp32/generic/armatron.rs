#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_core::cyd::{Cyd as _, CydDisplay, display::CydFrame};
use device_envoy_esp::cyd::DEFAULT_DISPLAY_SPI_HZ;
use device_envoy_esp::{
    button::PressedTo,
    button_watch,
    cyd::{CydEsp, CydStaticEsp, DEFAULT_FONT, Orientation},
    flash_block::{FlashBlock as _, FlashBlockEsp},
    init_and_start,
};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_core::examples::armatron::{self, BACKGROUND, FOREGROUND, run};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

button_watch! {
    ButtonWatch {
        pin: GPIO0,
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
    info!("Starting CYD armatron loop on ESP32 / generic");

    let [mut calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let button_watch = ButtonWatch::new(p.GPIO0, PressedTo::Ground, spawner).await?;

    static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
    let mut cyd = CydEsp::new(
        &CYD_STATIC, // statics
        p.SPI2,      // display_spi
        p.GPIO14,    // display_sck_pin
        p.GPIO13,    // display_mosi_pin
        p.GPIO12,    // display_miso_pin
        p.GPIO15,    // display_cs_pin
        p.GPIO2,     // display_dc_pin
        p.GPIO4,     // display_rst_pin
        p.GPIO21,    // display_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        Orientation::Landscape,       // orientation
        BACKGROUND,                   // background
        FOREGROUND,                   // foreground
        &DEFAULT_FONT,                // font
        p.SPI3,                       // touch_spi
        p.GPIO25,                     // touch_sck_pin
        p.GPIO32,                     // touch_mosi_pin
        p.GPIO39,                     // touch_miso_pin
        p.GPIO33,                     // touch_cs_pin
        p.GPIO36,                     // touch_irq_pin
        &mut calibration_flash_block, // calibration_flash_block
        &mut *button_watch,           // button_watch
    )
    .await?;
    info!("CYD display and touch initialized");
    match run(&mut cyd, &mut *button_watch).await? {
        armatron::Exit::CalibrationRequested => {
            calibration_flash_block.clear()?;
            let mut frame = cyd.display().full_frame_mut();
            frame.clear().write_text("rebooting").flush()?;
            info!("Restarting");
            esp_hal::system::software_reset();
        }
    }
}

#[derive(derive_more::From)]
enum Error {
    DeviceEnvoy(device_envoy_esp::Error),
    Cyd(device_envoy_esp::cyd::Error),
    Armatron(armatron::Error<device_envoy_esp::cyd::Error>),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DeviceEnvoy(error) => formatter.debug_tuple("DeviceEnvoy").field(error).finish(),
            Self::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
            Self::Armatron(error) => formatter.debug_tuple("Armatron").field(error).finish(),
        }
    }
}
