#![forbid(unsafe_code)]

use core::{convert::Infallible, f32::consts::TAU};

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle},
    text::{Baseline, Text},
};
use robot_arm_core::{Linkage, Pose, Vec3};
use wasm_bindgen::prelude::wasm_bindgen;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 240;
const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const HORIZONTAL_MIN: f32 = -8.0;
const HORIZONTAL_MAX: f32 = 8.0;
const Z_MIN: f32 = 0.0;
const Z_MAX: f32 = 10.0;
const TILT_X: i32 = 16;
const TILT_TOP: i32 = 24;
const TILT_BOTTOM: i32 = 224;
const SLIDER_LEFT: i32 = 230;
const SLIDER_RIGHT: i32 = 312;
const SLIDER_TRACK_LEFT: i32 = 230;
const SLIDER_TOP: i32 = 13;
const SLIDER_STEP: i32 = 32;
const SLIDER_COUNT: usize = 7;

const PARAM_NAMES: [&str; SLIDER_COUNT] = [
    "lower hand",
    "bend elbow",
    "close hand",
    "lower arm",
    "spin whole",
    "spin hand",
    "x/y view",
];

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
    z_mix: f32,
    active_control: Option<ActiveControl>,
}

#[wasm_bindgen]
impl CydSim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let mut sim = Self {
            buffer: FrameBuffer::new(),
            params: [0.5, 0.5, 0.0, 0.5, 0.5, 0.5],
            xy_mix: 0.5,
            z_mix: 0.0,
            active_control: None,
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
        self.active_control = control_at(x, y);
        self.update_touch(x, y);
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        self.update_touch(x, y);
    }

    pub fn touch_up(&mut self) {
        self.active_control = None;
    }
}

impl Default for CydSim {
    fn default() -> Self {
        Self::new()
    }
}

impl CydSim {
    fn update_touch(&mut self, x: f32, y: f32) {
        let Some(active_control) = self.active_control else {
            return;
        };

        match active_control {
            ActiveControl::RightSlider(slider_index) => {
                let value = ((x - SLIDER_TRACK_LEFT as f32)
                    / (SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32)
                    .clamp(0.0, 1.0);
                if slider_index < self.params.len() {
                    self.params[slider_index] = value;
                } else {
                    self.xy_mix = value;
                }
            }
            ActiveControl::Tilt => {
                self.z_mix =
                    (1.0 - (y - TILT_TOP as f32) / (TILT_BOTTOM - TILT_TOP) as f32).clamp(0.0, 1.0);
            }
        }
        self.render();
    }

    fn render(&mut self) {
        self.buffer.clear(Rgb565::BLACK);
        self.draw_grid();
        self.draw_sliders();
        self.draw_arm();
    }

    fn draw_grid(&mut self) {
        let style = PrimitiveStyle::with_stroke(Rgb565::CSS_DARK_SLATE_GRAY, 1);
        for horizontal in [-8.0, -4.0, 0.0, 4.0, 8.0] {
            let x = horizontal_to_screen(horizontal);
            Line::new(Point::new(x, 0), Point::new(x, SCREEN_HEIGHT as i32 - 1))
                .into_styled(style)
                .draw(&mut self.buffer)
                .ok();
        }
        for z in [0.0, 2.5, 5.0, 7.5, 10.0] {
            let y = vertical_to_screen(z, self.z_mix);
            Line::new(Point::new(0, y), Point::new(SCREEN_WIDTH as i32 - 1, y))
                .into_styled(style)
                .draw(&mut self.buffer)
                .ok();
        }
    }

    fn draw_arm(&mut self) {
        let mut previous: Option<Point> = None;
        for pose in LINKAGE.poses(&self.params) {
            let point = self.pose_to_screen(pose);
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
        Text::with_baseline("z/depth", Point::new(4, 5), text_style, Baseline::Top)
            .draw(&mut self.buffer)
            .ok();
        Line::new(
            Point::new(TILT_X, TILT_TOP),
            Point::new(TILT_X, TILT_BOTTOM),
        )
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 2))
        .draw(&mut self.buffer)
        .ok();
        let tilt_knob_y =
            TILT_TOP + ((TILT_BOTTOM - TILT_TOP) as f32 * (1.0 - self.z_mix)).round() as i32;
        Circle::with_center(Point::new(TILT_X, tilt_knob_y), 9)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW))
            .draw(&mut self.buffer)
            .ok();

        for slider_index in 0..SLIDER_COUNT {
            let y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
            let value = if slider_index < self.params.len() {
                self.params[slider_index]
            } else {
                self.xy_mix
            };

            Text::with_baseline(
                PARAM_NAMES[slider_index],
                Point::new(SLIDER_LEFT, y - 12),
                text_style,
                Baseline::Top,
            )
            .draw(&mut self.buffer)
            .ok();

            Line::new(
                Point::new(SLIDER_TRACK_LEFT, y + 8),
                Point::new(SLIDER_RIGHT, y + 8),
            )
            .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 2))
            .draw(&mut self.buffer)
            .ok();

            let knob_x = SLIDER_TRACK_LEFT
                + ((SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32 * value).round() as i32;
            Circle::with_center(Point::new(knob_x, y + 8), 9)
                .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW))
                .draw(&mut self.buffer)
                .ok();
        }
    }

    fn pose_to_screen(&self, pose: Pose) -> Point {
        let Vec3([x, y, z]) = pose.position();
        let projection = project(x, y, -z, self.xy_mix, self.z_mix);
        Point::new(
            horizontal_to_screen(projection.horizontal),
            vertical_to_screen(projection.vertical, self.z_mix),
        )
    }
}

