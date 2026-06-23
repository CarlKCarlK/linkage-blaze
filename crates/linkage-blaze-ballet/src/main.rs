#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use linkage_blaze_ballet::{ballet_frames::BALLET_FRAMES, ballet_render::BACKGROUND};
use linkage_blaze_cyd::{Cyd, CydDisplayConfig};
use log::info;

mod display;

use display::{CydBalletDisplay, CydBalletDisplayError};

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    Cyd(linkage_blaze_cyd::CydError),
    Display(CydBalletDisplayError),
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
        // todo000 are there 4 orientations?
        CydDisplayConfig::PORTRAIT,
    )?;
    // todo000 agent, remember to never delete my todo's.
    cyd.fill_screen(Cyd::rgb565(BACKGROUND))?;
    // todo000 in this case, we likely don't want a CydBalletDisplay struct.
    let mut display = CydBalletDisplay::new(cyd);
    info!("CYD display initialized");

    loop {
        info!("starting ballet cycle");
        for (frame_index, params) in BALLET_FRAMES.iter().enumerate() {
            // todo000 pull this back in.
            display.show_frame(frame_index, params)?;
            Delay::new().delay_millis(1);
        }
    }
}
// todo000 still need to review other files in the project.
