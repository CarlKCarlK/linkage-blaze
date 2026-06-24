use core::fmt;

use embedded_graphics::{
    Drawable,
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_9X15_BOLD},
    },
    pixelcolor::IntoStorage,
    prelude::{Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use esp_hal::time::Instant;
use linkage_blaze_core::Rgb888;
use linkage_blaze_cyd::{Cyd, CydError, PixelBuffer, RectPixels, RectView};
use linkage_blaze_core::PixelSurface;
use linkage_blaze_dance_classic::dance_render::{
    BACKGROUND, DANCE_HEIGHT, DANCE_TILE_HEIGHT, DANCE_TILE_PIXELS, DANCE_TILE_WIDTH, DANCE_WIDTH,
    DanceClock, DanceTileSink, PixelTarget, SCREEN_WIDTH, TEXT, TileFlush, WIFI_TEXT_TOP_LEFT,
    format_clock_12h, render_tile,
};
use log::info;
use static_cell::StaticCell;

const SMALL_GLYPH_HEIGHT: usize = 10;
const TIME_GLYPH_WIDTH: usize = 9;
const TIME_GLYPH_HEIGHT: usize = 15;
const TIME_TEXT_TOP: i32 = 6;
const TIME_TEXT_MAX_CHARS: usize = 11;
const TEXT_LINE_WIDTH: usize = 120;
const TEXT_LINE_WORKSPACE_PIXELS: usize = TEXT_LINE_WIDTH * TIME_GLYPH_HEIGHT;

type TextLineWorkspace = PixelBuffer<TEXT_LINE_WORKSPACE_PIXELS>;
type DanceWorkspace = PixelBuffer<DANCE_TILE_PIXELS>;

// Derived Debug reads this payload at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
pub enum CydDanceDisplayError {
    Cyd(CydError),
}

pub struct CydDanceDisplay {
    cyd: Cyd,
    text_line_workspace: &'static mut TextLineWorkspace,
    dance_workspace: &'static mut DanceWorkspace,
    background_cleared: bool,
}

impl CydDanceDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static TEXT_LINE_WORKSPACE: StaticCell<TextLineWorkspace> = StaticCell::new();
        static DANCE_WORKSPACE: StaticCell<DanceWorkspace> = StaticCell::new();

        Self {
            cyd,
            text_line_workspace: TextLineWorkspace::init_static(&TEXT_LINE_WORKSPACE),
            dance_workspace: DanceWorkspace::init_static(&DANCE_WORKSPACE),
            background_cleared: false,
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
            self.cyd.clear(Cyd::rgb565(BACKGROUND))?;
            self.background_cleared = true;
            info!("display background cleared");
        }

        self.show_dance(dance_time)?;

        let mut wifi_text = heapless::String::<32>::new();
        fmt::Write::write_fmt(
            &mut wifi_text,
            format_args!("WiFi {}", wifi_label(wifi_mode)),
        )
        .ok();
        info!("display updating wifi text: {}", wifi_text.as_str());
        self.show_small_text_line(wifi_text.as_str(), WIFI_TEXT_TOP_LEFT, 90)?;

        let time_text = dance_time.map_or("--:--:--", DanceTime::as_str);
        info!("display updating time text: {time_text}");
        self.show_time_text_line(time_text)?;

        info!("display show complete");
        Ok(())
    }

    fn show_time_text_line(&mut self, text: &str) -> Result<(), CydDanceDisplayError> {
        let width = text.chars().count() * TIME_GLYPH_WIDTH;
        assert!(
            width <= TEXT_LINE_WIDTH,
            "time text width must fit workspace"
        );
        let clear_width = TIME_TEXT_MAX_CHARS * TIME_GLYPH_WIDTH;
        let clear_left = (SCREEN_WIDTH as i32 - clear_width as i32) / 2;
        let text_left = clear_left + (clear_width as i32 - width as i32) / 2;
        let top_left = Point::new(text_left, TIME_TEXT_TOP);

        self.cyd.fill_rect(
            Rectangle::new(
                Point::new(clear_left, TIME_TEXT_TOP),
                Size::new(clear_width as u32, TIME_GLYPH_HEIGHT as u32),
            ),
            Cyd::rgb565(BACKGROUND),
        )?;

        let mut text_line_buffer = self.text_line_workspace.view_mut(width, TIME_GLYPH_HEIGHT);
        text_line_buffer.clear(Cyd::rgb565(BACKGROUND));
        Text::with_baseline(
            text,
            Point::new(0, 0),
            MonoTextStyle::new(&FONT_9X15_BOLD, Cyd::rgb565(TEXT)),
            Baseline::Top,
        )
        .draw(&mut text_line_buffer)
        .ok();
        self.cyd.flush(&text_line_buffer, top_left)?;

        Ok(())
    }

    fn show_small_text_line(
        &mut self,
        text: &str,
        top_left: Point,
        width: usize,
    ) -> Result<(), CydDanceDisplayError> {
        assert!(
            width <= TEXT_LINE_WIDTH,
            "text line width must fit workspace"
        );
        self.cyd.fill_rect(
            Rectangle::new(top_left, Size::new(width as u32, SMALL_GLYPH_HEIGHT as u32)),
            Cyd::rgb565(BACKGROUND),
        )?;

        let mut text_line_buffer = self.text_line_workspace.view_mut(width, SMALL_GLYPH_HEIGHT);
        text_line_buffer.clear(Cyd::rgb565(BACKGROUND));
        Text::with_baseline(
            text,
            Point::new(0, 0),
            MonoTextStyle::new(&FONT_6X10, Cyd::rgb565(TEXT)),
            Baseline::Top,
        )
        .draw(&mut text_line_buffer)
        .ok();
        self.cyd.flush(&text_line_buffer, top_left)?;

        Ok(())
    }

    fn show_dance(&mut self, dance_time: Option<&DanceTime>) -> Result<(), CydDanceDisplayError> {
        // Use from_time so hours/minutes reach the placards; from_params would
        // zero them and the signs would always read 12 and 0.
        let dance_clock = dance_time.map_or_else(DanceClock::new, |dance_time| {
            DanceClock::from_time(dance_time.hours, dance_time.minutes, dance_time.seconds)
        });
        let params = dance_clock.params();
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
        let mut sink = EspDanceTileSink {
            cyd: &mut self.cyd,
            dance_workspace: self.dance_workspace,
            result: Ok(()),
        };
        dance_clock.draw_tiles(&mut sink);
        sink.result?;
        let elapsed_ms = (Instant::now() - t0).as_millis();
        info!("dance draw complete: {elapsed_ms} ms");
        Ok(())
    }
}

struct EspDanceTileSink<'a> {
    cyd: &'a mut Cyd,
    dance_workspace: &'a mut DanceWorkspace,
    result: Result<(), CydDanceDisplayError>,
}

impl DanceTileSink for EspDanceTileSink<'_> {
    fn draw_tile(&mut self, tile_flush: TileFlush, params: &[f32; 3], hours: u8, minutes: u8) {
        if self.result.is_err() {
            return;
        }

        let mut dance_buffer = self
            .dance_workspace
            .view_mut(tile_flush.width, tile_flush.height);
        dance_buffer.clear(Cyd::rgb565(BACKGROUND));
        let mut target = RectViewTarget {
            rect_view: &mut dance_buffer,
        };
        render_tile(
            &mut PixelSurface { target: &mut target, tile_origin: tile_flush.origin },
            params,
            hours,
            minutes,
        );
        self.result = self
            .cyd
            .flush(&dance_buffer, tile_flush.top_left)
            .map_err(CydDanceDisplayError::from);
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
        self.rect_view.raw_pixels_mut()[y * stride + x] = Cyd::rgb565(color).into_storage();
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
        let text = format_clock_12h(hours, minutes, seconds);
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
}
