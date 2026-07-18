#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_core::cyd::{Cyd as _, CydDisplay, display::CydFrame};
use device_envoy_esp::cyd::DEFAULT_DISPLAY_SPI_HZ;
use device_envoy_esp::{
    button::PressedTo,
    button_watch,
    cyd::{CydError, CydEsp, CydStaticEsp, DEFAULT_FONT, Orientation},
    flash_block::{FlashBlock as _, FlashBlockEsp},
    init_and_start,
};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_core::examples::armatron::{
    ArmatronExit, BACKGROUND, Error as ArmatronError, FOREGROUND, armatron,
};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

button_watch! {
    CalibrationButtonWatch {
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
    info!("Starting CYD armatron loop on ESP32-S3 / generic");

    let [mut calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let calibration_button =
        CalibrationButtonWatch::new(p.GPIO6, PressedTo::Ground, spawner).await?;

    static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
    let mut cyd = CydEsp::new(
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
        Orientation::Landscape,       // orientation
        BACKGROUND,                   // background
        FOREGROUND,                   // foreground
        &DEFAULT_FONT,                // font
        p.SPI3,                       // touch_spi
        p.GPIO9,                      // touch_sck_pin
        p.GPIO10,                     // touch_mosi_pin
        p.GPIO11,                     // touch_miso_pin
        p.GPIO12,                     // touch_cs_pin
        p.GPIO13,                     // touch_irq_pin
        &mut calibration_flash_block, // calibration_flash_block
        &mut *calibration_button,     // recalibration_button
    )
    .await?;
    info!("CYD display and touch initialized");
    match armatron(&mut cyd, &mut *calibration_button).await? {
        ArmatronExit::CalibrationRequested => {
            clear_calibration_and_reset(&mut cyd, &mut calibration_flash_block).await?;
        }
    }

    unreachable!("software_reset does not return")
}

async fn clear_calibration_and_reset(
    cyd: &mut CydEsp,
    calibration_flash_block: &mut FlashBlockEsp,
) -> Result<(), Error> {
    calibration_flash_block.clear()?;
    reboot_with_message(cyd, "rebooting").await
}

async fn reboot_with_message(cyd: &mut CydEsp, message: &str) -> Result<(), Error> {
    let (display, _) = cyd.parts();
    let mut frame = display.full_frame_mut();
    frame.clear().write_text(message).flush()?;
    info!("Restarting");
    esp_hal::system::software_reset();
}

#[derive(derive_more::From)]
enum Error {
    DeviceEnvoy(device_envoy_esp::Error),
    Cyd(CydError),
    Armatron(ArmatronError<CydError>),
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
