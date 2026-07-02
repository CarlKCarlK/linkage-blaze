//! Generic helpers for the armatron example.
//!
//! The device-agnostic game loop lives here.
//!
//! The generic loop updates the armatron state, dispatches touch events, renders
//! changed frames, and flushes them through the [`Cyd`](linkage_blaze_cyd_core::Cyd)
//! frame boundary.

pub mod calibration;
mod controlled;
mod controls;
pub mod reverse_kinematics;

use core::{convert::Infallible, fmt::Write};

use embassy_time::Instant;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{IntoStorage, Rgb565, WebColors},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle},
    text::{Baseline, Text},
};
use linkage_blaze_core::{
    DrawSurface, LinkageFixed, LinkageView, Projection, Rgb888, Vec3, linkage, linkage_fixed,
    render_draw_items_3d, rgb565_from_rgb888_components,
};
use linkage_blaze_cyd_core::{Cyd, CydDisplay, CydFrame, CydTouch};
use nanorand::{Rng, WyRand};
use static_cell::StaticCell;

use controls::ArmatronControls;

use crate::infallible::InfallibleResultExt;

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BACKGROUND: Rgb888 = Rgb888::new(0, 0, 0); // black
pub const BLACK: Rgb888 = BACKGROUND;
pub const WHITE: Rgb888 = Rgb888::new(255, 255, 255); // white
pub const YELLOW: Rgb888 = Rgb888::new(255, 255, 0); // yellow
const BACKGROUND_565: Rgb565 = rgb565_from_rgb888_components(0, 0, 0); // black

// ── Armatron state constants ─────────────────────────────────────────────────

// todo00 I hate all these constants.
pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 240;
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

const TEXT_CHAR_WIDTH: i32 = 6;
const DISTANCE_REPORT_WIDTH: i32 = 14 * TEXT_CHAR_WIDTH;
const DISTANCE_REPORT_LEFT: i32 = ((SCREEN_WIDTH as i32 - DISTANCE_REPORT_WIDTH) / 2) - 16;
const FPS_TEXT_BUFFER_LEN: usize = 8;
const FPS_REPORT_WIDTH: i32 = FPS_TEXT_BUFFER_LEN as i32 * TEXT_CHAR_WIDTH;
const FPS_REPORT_LEFT: i32 = SCREEN_WIDTH as i32 - FPS_REPORT_WIDTH;
const FPS_REPORT_TOP: i32 = SCREEN_HEIGHT as i32 - 11;
const FPS_TEXT_POINT: Point = Point::new(FPS_REPORT_LEFT, FPS_REPORT_TOP);
const FPS_TEXT_STYLE: MonoTextStyle<'static, Rgb565> =
    MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_LIGHT_SLATE_GRAY);
const FPS_TEXT_MAX_TENTHS: u32 = 990;
const VERSION_TEXT: &str = concat!("v", env!("CARGO_PKG_VERSION"));
const VERSION_REPORT_LEFT: i32 =
    FPS_REPORT_LEFT - (VERSION_TEXT.len() as i32 * TEXT_CHAR_WIDTH) - TEXT_CHAR_WIDTH;
const VERSION_REPORT_TOP: i32 = FPS_REPORT_TOP;

// ---- world / display constants ----
const PIXELS_PER_UNIT: f32 = SCREEN_WIDTH as f32 / 16.0; // 16 world units span the screen width

// ---- parameter indices ----
const TARGET_PARAM_START: usize = 9;

// ---- colors ----
const SIM_WHITE: Rgb888 = Rgb888::CSS_WHITE;
const CYAN: Rgb888 = Rgb888::CSS_CYAN;
const SIM_YELLOW: Rgb888 = Rgb888::CSS_YELLOW;
const GREEN: Rgb888 = Rgb888::CSS_LIME;
const LIGHT_SLATE_GRAY: Rgb888 = Rgb888::CSS_LIGHT_SLATE_GRAY;

// ---- linkages ----
//
// Section 1: floor disk + axis lines (commented out).
// Section 2: arm.  Pen down for strokes.
// Section 3: target traversal (pen up) then target disk (commented out).
// todo0000000 can we use functions to avoid double allocation?
const CAMERA_CONTROL: LinkageFixed<3, 1, 8> =
    linkage_fixed!("../../../linkage-blaze-armatron-core/src/camera_control.lb.rs");
const GRID_9X9: LinkageFixed<0, 1, 81> =
    linkage_fixed!("../../../linkage-blaze-armatron-core/src/grid_9x9.lb.rs");
