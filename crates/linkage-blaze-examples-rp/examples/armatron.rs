#![no_std]
#![no_main]

use core::{convert::Infallible, fmt};

use defmt::info;
use defmt_rtt as _;
use device_envoy_core::cyd::{Cyd as _, CydDisplay, display::CydFrame};
use device_envoy_rp::cyd::DEFAULT_DISPLAY_SPI_HZ;
use device_envoy_rp::{
    Result,
    button::{ButtonRp, PressedTo},
    cyd::{CydRp, CydStaticRp, DEFAULT_FONT, Orientation},
    flash_block::{FlashBlock as _, FlashBlockRp},
};
use embassy_executor::Spawner;
use linkage_blaze_core::examples::armatron::{self, BACKGROUND_COLOR, Exit, FOREGROUND_COLOR};
use panic_probe as _;

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, Error> {
    info!("Starting CYD armatron loop on RP Pico");

    let p = embassy_rp::init(Default::default());

    let [mut calibration_flash_block] = FlashBlockRp::new_array::<1>(p.FLASH)?;
    let mut button = ButtonRp::new(p.PIN_15, PressedTo::Ground);

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
        BACKGROUND_COLOR,             // background_color
        FOREGROUND_COLOR,             // foreground_color
        &DEFAULT_FONT,                // font
        p.SPI1,                       // touch_spi
        p.PIN_10,                     // touch_sck_pin
        p.PIN_11,                     // touch_mosi_pin
        p.PIN_12,                     // touch_miso_pin
        p.PIN_13,                     // touch_cs_pin
        p.PIN_14,                     // touch_irq_pin
        &mut calibration_flash_block, // calibration_flash_block
        &mut button,                  // button
    )
    .await?;
    info!("CYD display and touch initialized");

    match armatron::run(&mut cyd, &mut button).await? {
        Exit::CalibrationRequested => {
            calibration_flash_block.clear()?;
            let mut frame = cyd.display().full_frame_mut();
            frame.clear().write_text("rebooting").flush()?;
            info!("Restarting");
            cortex_m::peripheral::SCB::sys_reset();
        }
    }
}

#[derive(derive_more::From)]
enum Error {
    DeviceEnvoy(device_envoy_rp::Error),
    Cyd(device_envoy_rp::cyd::Error),
    Armatron(armatron::Error<device_envoy_rp::cyd::Error>),
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceEnvoy(error) => formatter.debug_tuple("DeviceEnvoy").field(error).finish(),
            Self::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
            Self::Armatron(error) => formatter.debug_tuple("Armatron").field(error).finish(),
        }
    }
}
