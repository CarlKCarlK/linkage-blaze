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
use linkage_blaze_core::Rgb888;
use linkage_blaze_cyd::{Cyd, CydError, RectPixels, RectView, RectWorkspace};
use linkage_blaze_dance_classic::dance_render::{
    BG, DANCE_HEIGHT, DANCE_TILE_COLUMNS, DANCE_TILE_HEIGHT, DANCE_TILE_PIXELS, DANCE_TILE_ROWS,
    DANCE_TILE_WIDTH, DANCE_WIDTH, PixelTarget, TEXT, TIME_TEXT_TOP_LEFT, TileFlush,
    WIFI_TEXT_TOP_LEFT, dance_params, render_tile,
};
use log::info;
use static_cell::StaticCell;

const SMALL_GLYPH_WIDTH: usize = 6;
const SMALL_GLYPH_HEIGHT: usize = 10;
const GLYPH_WORKSPACE_PIXELS: usize = SMALL_GLYPH_WIDTH * SMALL_GLYPH_HEIGHT;

type GlyphWorkspace = RectWorkspace<GLYPH_WORKSPACE_PIXELS>;
type DanceWorkspace = RectWorkspace<DANCE_TILE_PIXELS>;

fn rgb565(color: Rgb888) -> Rgb565 {
    Rgb565::from(color)
}

pub enum CydDanceDisplayError {
    Cyd(CydError),
}

impl fmt::Debug for CydDanceDisplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CydDanceDisplayError::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
        }
    }
}

impl From<CydError> for CydDanceDisplayError {
    fn from(error: CydError) -> Self {
        Self::Cyd(error)
    }
}

pub struct CydDanceDisplay {
    cyd: Cyd,
    glyph_workspace: &'static mut GlyphWorkspace,
    dance_workspace: &'static mut DanceWorkspace,
    background_cleared: bool,
    last_time_text: heapless::String<16>,
    last_wifi_text: heapless::String<32>,
}

