#![no_std]
#![no_main]

use core::convert::Infallible;

use defmt::info;
use defmt_rtt as _;
use device_envoy_rp::Result;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;
use panic_probe as _;

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible> {
    info!("Starting CYD backlight test on RP Pico");
    let p = embassy_rp::init(Default::default());

    let mut backlight = Output::new(p.PIN_22, Level::Low);

    loop {
        info!("Backlight ON");
        backlight.set_high();
        Timer::after_secs(2).await;

        info!("Backlight OFF");
        backlight.set_low();
        Timer::after_secs(2).await;
    }
}
