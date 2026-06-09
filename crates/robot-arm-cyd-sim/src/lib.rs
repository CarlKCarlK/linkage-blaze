#![forbid(unsafe_code)]

use core::{convert::Infallible, f32::consts::FRAC_PI_2};

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use robot_arm_core::{Linkage, Pose, Vec3};
use wasm_bindgen::prelude::wasm_bindgen;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 240;
const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const ARM_WIDTH: i32 = 224;
const SLIDER_LEFT: i32 = 230;
const SLIDER_RIGHT: i32 = 312;
const SLIDER_TOP: i32 = 13;
const SLIDER_STEP: i32 = 32;
const SLIDER_COUNT: usize = 7;

const PARAM_NAMES: [&str; SLIDER_COUNT] =
    ["hand", "elbow", "close", "lower", "spin", "roll", "x/y"];

const LINKAGE: Linkage<6, 24> = Linkage::start()
    .yaw(90.0)
    .yaw_param(4, 180.0, -180.0) // spin whole arm
    .pitch(90.0)
    .forward(2.5)
    .pitch(-90.0)
    .pitch_param(3, 30.0, 0.0) // lower arm
    .forward(3.0)
    .yaw_param(1, 90.0, -90.0) // bend elbow
    .forward(3.0)
    .pitch_param(0, 90.0, -90.0) // lower hand
    .forward(1.0)
    .roll_param(5, 180.0, -180.0) // spin hand
    .forward(0.5)
    .yaw(90.0)
    .move_param(2, 0.5, 0.0) // close hand
    .yaw(-90.0)
    .forward(1.0)
    .yaw(180.0)
    .forward(1.0)
    .yaw(90.0)
    .move_param(2, 1.0, 0.0) // close hand
    .yaw(90.0)
    .forward(1.0);

#[wasm_bindgen]
pub struct CydSim {
    buffer: FrameBuffer,
    params: [f32; 6],
    xy_mix: f32,
    active_slider: Option<usize>,
}

#[wasm_bindgen]
impl CydSim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let mut sim = Self {
            buffer: FrameBuffer::new(),
            params: [0.5, 0.5, 0.0, 0.5, 0.5, 0.5],
            xy_mix: 0.5,
            active_slider: None,
        };
        sim.render();
        sim
    }

    pub fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    pub fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.buffer.rgba()
    }

    pub fn touch_down(&mut self, x: f32, y: f32) {
        self.active_slider = slider_at(y);
        self.update_touch(x);
    }

    pub fn touch_move(&mut self, x: f32, _y: f32) {
        self.update_touch(x);
    }

    pub fn touch_up(&mut self) {
        self.active_slider = None;
    }
}

impl Default for CydSim {
    fn default() -> Self {
        Self::new()
    }
}

impl CydSim {
    fn update_touch(&mut self, x: f32) {
        let Some(slider_index) = self.active_slider else {
            return;
        };

        let value =
            ((x - SLIDER_LEFT as f32) / (SLIDER_RIGHT - SLIDER_LEFT) as f32).clamp(0.0, 1.0);
        if slider_index < self.params.len() {
            self.params[slider_index] = value;
        } else {
            self.xy_mix = value;
        }
        self.render();
    }

    fn render(&mut self) {
        self.buffer.clear(Rgb565::BLACK);
        self.draw_viewport();
        self.draw_sliders();
    }

