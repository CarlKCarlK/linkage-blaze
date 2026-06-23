#![no_std]
#![no_main]

use core::{convert::Infallible, fmt};

use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use embedded_graphics::pixelcolor::Rgb565;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use linkage_blaze_ballet::{
    ballet_frames::{BALLET_FRAME_COUNT, BALLET_FRAMES},
    ballet_render::BG,
};
use linkage_blaze_core::Rgb888;
use linkage_blaze_cyd::{Cyd, CydDisplayConfig};
use log::info;

mod display;

use display::{CydBalletDisplay, CydBalletDisplayError};

fn rgb565(color: Rgb888) -> Rgb565 {
    Rgb565::from(color)
}

esp_bootloader_esp_idf::esp_app_desc!();

enum MainError {
    Cyd(linkage_blaze_cyd::CydError),
    Display(CydBalletDisplayError),
}

impl fmt::Debug for MainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MainError::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
            MainError::Display(error) => formatter.debug_tuple("Display").field(error).finish(),
        }
    }
}

impl From<linkage_blaze_cyd::CydError> for MainError {
    fn from(error: linkage_blaze_cyd::CydError) -> Self {
        MainError::Cyd(error)
    }
}

impl From<CydBalletDisplayError> for MainError {
    fn from(error: CydBalletDisplayError) -> Self {
        MainError::Display(error)
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

    info!("Starting CYD ballet loop");

    let mut cyd = Cyd::new_display(
        p.SPI2,
        p.GPIO14,
        p.GPIO13,
        p.GPIO12,
        p.GPIO15,
        p.GPIO2,
        p.GPIO4,
        p.GPIO21,
        CydDisplayConfig::PORTRAIT,
    )?;
    cyd.clear_now(rgb565(BG))?;
    let mut display = CydBalletDisplay::new(cyd);
    info!("CYD display initialized");

    let mut frame_index = 0;
    loop {
        let params = &BALLET_FRAMES[frame_index];
        display.show_frame(frame_index, params)?;
        frame_index += 1;
        if frame_index >= BALLET_FRAME_COUNT {
            frame_index = 0;
        }
        Delay::new().delay_millis(1);
    }
}
