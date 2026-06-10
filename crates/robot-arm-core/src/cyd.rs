use core::{convert::Infallible, f32::consts::TAU};

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use nanorand::{Rng, WyRand};

use crate::{Linkage, Pose, Vec3};

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 240;
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

const HORIZONTAL_MIN: f32 = -8.0;
const HORIZONTAL_MAX: f32 = 8.0;
const Z_MIN: f32 = 0.0;
const Z_MAX: f32 = 10.0;
const TILT_X: i32 = 16;
const ZOOM_X: i32 = 42;
const TILT_TOP: i32 = 24;
const TILT_BOTTOM: i32 = 224;
const ZOOM_TOP: i32 = 24;
const ZOOM_BOTTOM: i32 = 74;
const SLIDER_LEFT: i32 = 230;
const SLIDER_RIGHT: i32 = 312;
const SLIDER_TRACK_LEFT: i32 = 230;
const SLIDER_TOP: i32 = 24;
const SLIDER_STEP: i32 = 32;
const SLIDER_COUNT: usize = 6;
const VIEW_SLIDER_LEFT: i32 = 40;
const VIEW_SLIDER_RIGHT: i32 = 280;
const VIEW_SLIDER_Y: i32 = 226;
const TEXT_CHAR_WIDTH: i32 = 6;
const DISTANCE_REPORT_WIDTH: i32 = 10 * TEXT_CHAR_WIDTH;
const DISTANCE_REPORT_LEFT: i32 = (SCREEN_WIDTH as i32 - DISTANCE_REPORT_WIDTH) / 2;
const TARGET_CONTROL_TOP: i32 = 17;
const TARGET_BUTTON_WIDTH: u32 = 42;
const TARGET_BUTTON_HEIGHT: u32 = 14;
const TARGET_BUTTON_LABEL_WIDTH: i32 = 4 * TEXT_CHAR_WIDTH;
const TARGET_LABEL_WIDTH: i32 = 6 * TEXT_CHAR_WIDTH;
const TARGET_CONTROL_GAP: i32 = 4;
const TARGET_CONTROL_WIDTH: i32 =
    TARGET_BUTTON_WIDTH as i32 * 2 + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP * 2;
const PREV_BUTTON_LEFT: i32 = (SCREEN_WIDTH as i32 - TARGET_CONTROL_WIDTH) / 2;
const TARGET_LABEL_LEFT: i32 = PREV_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32 + TARGET_CONTROL_GAP;
const NEXT_BUTTON_LEFT: i32 = TARGET_LABEL_LEFT + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP;
const TARGET_MIN_DIAMETER: f32 = 0.1;
const TARGET_MAX_DIAMETER: f32 = 0.9;
const HAND_CORNER_POSE_INDICES: [usize; 4] = [15, 17, 21, 23];

