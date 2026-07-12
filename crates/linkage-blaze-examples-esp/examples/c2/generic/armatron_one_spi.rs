#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_core::cyd::{Cyd as _, CydDisplay, display::CydFrame};
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

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD armatron loop (one-SPI) on ESP32-C2 / generic");

    let [mut calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let mut calibration_button = ButtonEsp::new(p.GPIO18, PressedTo::Ground);

    static CYD_STATIC: CydStaticEsp<{ CydEspOneSpi::SCREEN_PIXELS }> = CydEspOneSpi::new_static();
    let (mut cyd, calibration_outcome) = CydEspOneSpi::new(
        &CYD_STATIC, // statics
        p.SPI2,      // spi
        p.GPIO6,     // sck_pin
        p.GPIO7,     // mosi_pin
        p.GPIO2,     // miso_pin
        p.GPIO10,    // lcd_cs_pin
        p.GPIO3,     // lcd_dc_pin
        p.GPIO4,     // lcd_rst_pin
        p.GPIO5,     // lcd_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        p.GPIO0,                      // touch_cs_pin
        p.GPIO1,                      // touch_irq_pin
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
    let mut frame = display.full_frame_mut();
    frame.clear().write_text(message).flush()?;
    info!("Restarting");
    esp_hal::system::software_reset();
}

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
            ArmatronError::Linkage(_) => MainError::FormatText,
            ArmatronError::Ui(_) => MainError::FormatText,
            ArmatronError::Cyd(error) => error.into(),
        }
    }
}
