#![no_std]
#![no_main]

// todo00 can/should there be a mode to share spi and cs pins?

use core::convert::Infallible;

use device_envoy_esp::{
    button::{Button as _, ButtonEsp, PressedTo},
    flash_block::{FlashBlock as _, FlashBlockEsp},
    init_and_start,
};
use embassy_executor::Spawner;
use esp_backtrace as _;

use linkage_blaze_cyd::{
    CydError, CydEsp, CydStaticEsp, DEFAULT_FONT, Orientation,
};
use linkage_blaze_cyd_core::{EnsureCalibrationError, ensure_calibration};
use linkage_blaze_example_core::armatron::{
    ArmatronOutcome, BACKGROUND, Error as ArmatronError, FOREGROUND, armatron,
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
    CalibrationDriverFlash,
}

impl From<device_envoy_esp::Error> for MainError {
    fn from(_error: device_envoy_esp::Error) -> Self {
        MainError::Flash
    }
}

impl From<CydError> for MainError {
    fn from(error: CydError) -> Self {
        match error {
            CydError::Flash(_) => MainError::Flash,
            CydError::DisplayInit(error) => match error {
                linkage_blaze_cyd::CydDisplayEspInitError::ConfigureDisplaySpi => {
                    MainError::ConfigureDisplaySpi
                }
                linkage_blaze_cyd::CydDisplayEspInitError::CreateDisplaySpiDevice => {
                    MainError::CreateDisplaySpiDevice
                }
                linkage_blaze_cyd::CydDisplayEspInitError::InitDisplay => MainError::InitDisplay,
            },
            CydError::TouchInit(error) => match error {
                linkage_blaze_cyd::CydTouchEspInitError::ConfigureTouchSpi => {
                    MainError::ConfigureTouchSpi
                }
                linkage_blaze_cyd::CydTouchEspInitError::CreateTouchSpiDevice => {
                    MainError::CreateTouchSpiDevice
                }
            },
            CydError::DisplayFlush(_) => MainError::FlushFrameBuffer,
            CydError::TouchUnavailable => unreachable!("touch always available when calibrated"),
            CydError::CalibrationUnavailable => {
                unreachable!("calibration is completed before entering the armatron loop")
            }
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

impl From<EnsureCalibrationError<CydError, device_envoy_esp::Error>> for MainError {
    fn from(error: EnsureCalibrationError<CydError, device_envoy_esp::Error>) -> Self {
        match error {
            EnsureCalibrationError::Device(error) => error.into(),
            EnsureCalibrationError::Flash(_error) => MainError::CalibrationDriverFlash,
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
    info!("Starting CYD armatron loop");

    let [mut calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let calibration_button = ButtonEsp::new(p.GPIO0, PressedTo::Ground);

    static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
    let mut cyd = CydEsp::new(
        &CYD_STATIC,
        p.SPI2,   // display SPI
        p.GPIO14, // display SCK
        p.GPIO13, // display MOSI
        p.GPIO12, // display MISO
        p.GPIO15, // display CS
        p.GPIO2,  // display DC
        p.GPIO4,  // display reset
        p.GPIO21, // display backlight
        Orientation::Landscape,
        BACKGROUND,              // default background
        FOREGROUND,              // default foreground
        &DEFAULT_FONT,           // default font
        p.SPI3,                  // touch SPI
        p.GPIO25,                // touch SCK
        p.GPIO32,                // touch MOSI
        p.GPIO39,                // touch MISO
        p.GPIO33,                // touch CS
        p.GPIO36,                // touch IRQ
    )?;
    info!("CYD display and touch initialized");

    let calibration_outcome = ensure_calibration(&mut cyd, &mut calibration_flash_block, || {
        calibration_button.is_pressed()
    })
    .await?;
    let calibration_config = calibration_outcome.calibration_config();
    cyd.set_calibration(calibration_config);
    if calibration_outcome.was_saved() {
        info!("Calibration saved; restarting");
        esp_hal::system::software_reset();
    }

    match armatron(&mut cyd).await? {
        ArmatronOutcome::CalibrateRequested => {
            calibration_flash_block.clear()?;
            info!("Calibration cleared; restarting");
            esp_hal::system::software_reset();
        }
    }
}