const PARAM_NAMES: [&str; SLIDER_COUNT] = [
    "lower hand",
    "bend elbow",
    "close hand",
    "lower arm",
    "spin whole",
    "spin hand",
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

pub struct CydSim {
    buffer: FrameBuffer,
    params: [f32; 6],
    xy_mix: f32,
    z_mix: f32,
    zoom: f32,
    target_seed: u32,
    active_control: Option<ActiveControl>,
}

impl CydSim {
    #[must_use]
    pub fn new() -> Self {
        let mut sim = Self {
            buffer: FrameBuffer::new(),
            params: [0.5, 0.5, 0.0, 0.5, 0.5, 0.5],
            xy_mix: 0.5 + 30.0 / 360.0,
            z_mix: 0.3,
            zoom: 0.5,
            target_seed: 0,
            active_control: None,
        };
        sim.render();
        sim
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    #[must_use]
    pub fn pixels(&self) -> &[Rgb565; SCREEN_PIXELS] {
        self.buffer.pixels()
    }

    pub fn touch_down(&mut self, x: f32, y: f32) {
        self.active_control = control_at(x, y);
        if matches!(self.active_control, Some(ActiveControl::PreviousTarget)) {
            self.target_seed = self.target_seed.wrapping_sub(1);
            self.active_control = None;
            self.render();
            return;
        }
        if matches!(self.active_control, Some(ActiveControl::NextTarget)) {
            self.target_seed = self.target_seed.wrapping_add(1);
            self.active_control = None;
            self.render();
            return;
        }
        self.update_touch(x, y);
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        self.update_touch(x, y);
    }

    pub fn touch_up(&mut self) {
        self.active_control = None;
    }

    fn update_touch(&mut self, x: f32, y: f32) {
        let Some(active_control) = self.active_control else {
            return;
        };

        match active_control {
            ActiveControl::RightSlider(slider_index) => {
                let value = ((x - SLIDER_TRACK_LEFT as f32)
                    / (SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32)
                    .clamp(0.0, 1.0);
                self.params[slider_index] = value;
            }
            ActiveControl::Tilt => {
                self.z_mix =
                    (1.0 - (y - TILT_TOP as f32) / (TILT_BOTTOM - TILT_TOP) as f32).clamp(0.0, 1.0);
            }
            ActiveControl::Zoom => {
                self.zoom =
                    (1.0 - (y - ZOOM_TOP as f32) / (ZOOM_BOTTOM - ZOOM_TOP) as f32).clamp(0.0, 1.0);
            }
            ActiveControl::XyView => {
                self.xy_mix = ((x - VIEW_SLIDER_LEFT as f32)
                    / (VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT) as f32)
                    .clamp(0.0, 1.0);
            }
            ActiveControl::PreviousTarget => {}
            ActiveControl::NextTarget => {}
        }
        self.render();
    }

    fn render(&mut self) {
        self.buffer.clear(Rgb565::BLACK);
        self.draw_grid();
        self.draw_target();
        self.draw_sliders();
        self.draw_arm();
        self.draw_report();
    }

    fn draw_grid(&mut self) {
        let style =
            PrimitiveStyle::with_stroke(Rgb565::CSS_DARK_SLATE_GRAY, zoomed_pixels(2, self.zoom));
        for grid in -4..=4 {
            let grid = grid as f32;
            Line::new(
                self.world_to_screen(grid, -4.0, 0.0),
                self.world_to_screen(grid, 4.0, 0.0),
            )
            .into_styled(style)
            .draw(&mut self.buffer)
            .ok();
            Line::new(
                self.world_to_screen(-4.0, grid, 0.0),
                self.world_to_screen(4.0, grid, 0.0),
            )
            .into_styled(style)
            .draw(&mut self.buffer)
            .ok();
        }
    }

    fn draw_arm(&mut self) {
        let rod_width = zoomed_pixels(3, self.zoom);
        let joint_diameter = zoomed_pixels(7, self.zoom);
        let mut previous: Option<Point> = None;
        for pose in LINKAGE.poses(&self.params) {
            let point = self.pose_to_screen(pose);
            if let Some(previous_point) = previous {
                Line::new(previous_point, point)
                    .into_styled(PrimitiveStyle::with_stroke(
                        Rgb565::CSS_DARK_CYAN,
                        rod_width,
                    ))
                    .draw(&mut self.buffer)
                    .ok();
            }
            Circle::with_center(point, joint_diameter)
                .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_DARK_CYAN))
                .draw(&mut self.buffer)
                .ok();
            previous = Some(point);
        }
    }

    fn draw_target(&mut self) {
        let target = target_from_seed(self.target_seed);
        let Vec3([x, y, z]) = target.center;
        let diameter = world_diameter_to_screen(target.diameter, self.zoom);

        Circle::with_center(self.world_to_screen(x, y, z), diameter)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
            .draw(&mut self.buffer)
            .ok();
    }

    fn draw_sliders(&mut self) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        Text::with_baseline("z", Point::new(11, 5), text_style, Baseline::Top)
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
            TILT_TOP + round_to_i32((TILT_BOTTOM - TILT_TOP) as f32 * (1.0 - self.z_mix));
        Circle::with_center(Point::new(TILT_X, tilt_knob_y), 9)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW))
            .draw(&mut self.buffer)
            .ok();

        Text::with_baseline("zoom", Point::new(29, 5), text_style, Baseline::Top)
            .draw(&mut self.buffer)
            .ok();
        Line::new(
            Point::new(ZOOM_X, ZOOM_TOP),
            Point::new(ZOOM_X, ZOOM_BOTTOM),
        )
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 2))
        .draw(&mut self.buffer)
        .ok();
        let zoom_knob_y =
            ZOOM_TOP + round_to_i32((ZOOM_BOTTOM - ZOOM_TOP) as f32 * (1.0 - self.zoom));
        Circle::with_center(Point::new(ZOOM_X, zoom_knob_y), 9)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW))
            .draw(&mut self.buffer)
            .ok();

        Rectangle::new(
            Point::new(PREV_BUTTON_LEFT, TARGET_CONTROL_TOP),
            Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
        )
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 1))
        .draw(&mut self.buffer)
        .ok();
        Text::with_baseline(
            "prev",
            Point::new(
                PREV_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
                TARGET_CONTROL_TOP + 2,
            ),
            text_style,
            Baseline::Top,
        )
        .draw(&mut self.buffer)
        .ok();
        Text::with_baseline(
            "target",
            Point::new(TARGET_LABEL_LEFT, TARGET_CONTROL_TOP + 2),
            text_style,
            Baseline::Top,
        )
        .draw(&mut self.buffer)
        .ok();
        Rectangle::new(
            Point::new(NEXT_BUTTON_LEFT, TARGET_CONTROL_TOP),
            Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
        )
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 1))
        .draw(&mut self.buffer)
        .ok();
        Text::with_baseline(
            "next",
            Point::new(
                NEXT_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
                TARGET_CONTROL_TOP + 2,
            ),
            text_style,
            Baseline::Top,
        )
        .draw(&mut self.buffer)
        .ok();

        for slider_index in 0..SLIDER_COUNT {
            let y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
            let value = self.params[slider_index];

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

            let knob_x =
                SLIDER_TRACK_LEFT + round_to_i32((SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32 * value);
            Circle::with_center(Point::new(knob_x, y + 8), 9)
                .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW))
                .draw(&mut self.buffer)
                .ok();
        }

        Text::with_baseline(
            "x/y view",
            Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y - 15),
            text_style,
            Baseline::Top,
        )
        .draw(&mut self.buffer)
        .ok();
        Line::new(
            Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y),
            Point::new(VIEW_SLIDER_RIGHT, VIEW_SLIDER_Y),
        )
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 2))
        .draw(&mut self.buffer)
        .ok();
        let view_knob_x = VIEW_SLIDER_LEFT
            + round_to_i32((VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT) as f32 * self.xy_mix);
        Circle::with_center(Point::new(view_knob_x, VIEW_SLIDER_Y), 9)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW))
            .draw(&mut self.buffer)
            .ok();
    }

    fn draw_report(&mut self) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        let mut report = DistanceReport::new();
        Text::with_baseline(
            report.as_str(self.target_distance()),
            Point::new(DISTANCE_REPORT_LEFT, 5),
            text_style,
            Baseline::Top,
        )
        .draw(&mut self.buffer)
        .ok();
    }

    fn pose_to_screen(&self, pose: Pose) -> Point {
        let Vec3([x, y, z]) = pose.position();
        self.world_to_screen(x, y, -z)
    }

    fn world_to_screen(&self, x: f32, y: f32, z: f32) -> Point {
        let projection = project(x, y, z, self.xy_mix, self.z_mix);
        Point::new(
            horizontal_to_screen(projection.horizontal, self.zoom),
            vertical_to_screen(projection.vertical, self.z_mix, self.zoom),
        )
    }

    fn target_distance(&self) -> f32 {
        let hand = hand_measurement(&self.params);
        let target = target_from_seed(self.target_seed);
        let Vec3([hand_x, hand_y, hand_z]) = hand.center;
        let Vec3([target_x, target_y, target_z]) = target.center;
        let size_delta = hand.width - target.diameter;

        libm::sqrtf(
            square(hand_x - target_x)
                + square(hand_y - target_y)
                + square(hand_z - target_z)
                + square(size_delta),
        )
    }
}

