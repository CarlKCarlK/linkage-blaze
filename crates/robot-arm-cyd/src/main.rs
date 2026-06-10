#![no_std]
#![no_main]

use core::{convert::Infallible, future::pending};

use embassy_executor::Spawner;
use esp_backtrace as _;
use log::info;
use robot_arm_core::cyd::CydSim;

use device_envoy_esp::{Result, init_and_start};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible> {
    init_and_start!(p);

    esp_println::logger::init_logger(log::LevelFilter::Info);

    // TODO0000 Verify CYD ESP32-2432S028R display and touch pinout on hardware.
    let cyd_sim = CydSim::new();
    let (width, height) = (cyd_sim.width(), cyd_sim.height());
    info!("robot-arm-cyd scaffold started: {width}x{height}");

    let _ = p;
    pending().await
}