const CAMERA_AND_GRID: LinkageFixed<3, 2, 88> = CAMERA_CONTROL.combine(GRID_9X9);
const ARMATRON1: LinkageFixed<6, 1, 25> =
    linkage_fixed!("../../../linkage-blaze-armatron-core/src/armatron1.lb.rs");
const ARMATRON1_WITH_JOINTS: LinkageFixed<6, 1, 45> = ARMATRON1.with_joint_spheres(0.15);
const LINKAGE0: LinkageFixed<9, 3, 133> = CAMERA_AND_GRID.combine(ARMATRON1_WITH_JOINTS);
const LINKAGE_FIXED: LinkageFixed<15, 4, 159> = LINKAGE0
    .restore("scene origin")
    .combine(ARMATRON1) // Add ghost arm to hold target.
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0);
const LINKAGE: LinkageView<15, 4> = LINKAGE_FIXED.view();
const ARM_TIP_LINKAGE_FIXED: LinkageFixed<9, 2, 32> = CAMERA_CONTROL.combine(ARMATRON1);
const ARM_TIP_LINKAGE: LinkageView<9, 2> = ARM_TIP_LINKAGE_FIXED.view();

pub const DOF: usize = LINKAGE.dof();

const SHOW_FPS_TEXT: bool = true;

// ── Generic armatron loop ─────────────────────────────────────────────────────

/// Run the armatron example forever.
///
/// Each iteration:
/// 1. Reads the next touch event from [`CydTouch::read_touch_event`].
/// 2. Updates local armatron params, touch, and fps state.
/// 3. If the frame changed, renders and presents a full-screen CYD frame.
///
/// Calibration is intentionally outside this game loop. Platform setup must
/// provide calibrated touch before calling [`armatron`]. The temporary
/// [`calibration`] module exists only so current platform examples can share
/// calibration UI helpers until that responsibility moves into the CYD device
/// layer.
pub async fn armatron<C>(cyd: &mut C) -> Result<Infallible, Error<C::Error>>
where
    C: Cyd,
{
    let (mut display, mut touch) = cyd.parts();

    // Set the initial params including a random target.
    let mut params = LINKAGE.param_defaults();
    let mut target_seed = 0;
    let mut rng = WyRand::new_seed(u64::from(target_seed));
    // todo00 how to we feel about "TARGET_PARAM_START"
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
    }

    // Set up state.
    let mut controls = ArmatronControls::new(&params);
    let mut previous_tick = None;

    // Set up buffers
    let mut frame = display.full_frame_mut();
    let mut fps_text = heapless::String::<FPS_TEXT_BUFFER_LEN>::new();

    loop {
        // todo000 review CydFrame::clear; its name collision with DrawTarget::clear(color) makes
        // generic frame code use fill(...) instead, which makes the clear helper much less useful.
        frame.fill(BACKGROUND_565);

        // Display FPS if requested and available.
        let current_tick = Instant::now();
        if SHOW_FPS_TEXT
            && let Some(previous_tick) = previous_tick
            && let Some((fps_whole, fps_fraction)) = display_fps_since(previous_tick, current_tick)
        {
            fps_text.clear();
            write!(&mut fps_text, "{fps_whole:>2}.{fps_fraction} fps")?;
            Text::with_baseline(&fps_text, FPS_TEXT_POINT, FPS_TEXT_STYLE, Baseline::Top)
                .draw(&mut frame)
                .unwrap_infallible();
        }
        previous_tick = Some(current_tick);

        // todo It's weird this doesn't return an error of the right type already and needs to be converted
        controls.handle_touch_event(touch.read_touch_event().map_err(Error::Cyd)?);
        controls.write_params(&mut params);

        if controls.previous_target_clicked() {
            target_seed = target_seed.wrapping_sub(1);
            randomize_target_params(target_seed, &mut params);
        }
        if controls.next_target_clicked() {
            target_seed = target_seed.wrapping_add(1);
            randomize_target_params(target_seed, &mut params);
        }

        {
            let mut surface = ArmatronSurface {
                buffer: &mut frame,
                result: Ok(()),
            };
            render_draw_items_3d(&projection(), &mut surface, LINKAGE.draw_items_3d(&params));
            surface.result.unwrap_infallible();
        }

        {
            let mut target_label = TargetLabel::new();
            controls
                .draw(&mut frame, target_label.as_str(target_seed))
                .unwrap_infallible();
        }

        {
            let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::from(SIM_WHITE));
            let mut report = DistanceReport::new();
            Text::with_baseline(
                report.as_str(target_distance(&params)),
                Point::new(DISTANCE_REPORT_LEFT, 5),
                text_style,
                Baseline::Top,
            )
            .draw(&mut frame)
            .unwrap_infallible();
        }

        {
            let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::from(LIGHT_SLATE_GRAY));
            Text::with_baseline(
                VERSION_TEXT,
                Point::new(VERSION_REPORT_LEFT, VERSION_REPORT_TOP),
                text_style,
                Baseline::Top,
            )
            .draw(&mut frame)
            .unwrap_infallible();
        }

        controls.draw_touch_cursor(&mut frame).unwrap_infallible();

        frame.flush().await.map_err(Error::Cyd)?;
    }
}