impl Default for CydSim {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum ActiveControl {
    RightSlider(usize),
    Tilt,
    Zoom,
    XyView,
    PreviousTarget,
    NextTarget,
}

#[derive(Clone, Copy)]
struct Projection {
    horizontal: f32,
    vertical: f32,
}

fn project(x: f32, y: f32, z: f32, xy_mix: f32, z_mix: f32) -> Projection {
    let angle = (xy_mix - 0.5) * TAU;
    let cos = libm::cosf(angle);
    let sin = libm::sinf(angle);
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
    if (x - ZOOM_X as f32).abs() <= 14.0 && (ZOOM_TOP as f32..=ZOOM_BOTTOM as f32).contains(&y) {
        return Some(ActiveControl::Zoom);
    }
    if (PREV_BUTTON_LEFT as f32..=(PREV_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32) as f32)
        .contains(&x)
        && (TARGET_CONTROL_TOP as f32..=(TARGET_CONTROL_TOP + TARGET_BUTTON_HEIGHT as i32) as f32)
            .contains(&y)
    {
        return Some(ActiveControl::PreviousTarget);
    }
    if (NEXT_BUTTON_LEFT as f32..=(NEXT_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32) as f32)
        .contains(&x)
        && (TARGET_CONTROL_TOP as f32..=(TARGET_CONTROL_TOP + TARGET_BUTTON_HEIGHT as i32) as f32)
            .contains(&y)
    {
        return Some(ActiveControl::NextTarget);
    }
    if (VIEW_SLIDER_Y as f32 - y).abs() <= 14.0
        && (VIEW_SLIDER_LEFT as f32..=VIEW_SLIDER_RIGHT as f32).contains(&x)
    {
        return Some(ActiveControl::XyView);
    }
    for slider_index in 0..SLIDER_COUNT {
        let slider_y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
        if x >= SLIDER_LEFT as f32 && (y - (slider_y + 8) as f32).abs() <= 13.0 {
            return Some(ActiveControl::RightSlider(slider_index));
        }
    }
    None
}

fn horizontal_to_screen(horizontal: f32, zoom: f32) -> i32 {
    scale_to_screen(
        horizontal,
        HORIZONTAL_MIN,
        HORIZONTAL_MAX,
        SCREEN_WIDTH,
        zoom,
    )
}

fn vertical_to_screen(vertical: f32, z_mix: f32, zoom: f32) -> i32 {
    let low = Z_MIN * (1.0 - z_mix) + HORIZONTAL_MIN * z_mix;
    let high = Z_MAX * (1.0 - z_mix) + HORIZONTAL_MAX * z_mix;
    (SCREEN_HEIGHT as i32 - 1) - scale_to_screen(vertical, low, high, SCREEN_HEIGHT, zoom)
}

fn scale_to_screen(value: f32, low: f32, high: f32, pixels: usize, zoom: f32) -> i32 {
    let scale = zoom_to_scale(zoom);
    let origin_fraction = ((0.0 - low) / (high - low)).clamp(0.0, 1.0);
    let fraction = (origin_fraction + ((value - 0.0) / (high - low)) * scale).clamp(0.0, 1.0);
    round_to_i32(fraction * (pixels - 1) as f32)
}

fn zoom_to_scale(zoom: f32) -> f32 {
    0.5 + zoom
}

fn zoomed_pixels(base_pixels: u32, zoom: f32) -> u32 {
    round_to_u32(base_pixels as f32 * zoom_to_scale(zoom)).max(1)
}

fn world_diameter_to_screen(diameter: f32, zoom: f32) -> u32 {
    round_to_u32(
        diameter * (SCREEN_WIDTH - 1) as f32 / (HORIZONTAL_MAX - HORIZONTAL_MIN)
            * zoom_to_scale(zoom),
    )
    .max(1)
}

#[derive(Clone, Copy)]
struct HandMeasurement {
    center: Vec3,
    width: f32,
}

#[derive(Clone, Copy)]
struct Target {
    center: Vec3,
    diameter: f32,
}

fn hand_measurement(params: &[f32; 6]) -> HandMeasurement {
    let mut corners = [Vec3::ZERO; 4];
    for (pose_index, pose) in LINKAGE.poses(params).enumerate() {
        for (corner_index, hand_pose_index) in HAND_CORNER_POSE_INDICES.iter().enumerate() {
            if pose_index == *hand_pose_index {
                corners[corner_index] = display_world_position(pose);
            }
        }
    }

    let center = (corners[0] + corners[1] + corners[2] + corners[3]) * 0.25;
    let width = (distance(corners[0], corners[2]) + distance(corners[1], corners[3])) * 0.5;

    HandMeasurement { center, width }
}

fn display_world_position(pose: Pose) -> Vec3 {
    let Vec3([x, y, z]) = pose.position();
    Vec3([x, y, -z])
}

fn target_from_seed(seed: u32) -> Target {
    let mut rng = WyRand::new_seed(u64::from(seed));
    let mut target_params = [0.0; 6];
    for (param_index, param) in target_params.iter_mut().enumerate() {
        *param = if param_index == 2 {
            0.0
        } else {
            random_fraction(&mut rng)
        };
    }

    let diameter = TARGET_MIN_DIAMETER
        + random_fraction(&mut rng) * (TARGET_MAX_DIAMETER - TARGET_MIN_DIAMETER);

    Target {
        center: hand_measurement(&target_params).center,
        diameter,
    }
}

fn random_fraction(rng: &mut WyRand) -> f32 {
    rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0)
}

