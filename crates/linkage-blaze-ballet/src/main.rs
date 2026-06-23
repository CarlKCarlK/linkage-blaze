#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::delay::Delay;
use linkage_blaze_ballet::{
    ballet_frames::{BALLET_FRAME_COUNT, BALLET_FRAMES},
    ballet_render::BG,
};
use linkage_blaze_cyd::{Cyd, CydDisplayConfig};
use log::info;

mod display;

use display::{CydBalletDisplay, CydBalletDisplayError};

esp_bootloader_esp_idf::esp_app_desc!();

// todo0000 is this the best way to do this? isn't there a crate?
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
    // todo000 is the _now suffix good?
    // todo000 BG violates our policy against abbreviations.
    cyd.clear_now(Cyd::rgb565(BG))?;
    // todo000 in this case, we likely don't want a CydBalletDisplay struct.
    let mut display = CydBalletDisplay::new(cyd);
    info!("CYD display initialized");

    // todo0000 shouldn't this just be a for loop?
    let mut frame_index = 0;
    loop {
        if frame_index == 0 {
            info!("starting ballet cycle");
        }
        let params = &BALLET_FRAMES[frame_index];
        // todo000 pull this back in.
        display.show_frame(frame_index, params)?;
        frame_index += 1;
        if frame_index >= BALLET_FRAME_COUNT {
            frame_index = 0;
        }
        Delay::new().delay_millis(1);
    }
}
// todo000 still need to review other files in the project.