    fn draw_viewport(&mut self) {
        let border = PrimitiveStyle::with_stroke(Rgb565::CSS_GRAY, 1);
        Rectangle::new(
            Point::new(0, 0),
            Size::new(ARM_WIDTH as u32, SCREEN_HEIGHT as u32),
        )
        .into_styled(border)
        .draw(&mut self.buffer)
        .ok();

        let bounds = self.arm_bounds();
        let mut previous: Option<Point> = None;
        for pose in LINKAGE.poses(&self.params) {
            let point = self.pose_to_screen(pose, bounds);
            if let Some(previous_point) = previous {
                Line::new(previous_point, point)
                    .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_DARK_CYAN, 3))
                    .draw(&mut self.buffer)
                    .ok();
            }
            Circle::with_center(point, 7)
                .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_DARK_CYAN))
                .draw(&mut self.buffer)
                .ok();
            previous = Some(point);
        }
    }

    fn draw_sliders(&mut self) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        for slider_index in 0..SLIDER_COUNT {
            let y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
            let value = if slider_index < self.params.len() {
                self.params[slider_index]
            } else {
                self.xy_mix
            };
            Text::with_baseline(
                PARAM_NAMES[slider_index],
                Point::new(SLIDER_LEFT, y - 10),
                text_style,
                Baseline::Top,
            )
            .draw(&mut self.buffer)
            .ok();

            Line::new(Point::new(SLIDER_LEFT, y), Point::new(SLIDER_RIGHT, y))
                .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 2))
                .draw(&mut self.buffer)
                .ok();

            let knob_x = SLIDER_LEFT + ((SLIDER_RIGHT - SLIDER_LEFT) as f32 * value).round() as i32;
            Circle::with_center(Point::new(knob_x, y), 9)
                .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW))
                .draw(&mut self.buffer)
                .ok();
        }
    }

    fn arm_bounds(&self) -> Bounds {
        let mut bounds = Bounds::new();
        for pose in LINKAGE.poses(&self.params) {
            let Vec3([x, y, z]) = pose.position();
            bounds.include(project_horizontal(x, y, self.xy_mix), -z);
        }
        bounds
    }

    fn pose_to_screen(&self, pose: Pose, bounds: Bounds) -> Point {
        let Vec3([x, y, z]) = pose.position();
        let horizontal = project_horizontal(x, y, self.xy_mix);
        let vertical = -z;
        let scale_x = (ARM_WIDTH as f32 - 24.0) / bounds.width().max(1.0);
        let scale_y = (SCREEN_HEIGHT as f32 - 24.0) / bounds.height().max(1.0);
        let scale = scale_x.min(scale_y);
        let center_x = (bounds.min_x + bounds.max_x) * 0.5;
        let center_y = (bounds.min_y + bounds.max_y) * 0.5;
        let screen_x = ARM_WIDTH as f32 * 0.5 + (horizontal - center_x) * scale;
        let screen_y = SCREEN_HEIGHT as f32 * 0.5 - (vertical - center_y) * scale;
        Point::new(screen_x.round() as i32, screen_y.round() as i32)
    }
}

fn slider_at(y: f32) -> Option<usize> {
    for slider_index in 0..SLIDER_COUNT {
        let slider_y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
        if (y - slider_y as f32).abs() <= 13.0 {
            return Some(slider_index);
        }
    }
    None
}

fn project_horizontal(x: f32, y: f32, xy_mix: f32) -> f32 {
    let angle = xy_mix * FRAC_PI_2;
    x * angle.cos() + y * angle.sin()
}

#[derive(Clone, Copy)]
struct Bounds {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

impl Bounds {
    fn new() -> Self {
        Self {
            min_x: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            min_y: f32::INFINITY,
            max_y: f32::NEG_INFINITY,
        }
    }

    fn include(&mut self, x: f32, y: f32) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    fn width(self) -> f32 {
        self.max_x - self.min_x
    }

    fn height(self) -> f32 {
        self.max_y - self.min_y
    }
}

struct FrameBuffer {
    pixels: [Rgb565; SCREEN_PIXELS],
}

impl FrameBuffer {
    fn new() -> Self {
        Self {
            pixels: [Rgb565::BLACK; SCREEN_PIXELS],
        }
    }

    fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color);
    }

    fn rgba(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(SCREEN_PIXELS * 4);
        for pixel in self.pixels {
            bytes.push(scale_rgb565_channel(pixel.r(), 31));
            bytes.push(scale_rgb565_channel(pixel.g(), 63));
            bytes.push(scale_rgb565_channel(pixel.b(), 31));
            bytes.push(255);
        }
        bytes
    }
}

impl DrawTarget for FrameBuffer {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let x = point.x as usize;
            let y = point.y as usize;
            if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
                continue;
            }
            self.pixels[y * SCREEN_WIDTH + x] = color;
        }
        Ok(())
    }
}

impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }
}

fn scale_rgb565_channel(value: u8, max: u8) -> u8 {
    ((u16::from(value) * 255) / u16::from(max)) as u8
}
