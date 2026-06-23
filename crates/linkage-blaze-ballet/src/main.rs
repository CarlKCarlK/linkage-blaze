#![no_std]
#![no_main]

use core::{convert::Infallible, fmt::Write};

use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point},
    text::{Baseline, Text},
};
use esp_backtrace as _;
use esp_hal::time::{Duration, Instant};
use linkage_blaze_ballet::{
    ballet_frames::{BALLET_FRAME_COUNT, BALLET_FRAMES},
    ballet_render::{BACKGROUND, TEXT, render_frame},
};
use linkage_blaze_cyd::{Cyd, CydDisplayConfig, CydStatic, PixelBufferFull};
use log::info;

// todo000 I'm not happy with all this noise.

// todo000 this seems unmotivated.
const SOURCE_FPS_X10: u32 = 1200;

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    Cyd(linkage_blaze_cyd::CydError),
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

    static CYD_STATIC: CydStatic<PixelBufferFull> = CydStatic::new();
    let mut cyd = Cyd::new_display_only(
        &CYD_STATIC,
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
    let background565 = Cyd::rgb565(BACKGROUND);
    let text565 = Cyd::rgb565(TEXT);
    // todo000 agent, remember to never delete my todo's.
    cyd.clear(background565)?;
    info!("CYD display initialized");

    let mut last_frame_duration = None;
    loop {
        info!("starting ballet cycle");
        for (frame_index, params) in BALLET_FRAMES.iter().enumerate() {
            let started = Instant::now();
            let mut cyd_frame = cyd.full_frame_mut();
            cyd_frame.clear(background565);
            render_frame(&mut cyd_frame, params);
            draw_status(&mut cyd_frame, text565, frame_index, last_frame_duration);
            cyd_frame.flush()?;
            last_frame_duration = Some(Instant::now() - started);
        }
    }
}
// todo000 still need to review other files in the project.

fn draw_status<D>(
    draw_target: &mut D,
    text565: Rgb565,
    frame_index: usize,
    last_frame_duration: Option<Duration>,
) where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    let mut status = heapless::String::<64>::new();
    if let Some(last_frame_duration) = last_frame_duration {
        let elapsed_ms = last_frame_duration.as_millis().max(1);
        let fps_x10 = (10_000 / elapsed_ms) as u32;
        let slomo_x10 = if fps_x10 == 0 {
            0
        } else {
            (SOURCE_FPS_X10 * 10 + fps_x10 / 2) / fps_x10
        };
        Write::write_fmt(
            &mut status,
            format_args!(
                "{}/{}  fps {}.{}  slow {}.{}x",
                frame_index + 1,
                BALLET_FRAME_COUNT,
                fps_x10 / 10,
                fps_x10 % 10,
                slomo_x10 / 10,
                slomo_x10 % 10
            ),
        )
        .ok();
    } else {
        Write::write_fmt(
            &mut status,
            format_args!(
                "{}/{}  fps --.-  slow --.-x",
                frame_index + 1,
                BALLET_FRAME_COUNT
            ),
        )
        .ok();
    }

    Text::with_baseline(
        status.as_str(),
        Point::new(0, 0),
        MonoTextStyle::new(&FONT_6X10, text565),
        Baseline::Top,
    )
    .draw(draw_target)
    .ok();
}
