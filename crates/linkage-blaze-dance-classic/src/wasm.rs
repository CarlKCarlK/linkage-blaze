use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{Rgb888, RgbColor},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::dance_render::{
    BG, DANCE_TILE_COLUMNS, DANCE_TILE_HEIGHT, DANCE_TILE_ROWS, DANCE_TILE_WIDTH, PixelTarget,
    SCREEN_HEIGHT, SCREEN_WIDTH, TEXT, TIME_TEXT_TOP_LEFT, TileFlush, WIFI_TEXT_TOP_LEFT,
    dance_params, render_tile,
};

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
        let mut time_text = heapless::String::<16>::new();
        fmt::Write::write_fmt(
            &mut time_text,
            format_args!("{:02}:{:02}:{:02}", hours, minutes, seconds),
        )
        .ok();
        self.render_frame(dance_params(hours, minutes, seconds), time_text.as_str());
    }

    #[wasm_bindgen(js_name = renderParams)]
    pub fn render_params(&mut self, params: Vec<f32>) {
        let params = [
            *params.first().unwrap_or(&0.5),
            *params.get(1).unwrap_or(&0.5),
            *params.get(2).unwrap_or(&0.5),
        ];
        self.render_frame(params, "params");
    }
}

impl DanceClockSim {
    fn render_frame(&mut self, params: [f32; 3], label: &str) {
        fill_frame(&mut self.rgba, BG);
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
                let mut target = RgbaTileTarget {
                    rgba: &mut self.rgba,
                    top_left: tile_flush.top_left,
                    width: tile_flush.width,
                    height: tile_flush.height,
                };
                render_tile(&mut target, &params, tile_flush.origin);
            }
        }
        draw_text(
            &mut self.rgba,
            WIFI_TEXT_TOP_LEFT,
            "WiFi SIM",
            Rectangle::new(WIFI_TEXT_TOP_LEFT, Size::new(90, 10)),
        );

        draw_text(
            &mut self.rgba,
            TIME_TEXT_TOP_LEFT,
            label,
            Rectangle::new(TIME_TEXT_TOP_LEFT, Size::new(72, 10)),
        );
    }
}

impl Default for DanceClockSim {
    fn default() -> Self {
        Self::new()
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