/// Error from the generic armatron loop, generic over the CYD device error `F`.
///
/// Local errors such as [`core::fmt::Error`] get a derived `From`, so they
/// propagate with a plain `?`. The CYD device error `F` is converted explicitly
/// with `.map_err(Error::Cyd)` at the call site: a blanket `From<F>` would be
/// greedy enough to collide with those concrete `From`s under coherence.
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// Formatting the FPS report failed.
    FpsReport(core::fmt::Error),
    /// Reading touch events or flushing a frame failed.
    #[from(ignore)]
    Cyd(F),
}

struct ArmatronSurface<'a, T: DrawTarget<Color = Rgb565>> {
    buffer: &'a mut T,
    /// First error produced by any draw, or `Ok(())` if every draw succeeded.
    /// Once an error is recorded, later draws are skipped so the first failure wins.
    result: Result<(), T::Error>,
}

impl<T: DrawTarget<Color = Rgb565>> DrawSurface for ArmatronSurface<'_, T> {
    fn stroke(&mut self, start: (f32, f32), end: (f32, f32), color: Rgb888, pixel_width: f32) {
        if self.result.is_err() {
            return;
        }
        let start = Point::new(start.0 as i32, start.1 as i32);
        let end = Point::new(end.0 as i32, end.1 as i32);
        let width = round_to_u32(pixel_width).max(1);
        let color = Rgb565::from(color);
        self.result = Line::new(start, end)
            .into_styled(PrimitiveStyle::with_stroke(color, width))
            .draw(self.buffer);
    }

    fn filled_ellipse(
        &mut self,
        center: (f32, f32),
        axis_a: (f32, f32),
        axis_b: (f32, f32),
        color: Rgb888,
    ) {
        if self.result.is_err() {
            return;
        }
        let cx = center.0 as i32;
        let cy = center.1 as i32;
        let (ax, ay) = axis_a;
        let (bx, by) = axis_b;
        let det = ax * by - bx * ay;
        let det_sq = det * det;
        if det_sq < 0.25 {
            return;
        }
        let hw = libm::sqrtf(ax * ax + bx * bx) as i32 + 1;
        let hh = libm::sqrtf(ay * ay + by * by) as i32 + 1;
        let x0 = (cx - hw).max(0);
        let y0 = (cy - hh).max(0);
        let x1 = (cx + hw).min(SCREEN_WIDTH as i32 - 1);
        let y1 = (cy + hh).min(SCREEN_HEIGHT as i32 - 1);
        let color = Rgb565::from(color);
        self.result = self.buffer.draw_iter((y0..=y1).flat_map(move |y| {
            (x0..=x1).filter_map(move |x| {
                let dx = x as f32 - cx as f32;
                let dy = y as f32 - cy as f32;
                let u = by * dx - bx * dy;
                let v = ax * dy - ay * dx;
                if u * u + v * v <= det_sq {
                    Some(Pixel(Point::new(x, y), color))
                } else {
                    None
                }
            })
        }));
    }

    fn filled_circle(&mut self, center: (f32, f32), pixel_radius: f32, color: Rgb888) {
        if self.result.is_err() {
            return;
        }
        if pixel_radius <= 0.0 {
            return;
        }
        let diameter = round_to_u32(pixel_radius * 2.0);
        if diameter == 0 {
            return;
        }
        self.result = Circle::with_center(Point::new(center.0 as i32, center.1 as i32), diameter)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::from(color)))
            .draw(self.buffer);
    }
}

//todo0000 revisit Robot Ortho projection (+Z up, +Y left, drops X): reconsider after camera_control is updated
fn projection() -> Projection {
    Projection::front_perspective(
        Point::new(SCREEN_WIDTH as i32 / 2, SCREEN_HEIGHT as i32 / 2),
        PIXELS_PER_UNIT,
        30.0,
    )
}

// ── FrameBuffer ────────────────────────────────────────────────────────────────

pub struct FrameBuffer {
    pixels: [u16; SCREEN_PIXELS],
}

