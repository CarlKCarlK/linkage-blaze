#![no_std]
#![no_main]

use core::convert::Infallible;

use defmt::info;
use defmt_rtt as _;
use device_envoy_core::cyd::{Cyd as _, CydDisplay};
use device_envoy_rp::cyd::DEFAULT_DISPLAY_SPI_HZ;
use device_envoy_rp::{
    Result,
    button::{ButtonRp, PressedTo},
    cyd::{CydError, CydRpOneSpi, CydRpOneSpiStatic, DEFAULT_FONT, Orientation},
    flash_block::{FlashBlock as _, FlashBlockRp},
};
use embassy_executor::Spawner;
use embassy_rp::peripherals::SPI0;
use linkage_blaze_core::examples::armatron::{
    ArmatronExit, BACKGROUND, Error as ArmatronError, FOREGROUND, armatron,
};
use panic_probe as _;

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    info!("Starting CYD armatron loop (one-SPI) on RP Pico");

    let p = embassy_rp::init(Default::default());

    let [mut calibration_flash_block] = FlashBlockRp::new_array::<1>(p.FLASH)?;
    let mut calibration_button = ButtonRp::new(p.PIN_15, PressedTo::Ground);

    static CYD_STATIC: CydRpOneSpiStatic<SPI0, { CydRpOneSpi::<SPI0>::SCREEN_PIXELS }> =
        CydRpOneSpi::new_static();
    let (mut cyd, calibration_outcome) = CydRpOneSpi::new(
        &CYD_STATIC, // statics
        p.SPI0,      // spi
        p.PIN_18,    // sck_pin
        p.PIN_19,    // mosi_pin
        p.PIN_16,    // miso_pin
        p.PIN_17,    // lcd_cs_pin
        p.PIN_20,    // lcd_dc_pin
        p.PIN_21,    // lcd_rst_pin
        p.PIN_22,    // lcd_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        p.PIN_13,                     // touch_cs_pin
        p.PIN_14,                     // touch_irq_pin
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
        cortex_m::peripheral::SCB::sys_reset();
    }

    match armatron(&mut cyd, &mut calibration_button).await? {
        ArmatronExit::CalibrationRequested => {
            clear_calibration_and_reset(&mut cyd, &mut calibration_flash_block).await?;
        }
    }

    unreachable!("sys_reset does not return")
}

async fn clear_calibration_and_reset(
    cyd: &mut CydRpOneSpi<SPI0>,
    calibration_flash_block: &mut FlashBlockRp,
) -> Result<(), MainError> {
    calibration_flash_block.clear()?;
    reboot_with_message(cyd, "rebooting").await
}

async fn reboot_with_message(cyd: &mut CydRpOneSpi<SPI0>, message: &str) -> Result<(), MainError> {
    let display = cyd.display();
    let background565 = display.background_565();
    let mut frame = display.full_frame_mut();
    frame.fill(background565).write_text(message).flush()?;
    info!("Restarting");
    cortex_m::peripheral::SCB::sys_reset();
}

#[derive(Debug)]
enum MainError {
    DeviceEnvoy,
    Cyd,
    Armatron,
}

impl From<device_envoy_rp::Error> for MainError {
    fn from(_error: device_envoy_rp::Error) -> Self {
        Self::DeviceEnvoy
    }
}

impl From<CydError> for MainError {
    fn from(_error: CydError) -> Self {
        Self::Cyd
    }
}

impl From<ArmatronError<CydError>> for MainError {
    fn from(_error: ArmatronError<CydError>) -> Self {
        Self::Armatron
    }
}