impl CydDanceDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static GLYPH_WORKSPACE: StaticCell<GlyphWorkspace> = StaticCell::new();
        static DANCE_WORKSPACE: StaticCell<DanceWorkspace> = StaticCell::new();

        Self {
            cyd,
            glyph_workspace: GlyphWorkspace::init_static(&GLYPH_WORKSPACE),
            dance_workspace: DanceWorkspace::init_static(&DANCE_WORKSPACE),
            background_cleared: false,
            last_time_text: heapless::String::new(),
            last_wifi_text: heapless::String::new(),
        }
    }

    pub fn show(
        &mut self,
        wifi_mode: &str,
        dance_time: Option<&DanceTime>,
    ) -> Result<(), CydDanceDisplayError> {
        info!(
            "display show start: wifi_mode={wifi_mode}, has_time={}",
            dance_time.is_some()
        );
        if !self.background_cleared {
            info!("display clearing background");
            self.cyd.clear_now(rgb565(BG))?;
            self.background_cleared = true;
            info!("display background cleared");
        }

        let mut wifi_text = heapless::String::<32>::new();
        fmt::Write::write_fmt(
            &mut wifi_text,
            format_args!("WiFi {}", wifi_label(wifi_mode)),
        )
        .ok();
        if wifi_text.as_str() != self.last_wifi_text.as_str() {
            info!("display updating wifi text: {}", wifi_text.as_str());
            self.show_small_text_line(wifi_text.as_str(), WIFI_TEXT_TOP_LEFT, 90)?;
            self.last_wifi_text.clear();
            self.last_wifi_text.push_str(wifi_text.as_str()).ok();
        }

        let time_text = dance_time.map_or("--:--:--", DanceTime::as_str);
        if time_text != self.last_time_text.as_str() {
            info!("display updating time text: {time_text}");
            self.show_small_text_line(time_text, TIME_TEXT_TOP_LEFT, 72)?;
            self.last_time_text.clear();
            self.last_time_text.push_str(time_text).ok();
        }

        self.show_dance(dance_time)?;

        info!("display show complete");
        Ok(())
    }

    fn show_small_text_line(
        &mut self,
        text: &str,
        top_left: Point,
        width: usize,
    ) -> Result<(), CydDanceDisplayError> {
        self.cyd.fill_rect_now(
            Rectangle::new(top_left, Size::new(width as u32, SMALL_GLYPH_HEIGHT as u32)),
            rgb565(BG),
        )?;

        let mut glyph_left = top_left.x;
        for character in text.chars() {
            let mut character_text = heapless::String::<4>::new();
            fmt::Write::write_char(&mut character_text, character).ok();
            let mut glyph_buffer = self
                .glyph_workspace
                .view_mut(SMALL_GLYPH_WIDTH, SMALL_GLYPH_HEIGHT);
            glyph_buffer.clear(rgb565(BG));
            Text::with_baseline(
                character_text.as_str(),
                Point::new(0, 0),
                MonoTextStyle::new(&FONT_6X10, rgb565(TEXT)),
                Baseline::Top,
            )
            .draw(&mut glyph_buffer)
            .ok();
            self.cyd
                .flush(&glyph_buffer, Point::new(glyph_left, top_left.y))?;
            glyph_left += SMALL_GLYPH_WIDTH as i32;
        }

        Ok(())
    }

    fn show_dance(&mut self, dance_time: Option<&DanceTime>) -> Result<(), CydDanceDisplayError> {
        let params = dance_time.map_or([0.5; 3], DanceTime::params);
        info!(
            "dance draw start: params=({:.3}, {:.3}, {:.3}), area={}x{}, tile={}x{}",
            params[0],
            params[1],
            params[2],
            DANCE_WIDTH,
            DANCE_HEIGHT,
            DANCE_TILE_WIDTH,
            DANCE_TILE_HEIGHT
        );
        let t0 = Instant::now();
        for tile_row in 0..DANCE_TILE_ROWS {
            for tile_column in 0..DANCE_TILE_COLUMNS {
                let tile_x = tile_column * DANCE_TILE_WIDTH;
                let tile_y = tile_row * DANCE_TILE_HEIGHT;
                let tile_origin = Point::new(tile_x as i32, tile_y as i32);
                let Some(tile_flush) =
                    TileFlush::new(tile_origin, DANCE_TILE_WIDTH, DANCE_TILE_HEIGHT)
                else {
                    continue;
                };
                let mut dance_buffer = self
                    .dance_workspace
                    .view_mut(tile_flush.width, tile_flush.height);
                dance_buffer.clear(rgb565(BG));
                let mut target = RectViewTarget {
                    rect_view: &mut dance_buffer,
                };
                render_tile(&mut target, &params, tile_flush.origin);
                self.cyd.flush(&dance_buffer, tile_flush.top_left)?;
            }
        }
        let elapsed_ms = (Instant::now() - t0).as_millis();
        info!("dance draw complete: {elapsed_ms} ms");
        Ok(())
    }
}

struct RectViewTarget<'a, 'b> {
    rect_view: &'a mut RectView<'b>,
}

impl PixelTarget for RectViewTarget<'_, '_> {
    fn width(&self) -> usize {
        self.rect_view.width()
    }

    fn height(&self) -> usize {
        self.rect_view.height()
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        let stride = self.rect_view.width();
        self.rect_view.raw_pixels_mut()[y * stride + x] = rgb565(color).into_storage();
    }
}

fn wifi_label(wifi_mode: &str) -> &str {
    match wifi_mode {
        "connected" => "OK",
        "connecting" => "...",
        "connect failed" => "fail",
        "setup CydDance" => "setup",
        _ => wifi_mode,
    }
}

pub struct DanceTime {
    text: heapless::String<16>,
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl DanceTime {
    pub fn new(hours: u8, minutes: u8, seconds: u8) -> Result<Self, fmt::Error> {
        let mut text = heapless::String::<16>::new();
        fmt::Write::write_fmt(
            &mut text,
            format_args!("{:02}:{:02}:{:02}", hours, minutes, seconds),
        )?;
        Ok(Self {
            text,
            hours,
            minutes,
            seconds,
        })
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn params(&self) -> [f32; 3] {
        dance_params(self.hours, self.minutes, self.seconds)
    }
}
