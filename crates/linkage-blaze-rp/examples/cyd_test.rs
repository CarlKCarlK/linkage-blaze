#![no_std]
#![no_main]

use core::convert::Infallible;

use defmt::info;
use defmt_rtt as _;
use device_envoy_core::cyd::CydDisplay;
use device_envoy_rp::{
    Result,
    cyd::{
        CydDisplayRp, CydError, CydRp, CydStaticRp, DEFAULT_DISPLAY_SPI_HZ, DEFAULT_FONT,
        Orientation,
    },
};
use embassy_executor::Spawner;
use embassy_time::Timer;
use embedded_graphics::{
    pixelcolor::{Rgb565, Rgb888},
    prelude::RgbColor,
};
use panic_probe as _;

const BACKGROUND: Rgb888 = Rgb888::new(0, 0, 0); // black
const FOREGROUND: Rgb888 = Rgb888::new(255, 255, 255); // white

#[derive(Debug)]
enum MainError {
    Cyd,
}

impl From<CydError> for MainError {
    fn from(_error: CydError) -> Self {
        Self::Cyd
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    info!("Starting CYD screen test on RP Pico DIRECT-FILL-V2");

    let p = embassy_rp::init(Default::default());

    static CYD_STATIC: CydStaticRp<0> = CydRp::new_static();
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
        Orientation::Landscape, // orientation
        BACKGROUND,             // background
        FOREGROUND,             // foreground
        &DEFAULT_FONT,          // font
    )?;
    info!("CYD display initialized");

    loop {
        show_step(&mut display, Rgb565::MAGENTA, "MAGENTA").await?;
        show_step(&mut display, Rgb565::YELLOW, "YELLOW").await?;
        show_step(&mut display, Rgb565::CYAN, "CYAN").await?;
        show_step(&mut display, Rgb565::WHITE, "WHITE").await?;
        show_step(&mut display, Rgb565::BLACK, "BLACK").await?;
    }
}

async fn show_step(display: &mut CydDisplayRp, color: Rgb565, label: &str) -> Result<(), CydError> {
    info!("Screen test direct-fill step: {}", label);
    display.fill(color)?;
    Timer::after_secs(2).await;
    Ok(())
}
