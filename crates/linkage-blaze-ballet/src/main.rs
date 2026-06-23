#![no_std]
#![no_main]

use core::{convert::Infallible, fmt::Write};

use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::IntoStorage,
    prelude::Point,
    text::{Baseline, Text},
};
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::time::Instant;
use linkage_blaze_ballet::{
    ballet_frames::{BALLET_FRAME_COUNT, BALLET_FRAMES},
    ballet_render::{BACKGROUND, PixelTarget, TEXT, render_frame},
};
use linkage_blaze_core::Rgb888;
use linkage_blaze_cyd::{Cyd, CydDisplayConfig, PixelBuffer, RectPixels, RectView};
use log::info;
use static_cell::StaticCell;

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
    cyd.clear(Cyd::rgb565(BACKGROUND))?;
    // todo000 RectWorkspace is a bad name
    static SCREEN_PIXEL_BUFFER: StaticCell<PixelBuffer<{ Cyd::SCREEN_PIXELS }>> = StaticCell::new();
    let screen_pixel_buffer = PixelBuffer::init_static(&SCREEN_PIXEL_BUFFER);
    let screen_size = cyd.screen_size();
    let mut last_frame_ms = 0;
    info!("CYD display initialized");

    loop {
        info!("starting ballet cycle");
        for (frame_index, params) in BALLET_FRAMES.iter().enumerate() {
            let started = Instant::now();
            // todo000 (may no longer apply) these consts should be read from the cyd object, not be here.
            // todo000 (may no longer apply) why are these constants need at all?
            let mut screen_buffer = screen_pixel_buffer
                .view_mut(screen_size.width as usize, screen_size.height as usize);
            screen_buffer.clear(Cyd::rgb565(BACKGROUND));
            {
                // todo000 (may no longer apply) what??? EspBalletTileSink
                // todo000 continue review from this point
                let mut target = FullScreenTarget {
                    screen_buffer: &mut screen_buffer,
                };
                render_frame(&mut target, params);
            }
            draw_status(&mut screen_buffer, frame_index, last_frame_ms);
            cyd.flush(&screen_buffer, Point::new(0, 0))?;
            last_frame_ms = (Instant::now() - started).as_millis();
            Delay::new().delay_millis(1);
        }
    }
}
// todo000 still need to review other files in the project.

fn draw_status(screen_buffer: &mut RectView<'_>, frame_index: usize, last_frame_ms: u64) {
    let elapsed_ms = last_frame_ms.max(1);
    let fps_x10 = (10_000 / elapsed_ms) as u32;
    let slomo_x10 = if fps_x10 == 0 {
        0
    } else {
        (SOURCE_FPS_X10 * 10 + fps_x10 / 2) / fps_x10
    };
    let mut status = heapless::String::<64>::new();
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

    Text::with_baseline(
        status.as_str(),
        Point::new(0, 0),
        MonoTextStyle::new(&FONT_6X10, Cyd::rgb565(TEXT)),
        Baseline::Top,
    )
    .draw(screen_buffer)
    .ok();
}

struct FullScreenTarget<'a, 'b> {
    screen_buffer: &'a mut RectView<'b>,
}

impl PixelTarget for FullScreenTarget<'_, '_> {
    fn width(&self) -> usize {
        self.screen_buffer.width()
    }

    fn height(&self) -> usize {
        self.screen_buffer.height()
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        if x >= self.screen_buffer.width() || y >= self.screen_buffer.height() {
            return;
        }
        let stride = self.screen_buffer.width();
        self.screen_buffer.raw_pixels_mut()[y * stride + x] = Cyd::rgb565(color).into_storage();
    }
}
