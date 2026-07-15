#![no_std]
#![no_main]

use core::convert::Infallible;

use defmt::info;
use defmt_rtt as _;
use device_envoy_core::cyd::{Cyd as _, CydDisplay, display::CydFrame};
use device_envoy_rp::cyd::DEFAULT_DISPLAY_SPI_HZ;
use device_envoy_rp::{
    Result,
    button::{ButtonRp, PressedTo},
    cyd::{CydError, CydRp, CydStaticRp, DEFAULT_FONT, Orientation},
    flash_block::{FlashBlock as _, FlashBlockRp},
};
use embassy_executor::Spawner;
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
    info!("Starting CYD armatron loop on RP Pico");

    let p = embassy_rp::init(Default::default());

    let [mut calibration_flash_block] = FlashBlockRp::new_array::<1>(p.FLASH)?;
    let mut calibration_button = ButtonRp::new(p.PIN_15, PressedTo::Ground);

    static CYD_STATIC: CydStaticRp<{ CydRp::SCREEN_PIXELS }> = CydRp::new_static();
    let mut cyd = CydRp::new(
        &CYD_STATIC, // statics
        p.SPI0,      // display_spi
        p.PIN_18,    // display_sck_pin
        p.PIN_19,    // display_mosi_pin
        p.PIN_16,    // display_miso_pin
        p.PIN_17,    // display_cs_pin
        p.PIN_20,    // display_dc_pin
        p.PIN_21,    // display_rst_pin
        p.PIN_22,    // display_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        Orientation::Landscape,       // orientation
        BACKGROUND,                   // background
        FOREGROUND,                   // foreground
        &DEFAULT_FONT,                // font
        p.SPI1,                       // touch_spi
        p.PIN_10,                     // touch_sck_pin
        p.PIN_11,                     // touch_mosi_pin
        p.PIN_12,                     // touch_miso_pin
        p.PIN_13,                     // touch_cs_pin
        p.PIN_14,                     // touch_irq_pin
        &mut calibration_flash_block, // calibration_flash_block
        &mut calibration_button,      // recalibration_button
    )
    .await?;
    info!("CYD display and touch initialized");

    match armatron(&mut cyd, &mut calibration_button).await? {
        ArmatronExit::CalibrationRequested => {
            clear_calibration_and_reset(&mut cyd, &mut calibration_flash_block).await?;
        }
    }

    unreachable!("sys_reset does not return")
}

async fn clear_calibration_and_reset(
    cyd: &mut CydRp,
    calibration_flash_block: &mut FlashBlockRp,
) -> Result<(), MainError> {
    calibration_flash_block.clear()?;
    reboot_with_message(cyd, "rebooting").await
}

async fn reboot_with_message(cyd: &mut CydRp, message: &str) -> Result<(), MainError> {
    let display = cyd.display();
    let mut frame = display.full_frame_mut();
    frame.clear().write_text(message).flush()?;
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