#[derive(Clone, Copy)]
enum ActiveControl {
    RightSlider(usize),
    Tilt,
}

#[derive(Clone, Copy)]
struct Projection {
    horizontal: f32,
    vertical: f32,
}

fn project(x: f32, y: f32, z: f32, xy_mix: f32, z_mix: f32) -> Projection {
    let angle = (xy_mix - 0.5) * TAU;
    let cos = angle.cos();
    let sin = angle.sin();
    let horizontal = x * cos + y * sin;
    let depth = -x * sin + y * cos;
    Projection {
        horizontal,
        vertical: z * (1.0 - z_mix) + depth * z_mix,
    }
}

fn control_at(x: f32, y: f32) -> Option<ActiveControl> {
    if (x - TILT_X as f32).abs() <= 14.0 && (TILT_TOP as f32..=TILT_BOTTOM as f32).contains(&y) {
        return Some(ActiveControl::Tilt);
    }
    for slider_index in 0..SLIDER_COUNT {
        let slider_y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
        if x >= SLIDER_LEFT as f32 && (y - (slider_y + 8) as f32).abs() <= 13.0 {
            return Some(ActiveControl::RightSlider(slider_index));
        }
    }
    None
}

fn horizontal_to_screen(horizontal: f32) -> i32 {
    scale_to_screen(horizontal, HORIZONTAL_MIN, HORIZONTAL_MAX, SCREEN_WIDTH)
}

fn vertical_to_screen(vertical: f32, z_mix: f32) -> i32 {
    let low = Z_MIN * (1.0 - z_mix) + HORIZONTAL_MIN * z_mix;
    let high = Z_MAX * (1.0 - z_mix) + HORIZONTAL_MAX * z_mix;
    (SCREEN_HEIGHT as i32 - 1) - scale_to_screen(vertical, low, high, SCREEN_HEIGHT)
}

fn scale_to_screen(value: f32, low: f32, high: f32, pixels: usize) -> i32 {
    let fraction = ((value - low) / (high - low)).clamp(0.0, 1.0);
    (fraction * (pixels - 1) as f32).round() as i32
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
