#![no_std]
#![no_main]
#![allow(long_running_const_eval)]

use core::convert::Infallible;

use defmt::info;
use defmt_rtt as _;
use device_envoy_rp::{
    Result,
    cyd::{CydDisplayRp, CydError, CydRp, CydStaticRp, DEFAULT_DISPLAY_SPI_HZ},
};
use embassy_executor::Spawner;
use linkage_blaze_core::examples::ballet::{
    self, BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, ballet,
};
use panic_probe as _;

#[derive(Debug)]
enum MainError {
    Cyd,
    Ballet,
}

impl From<CydError> for MainError {
    fn from(_error: CydError) -> Self {
        Self::Cyd
    }
}

impl From<ballet::Error<CydError>> for MainError {
    fn from(_error: ballet::Error<CydError>) -> Self {
        Self::Ballet
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    info!("Starting CYD ballet loop on RP Pico");

    let p = embassy_rp::init(Default::default());

    static CYD_STATIC: CydStaticRp<{ CydRp::SCREEN_PIXELS }> = CydRp::new_static();
    let mut display = CydDisplayRp::new(
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
        ORIENTATION, // orientation
        BACKGROUND,  // background
        FOREGROUND,  // foreground
        &TOP_FONT,   // font
    )?;
    info!("CYD display initialized");

    Ok(ballet(&mut display).await?)
}
