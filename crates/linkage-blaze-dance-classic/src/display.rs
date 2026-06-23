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
    BG, DANCE_HEIGHT, DANCE_TILE_HEIGHT, DANCE_TILE_PIXELS, DANCE_TILE_WIDTH, DANCE_WIDTH,
    DanceClock, DanceTileSink, PixelTarget, TEXT, TIME_TEXT_TOP_LEFT, TileFlush,
    WIFI_TEXT_TOP_LEFT, dance_params, format_clock_12h, render_tile,
};
use log::info;
use static_cell::StaticCell;

const SMALL_GLYPH_HEIGHT: usize = 10;
const TEXT_LINE_WIDTH: usize = 90;
const TEXT_LINE_WORKSPACE_PIXELS: usize = TEXT_LINE_WIDTH * SMALL_GLYPH_HEIGHT;

type TextLineWorkspace = RectWorkspace<TEXT_LINE_WORKSPACE_PIXELS>;
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
    text_line_workspace: &'static mut TextLineWorkspace,
    dance_workspace: &'static mut DanceWorkspace,
    background_cleared: bool,
    last_time_text: heapless::String<16>,
    last_wifi_text: heapless::String<32>,
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

        self.show_dance(dance_time)?;

        let mut wifi_text = heapless::String::<32>::new();
        fmt::Write::write_fmt(
            &mut wifi_text,
            format_args!("WiFi {}", wifi_label(wifi_mode)),
        )
        .ok();
        info!("display updating wifi text: {}", wifi_text.as_str());
        self.show_small_text_line(wifi_text.as_str(), WIFI_TEXT_TOP_LEFT, 90)?;
        self.last_wifi_text.clear();
        self.last_wifi_text.push_str(wifi_text.as_str()).ok();

        let time_text = dance_time.map_or("--:--:--", DanceTime::as_str);
        info!("display updating time text: {time_text}");
        self.show_small_text_line(time_text, TIME_TEXT_TOP_LEFT, 72)?;
        self.last_time_text.clear();
        self.last_time_text.push_str(time_text).ok();

        info!("display show complete");
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
        self.cyd.fill_rect_now(
            Rectangle::new(top_left, Size::new(width as u32, SMALL_GLYPH_HEIGHT as u32)),
            rgb565(BG),
        )?;

        let mut text_line_buffer = self.text_line_workspace.view_mut(width, SMALL_GLYPH_HEIGHT);
        text_line_buffer.clear(rgb565(BG));
        Text::with_baseline(
            text,
            Point::new(0, 0),
            MonoTextStyle::new(&FONT_6X10, rgb565(TEXT)),
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
        dance_buffer.clear(rgb565(BG));
        let mut target = RectViewTarget {
            rect_view: &mut dance_buffer,
        };
        render_tile(&mut target, params, tile_flush.origin, hours, minutes);
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

    fn params(&self) -> [f32; 3] {
        dance_params(self.hours, self.minutes, self.seconds)
    }
}