impl FrameBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: [0; SCREEN_PIXELS],
        }
    }

    pub fn static_new() -> &'static mut Self {
        static FRAME_BUFFER: StaticCell<FrameBuffer> = StaticCell::new();
        FRAME_BUFFER.init_with(FrameBuffer::new)
    }

    pub fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color.into_storage());
    }

    pub fn raw_pixels_mut(&mut self) -> &mut [u16; SCREEN_PIXELS] {
        &mut self.pixels
    }

    #[must_use]
    pub fn raw_pixels(&self) -> &[u16; SCREEN_PIXELS] {
        &self.pixels
    }
}

impl DrawTarget for FrameBuffer {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }

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
            self.pixels[y * SCREEN_WIDTH + x] = color.into_storage();
        }
        Ok(())
    }
}

impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Private helper functions ───────────────────────────────────────────────────

fn randomize_target_params(target_seed: u8, params: &mut [f32; DOF]) {
    let mut rng = WyRand::new_seed(u64::from(target_seed));
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
    }
}

fn display_fps_since(previous_tick: Instant, current_tick: Instant) -> Option<(u32, u32)> {
    let elapsed_micros = current_tick
        .saturating_duration_since(previous_tick)
        .as_micros();

    (elapsed_micros != 0).then(|| {
        // Convert microseconds/frame to tenths of frames/second, rounded.
        let fps_tenths = 10_000_000_u64.saturating_add(elapsed_micros / 2) / elapsed_micros;
        let fps_tenths = fps_tenths.min(u64::from(FPS_TEXT_MAX_TENTHS)) as u32;
        (fps_tenths / 10, fps_tenths % 10)
    })
}

fn arm_tip(rk_linkage: LinkageView<'_, 9, 2>, params: &[f32; DOF]) -> Vec3 {
    let mut arm_params = [0.0f32; 9];
    arm_params.copy_from_slice(&params[..9]);
    rk_linkage.final_pose(&arm_params).position()
}

fn target_center(linkage: LinkageView<'_, 15, 4>, params: &[f32; DOF]) -> Vec3 {
    linkage.final_pose(params).position()
}

fn compute_target_distance(
    rk_linkage: LinkageView<'_, 9, 2>,
    linkage: LinkageView<'_, 15, 4>,
    params: &[f32; DOF],
) -> f32 {
    distance(arm_tip(rk_linkage, params), target_center(linkage, params))
}

fn target_distance(params: &[f32; DOF]) -> f32 {
    compute_target_distance(ARM_TIP_LINKAGE, LINKAGE, params)
}

fn fill_style(color: Rgb888) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_fill(Rgb565::from(color))
}

fn distance(left: Vec3, right: Vec3) -> f32 {
    let Vec3([left_x, left_y, left_z]) = left;
    let Vec3([right_x, right_y, right_z]) = right;
    libm::sqrtf(square(left_x - right_x) + square(left_y - right_y) + square(left_z - right_z))
}

fn square(value: f32) -> f32 {
    value * value
}

fn round_to_u32(value: f32) -> u32 {
    libm::roundf(value) as u32
}

struct TargetLabel {
    bytes: [u8; 11],
    len: usize,
}

impl TargetLabel {
    fn new() -> Self {
        Self {
            bytes: *b"target #000",
            len: 11,
        }
    }

    fn as_str(&mut self, value: u8) -> &str {
        let hundreds = value / 100;
        let tens = (value / 10) % 10;
        let ones = value % 10;

        if hundreds > 0 {
            self.bytes[8] = b'0' + hundreds;
            self.bytes[9] = b'0' + tens;
            self.bytes[10] = b'0' + ones;
            self.len = 11;
        } else if tens > 0 {
            self.bytes[8] = b'0' + tens;
            self.bytes[9] = b'0' + ones;
            self.len = 10;
        } else {
            self.bytes[8] = b'0' + ones;
            self.len = 9;
        }

        core::str::from_utf8(&self.bytes[..self.len]).expect("target label is ASCII")
    }
}

struct DistanceReport {
    bytes: [u8; 14],
    len: usize,
}

impl DistanceReport {
    fn new() -> Self {
        Self {
            bytes: *b"distance 00.00",
            len: 14,
        }
    }

    fn as_str(&mut self, value: f32) -> &str {
        let hundredths = round_to_u32(value.clamp(0.0, 99.99) * 100.0);
        let whole = hundredths / 100;
        let fraction = hundredths % 100;

        self.bytes[9] = b'0' + (whole / 10) as u8;
        self.bytes[10] = b'0' + (whole % 10) as u8;
        self.bytes[12] = b'0' + (fraction / 10) as u8;
        self.bytes[13] = b'0' + (fraction % 10) as u8;

        core::str::from_utf8(&self.bytes[..self.len]).expect("distance report is ASCII")
    }
}