fn distance(left: Vec3, right: Vec3) -> f32 {
    let Vec3([left_x, left_y, left_z]) = left;
    let Vec3([right_x, right_y, right_z]) = right;
    libm::sqrtf(square(left_x - right_x) + square(left_y - right_y) + square(left_z - right_z))
}

fn square(value: f32) -> f32 {
    value * value
}

fn round_to_i32(value: f32) -> i32 {
    libm::roundf(value) as i32
}

fn round_to_u32(value: f32) -> u32 {
    libm::roundf(value) as u32
}

struct DistanceReport {
    bytes: [u8; 10],
    len: usize,
}

impl DistanceReport {
    fn new() -> Self {
        Self {
            bytes: *b"dist 00.00",
            len: 10,
        }
    }

    fn as_str(&mut self, value: f32) -> &str {
        let hundredths = round_to_u32(value.clamp(0.0, 99.99) * 100.0);
        let whole = hundredths / 100;
        let fraction = hundredths % 100;

        self.bytes[5] = b'0' + (whole / 10) as u8;
        self.bytes[6] = b'0' + (whole % 10) as u8;
        self.bytes[8] = b'0' + (fraction / 10) as u8;
        self.bytes[9] = b'0' + (fraction % 10) as u8;

        core::str::from_utf8(&self.bytes[..self.len]).expect("distance report is ASCII")
    }
}

pub struct FrameBuffer {
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

    #[must_use]
    pub fn pixels(&self) -> &[Rgb565; SCREEN_PIXELS] {
        &self.pixels
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
