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
    ballet_frames::{BALLET_DOF, BALLET_FRAME_COUNT, BALLET_FRAMES},
    ballet_render::{
        BACKGROUND, BalletTileSink, PixelTarget, SCREEN_HEIGHT, SCREEN_WIDTH, TEXT, TileFlush,
        draw_tiles, render_tile,
    },
};
use linkage_blaze_core::Rgb888;
use linkage_blaze_cyd::{Cyd, CydDisplayConfig, RectPixels, RectView, RectWorkspace};
use log::info;
use static_cell::StaticCell;

const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const SOURCE_FPS_X10: u32 = 1200;

type ScreenWorkspace = RectWorkspace<SCREEN_PIXELS>;

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
    cyd.fill_screen(Cyd::rgb565(BACKGROUND))?;
    // todo000 (may no longer apply) in this case, we likely don't want a CydBalletDisplay struct.
    static SCREEN_WORKSPACE: StaticCell<ScreenWorkspace> = StaticCell::new();
    let screen_workspace = ScreenWorkspace::init_static(&SCREEN_WORKSPACE);
    let mut last_frame_ms = 0;
    info!("CYD display initialized");

    loop {
        info!("starting ballet cycle");
        for (frame_index, params) in BALLET_FRAMES.iter().enumerate() {
            // todo000 pull this back in.
            let started = Instant::now();
            let mut screen_buffer = screen_workspace.view_mut(SCREEN_WIDTH, SCREEN_HEIGHT);
            screen_buffer.clear(Cyd::rgb565(BACKGROUND));
            {
                let mut sink = EspBalletTileSink {
                    screen_buffer: &mut screen_buffer,
                };
                draw_tiles(params, &mut sink);
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

struct EspBalletTileSink<'a, 'b> {
    screen_buffer: &'a mut RectView<'b>,
}

impl BalletTileSink for EspBalletTileSink<'_, '_> {
    fn draw_tile(&mut self, tile_flush: TileFlush, params: &[f32; BALLET_DOF]) {
        let mut target = FullScreenTileTarget {
            screen_buffer: self.screen_buffer,
            top_left: tile_flush.top_left,
            width: tile_flush.width,
            height: tile_flush.height,
        };
        render_tile(&mut target, params, tile_flush.origin);
    }
}

struct FullScreenTileTarget<'a, 'b> {
    screen_buffer: &'a mut RectView<'b>,
    top_left: Point,
    width: usize,
    height: usize,
}

impl PixelTarget for FullScreenTileTarget<'_, '_> {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        let screen_x = self.top_left.x + x as i32;
        let screen_y = self.top_left.y + y as i32;
        if screen_x < 0 || screen_y < 0 {
            return;
        }
        let screen_x = screen_x as usize;
        let screen_y = screen_y as usize;
        if screen_x >= SCREEN_WIDTH || screen_y >= SCREEN_HEIGHT {
            return;
        }
        let stride = self.screen_buffer.width();
        self.screen_buffer.raw_pixels_mut()[screen_y * stride + screen_x] =
            Cyd::rgb565(color).into_storage();
    }
}
