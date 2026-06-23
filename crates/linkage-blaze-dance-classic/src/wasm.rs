use alloc::vec;
use alloc::vec::Vec;

use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_9X15_BOLD},
    },
    pixelcolor::{Rgb888, RgbColor},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::dance_render::{
    BG, DanceClock, DanceTileSink, PixelTarget, SCREEN_HEIGHT, SCREEN_WIDTH, TEXT, TileFlush,
    WIFI_TEXT_TOP_LEFT, format_clock_12h, render_tile,
};

// Top time text: bold and bigger than the tiny "WiFi SIM" status, centered.
const TIME_FONT_W: i32 = 9; // FONT_9X15_BOLD glyph width
const TIME_FONT_H: i32 = 15;
const TIME_TEXT_TOP: i32 = 6;

const RGBA_CHANNELS: usize = 4;

#[wasm_bindgen]
pub struct DanceClockSim {
    rgba: Vec<u8>,
}

#[wasm_bindgen]
impl DanceClockSim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            rgba: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT * RGBA_CHANNELS],
        }
    }

    #[wasm_bindgen]
    pub fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    #[wasm_bindgen]
    pub fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    #[wasm_bindgen]
    pub fn rgba(&self) -> Vec<u8> {
        self.rgba.clone()
    }

    #[wasm_bindgen(js_name = renderTime)]
    pub fn render_time(&mut self, hours: u8, minutes: u8, seconds: u8) {
        let time_text = format_clock_12h(hours, minutes, seconds);
        self.render_frame(
            DanceClock::from_time(hours, minutes, seconds),
            time_text.as_str(),
        );
    }

    #[wasm_bindgen(js_name = renderParams)]
    pub fn render_params(&mut self, params: Vec<f32>) {
        let params = [
            *params.first().unwrap_or(&0.5),
            *params.get(1).unwrap_or(&0.5),
            *params.get(2).unwrap_or(&0.5),
        ];
        self.render_frame(DanceClock::from_params(params), "params");
    }
}

impl DanceClockSim {
    fn render_frame(&mut self, dance_clock: DanceClock, label: &str) {
        fill_frame(&mut self.rgba, BG);
        let mut tile_sink = RgbaDanceTileSink {
            rgba: &mut self.rgba,
        };
        dance_clock.draw_tiles(&mut tile_sink);
        draw_text(
            &mut self.rgba,
            WIFI_TEXT_TOP_LEFT,
            "WiFi SIM",
            Rectangle::new(WIFI_TEXT_TOP_LEFT, Size::new(90, 10)),
        );

        draw_time(&mut self.rgba, label);
    }
}

impl Default for DanceClockSim {
    fn default() -> Self {
        Self::new()
    }
}

struct RgbaDanceTileSink<'a> {
    rgba: &'a mut [u8],
}

impl DanceTileSink for RgbaDanceTileSink<'_> {
    fn draw_tile(&mut self, tile_flush: TileFlush, params: &[f32; 3], hours: u8, minutes: u8) {
        let mut target = RgbaTileTarget {
            rgba: self.rgba,
            top_left: tile_flush.top_left,
            width: tile_flush.width,
            height: tile_flush.height,
        };
        render_tile(&mut target, params, tile_flush.origin, hours, minutes);
    }
}

struct RgbaTileTarget<'a> {
    rgba: &'a mut [u8],
    top_left: Point,
    width: usize,
    height: usize,
}

impl PixelTarget for RgbaTileTarget<'_> {
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
        put_screen_pixel(self.rgba, screen_x as usize, screen_y as usize, color);
    }
}

struct RgbaDrawTarget<'a> {
    rgba: &'a mut [u8],
}

impl DrawTarget for RgbaDrawTarget<'_> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            put_screen_pixel(self.rgba, point.x as usize, point.y as usize, color);
        }
        Ok(())
    }
}

impl OriginDimensions for RgbaDrawTarget<'_> {
    fn size(&self) -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }
}

fn draw_time(rgba: &mut [u8], text: &str) {
    let field_w = text.chars().count() as i32 * TIME_FONT_W;
    let left = (SCREEN_WIDTH as i32 - field_w) / 2;
    let top_left = Point::new(left, TIME_TEXT_TOP);
    fill_rect(
        rgba,
        Rectangle::new(top_left, Size::new(field_w as u32, TIME_FONT_H as u32)),
        BG,
    );
    let mut target = RgbaDrawTarget { rgba };
    Text::with_baseline(
        text,
        top_left,
        MonoTextStyle::new(&FONT_9X15_BOLD, TEXT),
        Baseline::Top,
    )
    .draw(&mut target)
    .ok();
}

fn draw_text(rgba: &mut [u8], top_left: Point, text: &str, clear_rect: Rectangle) {
    fill_rect(rgba, clear_rect, BG);
    let mut target = RgbaDrawTarget { rgba };
    Text::with_baseline(
        text,
        top_left,
        MonoTextStyle::new(&FONT_6X10, TEXT),
        Baseline::Top,
    )
    .draw(&mut target)
    .ok();
}

fn fill_frame(rgba: &mut [u8], color: Rgb888) {
    fill_rect(
        rgba,
        Rectangle::new(
            Point::new(0, 0),
            Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
        ),
        color,
    );
}

fn fill_rect(rgba: &mut [u8], rectangle: Rectangle, color: Rgb888) {
    let start_x = rectangle.top_left.x.max(0) as usize;
    let start_y = rectangle.top_left.y.max(0) as usize;
    let end_x = (rectangle.top_left.x + rectangle.size.width as i32)
        .min(SCREEN_WIDTH as i32)
        .max(0) as usize;
    let end_y = (rectangle.top_left.y + rectangle.size.height as i32)
        .min(SCREEN_HEIGHT as i32)
        .max(0) as usize;

    for y in start_y..end_y {
        for x in start_x..end_x {
            put_screen_pixel(rgba, x, y, color);
        }
    }
}

fn put_screen_pixel(rgba: &mut [u8], x: usize, y: usize, color: Rgb888) {
    if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
        return;
    }
    let index = (y * SCREEN_WIDTH + x) * RGBA_CHANNELS;
    rgba[index] = color.r();
    rgba[index + 1] = color.g();
    rgba[index + 2] = color.b();
    rgba[index + 3] = 255;
}
