use core::fmt;

use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{IntoStorage, Rgb565},
    prelude::{Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use esp_hal::time::Instant;
use linkage_blaze_ballet::{
    ballet_frames::{BALLET_DOF, BALLET_FRAME_COUNT},
    ballet_render::{
        BG, BalletTileSink, PixelTarget, SCREEN_HEIGHT, SCREEN_WIDTH, TEXT, TileFlush, draw_tiles,
        render_tile,
    },
};
use linkage_blaze_core::Rgb888;
use linkage_blaze_cyd::{Cyd, CydError, RectPixels, RectView, RectWorkspace};
use static_cell::StaticCell;

const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const STATUS_HEIGHT: usize = 10;
const SOURCE_FPS_X10: u32 = 1200;

type ScreenWorkspace = RectWorkspace<SCREEN_PIXELS>;

fn rgb565(color: Rgb888) -> Rgb565 {
    Rgb565::from(color)
}

pub enum CydBalletDisplayError {
    Cyd(CydError),
}

impl fmt::Debug for CydBalletDisplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CydBalletDisplayError::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
        }
    }
}

impl From<CydError> for CydBalletDisplayError {
    fn from(error: CydError) -> Self {
        Self::Cyd(error)
    }
}

pub struct CydBalletDisplay {
    cyd: Cyd,
    screen_workspace: &'static mut ScreenWorkspace,
    background_cleared: bool,
    last_frame_ms: u64,
}

impl CydBalletDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static SCREEN_WORKSPACE: StaticCell<ScreenWorkspace> = StaticCell::new();

        Self {
            cyd,
            screen_workspace: ScreenWorkspace::init_static(&SCREEN_WORKSPACE),
            background_cleared: false,
            last_frame_ms: 0,
        }
    }

    pub fn show_frame(
        &mut self,
        frame_index: usize,
        params: &[f32; BALLET_DOF],
    ) -> Result<(), CydBalletDisplayError> {
        if !self.background_cleared {
            self.cyd.clear_now(rgb565(BG))?;
            self.background_cleared = true;
        }

        let started = Instant::now();
        let mut screen_buffer = self.screen_workspace.view_mut(SCREEN_WIDTH, SCREEN_HEIGHT);
        screen_buffer.clear(rgb565(BG));
        let mut sink = EspBalletTileSink {
            screen_buffer: &mut screen_buffer,
        };
        draw_tiles(params, &mut sink);
        self.draw_status(&mut screen_buffer, frame_index);
        self.cyd.flush(&screen_buffer, Point::new(0, 0))?;
        self.last_frame_ms = (Instant::now() - started).as_millis();
        Ok(())
    }

    fn draw_status(&mut self, screen_buffer: &mut RectView<'_>, frame_index: usize) {
        let elapsed_ms = self.last_frame_ms.max(1);
        let fps_x10 = (10_000 / elapsed_ms) as u32;
        let slomo_x10 = if fps_x10 == 0 {
            0
        } else {
            (SOURCE_FPS_X10 * 10 + fps_x10 / 2) / fps_x10
        };
        let mut status = heapless::String::<64>::new();
        fmt::Write::write_fmt(
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
            MonoTextStyle::new(&FONT_6X10, rgb565(TEXT)),
            Baseline::Top,
        )
        .draw(screen_buffer)
        .ok();
    }
}

struct EspBalletTileSink<'a> {
    screen_buffer: &'a mut RectView<'a>,
}

impl BalletTileSink for EspBalletTileSink<'_> {
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
            rgb565(color).into_storage();
    }
}
