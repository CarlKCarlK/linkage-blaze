#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_core::cyd::{Cyd as _, CydDisplay};
use device_envoy_esp::cyd::DEFAULT_DISPLAY_SPI_HZ;
use device_envoy_esp::{
    button::{ButtonEsp, PressedTo},
    cyd::{CydError, CydEspOneSpi, CydStaticEsp, DEFAULT_FONT, Orientation},
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

#[derive(Debug)]
enum MainError {
    Flash,
    ConfigureDisplaySpi,
    CreateDisplaySpiDevice,
    ConfigureTouchSpi,
    CreateTouchSpiDevice,
    InitDisplay,
    FlushFrameBuffer,
    FormatText,
}

impl From<device_envoy_esp::Error> for MainError {
    fn from(_error: device_envoy_esp::Error) -> Self {
        MainError::Flash
    }
}

impl From<CydError> for MainError {
    fn from(error: CydError) -> Self {
        match error {
            CydError::DisplayInit(error) => match error {
                device_envoy_esp::cyd::CydDisplayEspInitError::ConfigureDisplaySpi => {
                    MainError::ConfigureDisplaySpi
                }
                device_envoy_esp::cyd::CydDisplayEspInitError::CreateDisplaySpiDevice => {
                    MainError::CreateDisplaySpiDevice
                }
                device_envoy_esp::cyd::CydDisplayEspInitError::InitDisplay => {
                    MainError::InitDisplay
                }
            },
            CydError::TouchInit(error) => match error {
                device_envoy_esp::cyd::CydTouchEspInitError::ConfigureTouchSpi => {
                    MainError::ConfigureTouchSpi
                }
                device_envoy_esp::cyd::CydTouchEspInitError::CreateTouchSpiDevice => {
                    MainError::CreateTouchSpiDevice
                }
            },
            CydError::DisplayFlush(_) => MainError::FlushFrameBuffer,
        }
    }
}

impl From<ArmatronError<CydError>> for MainError {
    fn from(error: ArmatronError<CydError>) -> Self {
        match error {
            ArmatronError::Ui(_) => MainError::FormatText,
            ArmatronError::Cyd(error) => error.into(),
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD armatron loop (one-SPI) on ESP32-S3 / esp32-s3-devkitc-1-v1.1-n16r8");

    let [mut calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let mut calibration_button = ButtonEsp::new(p.GPIO6, PressedTo::Ground);

    static CYD_STATIC: CydStaticEsp<{ CydEspOneSpi::SCREEN_PIXELS }> = CydEspOneSpi::new_static();
    let (mut cyd, calibration_outcome) = CydEspOneSpi::new(
        &CYD_STATIC, // statics
        p.SPI2,      // spi
        p.GPIO1,     // sck_pin
        p.GPIO2,     // mosi_pin
        p.GPIO3,     // miso_pin
        p.GPIO4,     // lcd_cs_pin
        p.GPIO5,     // lcd_dc_pin
        p.GPIO7,     // lcd_rst_pin
        p.GPIO8,     // lcd_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        p.GPIO12,                     // touch_cs_pin
        p.GPIO13,                     // touch_irq_pin
        Orientation::Landscape,       // orientation
        BACKGROUND,                   // background
        FOREGROUND,                   // foreground
        &DEFAULT_FONT,                // font
        &mut calibration_flash_block, // calibration_flash_block
        &mut calibration_button,      // recalibration_button
        Some("rebooting"),            // confirmed_message
    )
    .await?;
    info!("CYD display and touch initialized");
    if calibration_outcome.was_saved() {
        info!("Restarting");
        esp_hal::system::software_reset();
    }

    match armatron(&mut cyd, &mut calibration_button).await? {
        ArmatronExit::CalibrationRequested => {
            clear_calibration_and_reset(&mut cyd, &mut calibration_flash_block).await?;
        }
    }

    unreachable!("software_reset does not return")
}

async fn clear_calibration_and_reset(
    cyd: &mut CydEspOneSpi,
    calibration_flash_block: &mut FlashBlockEsp,
) -> Result<(), MainError> {
    calibration_flash_block.clear()?;
    reboot_with_message(cyd, "rebooting").await
}

async fn reboot_with_message(cyd: &mut CydEspOneSpi, message: &str) -> Result<(), MainError> {
    let display = cyd.display();
    let background565 = display.background_565();
    let mut frame = display.full_frame_mut();
    frame.fill(background565).write_text(message).flush()?;
    info!("Restarting");
    esp_hal::system::software_reset();
}
