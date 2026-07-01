//! Generic helpers for the armatron example.
//!
//! The device-agnostic game loop lives here.
//!
//! The generic loop updates the armatron state, dispatches touch input, renders
//! changed frames, and flushes them through the [`Cyd`](linkage_blaze_cyd_core::Cyd)
//! frame boundary.

pub mod calibration;
mod controlled;
pub mod reverse_kinematics;

use core::convert::Infallible;

use embassy_time::Instant;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{IntoStorage, Rgb565, WebColors},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    text::{Baseline, Text},
};
use linkage_blaze_core::{
    DrawSurface, LinkageFixed, LinkageView, Projection, Rgb888, Vec3, linkage, linkage_fixed,
    render_draw_items_3d,
};
use linkage_blaze_cyd_core::{Cyd, CydFrame, TouchInputEvent};
use nanorand::{Rng, WyRand};
use static_cell::StaticCell;

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BLACK: Rgb888 = Rgb888::new(0, 0, 0); // black
pub const WHITE: Rgb888 = Rgb888::new(255, 255, 255); // white
pub const YELLOW: Rgb888 = Rgb888::new(255, 255, 0); // yellow

// ── Armatron state constants ─────────────────────────────────────────────────

// todo00 I hate all these constants.
pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 240;
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

// ---- layout constants ----
const TILT_X: i32 = 16;
const DOLLY_X: i32 = 42;
const TILT_TOP: i32 = 24;
const TILT_BOTTOM: i32 = 224;
const DOLLY_TOP: i32 = 24;
const DOLLY_BOTTOM: i32 = 74;
const RK_CONTROL_TOP: i32 = 86;
const RK_RUN_LEFT: i32 = 27;
const RK_STEP_LEFT: i32 = 55;
const RK_BUTTON_SIZE: i32 = 18;
const SLIDER_LEFT: i32 = 230;
const SLIDER_RIGHT: i32 = 312;
const SLIDER_TRACK_LEFT: i32 = 230;
const SLIDER_TOP: i32 = 24;
const SLIDER_STEP: i32 = 32;
const VIEW_SLIDER_LEFT: i32 = 40;
const VIEW_SLIDER_RIGHT: i32 = 252;
const VIEW_SLIDER_Y: i32 = 226;
const CALIBRATE_BUTTON_LEFT: i32 = 288;
const CALIBRATE_BUTTON_TOP: i32 = 212;
const CALIBRATE_BUTTON_WIDTH: u32 = 30;
const CALIBRATE_BUTTON_HEIGHT: u32 = 14;
const TEXT_CHAR_WIDTH: i32 = 6;
const DISTANCE_REPORT_WIDTH: i32 = 14 * TEXT_CHAR_WIDTH;
const DISTANCE_REPORT_LEFT: i32 = ((SCREEN_WIDTH as i32 - DISTANCE_REPORT_WIDTH) / 2) - 16;
const FPS_REPORT_WIDTH: i32 = 7 * TEXT_CHAR_WIDTH;
const FPS_REPORT_LEFT: i32 = SCREEN_WIDTH as i32 - FPS_REPORT_WIDTH;
const FPS_REPORT_TOP: i32 = SCREEN_HEIGHT as i32 - 11;
const VERSION_TEXT: &str = concat!("v", env!("CARGO_PKG_VERSION"));
const VERSION_REPORT_LEFT: i32 =
    FPS_REPORT_LEFT - (VERSION_TEXT.len() as i32 * TEXT_CHAR_WIDTH) - TEXT_CHAR_WIDTH;
const VERSION_REPORT_TOP: i32 = FPS_REPORT_TOP;
const TARGET_CONTROL_TOP: i32 = 17;
const TARGET_BUTTON_WIDTH: u32 = 42;
const TARGET_BUTTON_HEIGHT: u32 = 14;
const TARGET_BUTTON_LABEL_WIDTH: i32 = 4 * TEXT_CHAR_WIDTH;
const TARGET_LABEL_WIDTH: i32 = 11 * TEXT_CHAR_WIDTH;
const TARGET_CONTROL_GAP: i32 = 4;
const TARGET_CONTROL_WIDTH: i32 =
    TARGET_BUTTON_WIDTH as i32 * 2 + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP * 2;
const PREV_BUTTON_LEFT: i32 = ((SCREEN_WIDTH as i32 - TARGET_CONTROL_WIDTH) / 2) - 16;
const TARGET_LABEL_LEFT: i32 = PREV_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32 + TARGET_CONTROL_GAP;
const NEXT_BUTTON_LEFT: i32 = TARGET_LABEL_LEFT + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP;

// ---- world / display constants ----
const PIXELS_PER_UNIT: f32 = SCREEN_WIDTH as f32 / 16.0; // 16 world units span the screen width

// ---- parameter indices ----
const ARM_PARAM_START: usize = 3;
const ARM_PARAM_COUNT: usize = 6;
const TARGET_PARAM_START: usize = 9;

// ---- colors ----
const SIM_BLACK: Rgb888 = Rgb888::CSS_BLACK;
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

const BASE_YAW_PARAM: usize = 0;
const BASE_PITCH_PARAM: usize = 1;
const DOLLY_PARAM: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TouchInputOutcome {
    Unchanged,
    Changed,
}

// ── Generic armatron loop ─────────────────────────────────────────────────────

/// Run the armatron example forever.
///
/// Each iteration:
/// 1. Reads the next touch event from [`Cyd::read_touch_input`].
/// 2. Updates local armatron params, touch, and fps state.
/// 3. If the frame changed, renders and presents a full-screen CYD frame.
///
/// Calibration is intentionally outside this game loop. Platform setup must
/// provide calibrated touch before calling [`armatron`]. The temporary
/// [`calibration`] module exists only so current platform examples can share
/// calibration UI helpers until that responsibility moves into the CYD device
/// layer.
pub async fn armatron<C>(cyd: &mut C) -> Result<Infallible, C::Error>
where
    C: Cyd,
{
    // Set the initial params including a random target.
    let mut params = LINKAGE.param_defaults();
    let mut target_seed = 0;
    let mut rng = WyRand::new_seed(u64::from(target_seed));
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
    }

    let mut active_control = None;
    let mut previous_tick = None;
    let show_fps = false;
    let mut fps = None;
    let mut touch_cursor = None;

    loop {
        let touch = cyd.read_touch_input()?;
        let now = Instant::now();
        let previous_tick_before_frame = previous_tick;
        let first_tick = previous_tick_before_frame.is_none();
        previous_tick = Some(now);
        let fps_draw_requested = update_fps(show_fps, previous_tick_before_frame, now, &mut fps);
        let touch_input_outcome = touch.map_or(TouchInputOutcome::Unchanged, |touch_input_event| {
            handle_touch_input_event(
                touch_input_event,
                &mut params,
                &mut target_seed,
                &mut active_control,
                &mut touch_cursor,
            )
        });

        let draw_requested = matches!(touch_input_outcome, TouchInputOutcome::Changed)
            || first_tick
            || fps_draw_requested;
        if draw_requested {
            let mut frame = cyd.full_frame_mut();
            match draw_armatron(
                &mut frame,
                &params,
                target_seed,
                show_fps,
                fps,
                touch_cursor,
            ) {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
            frame.flush().await?;
        }
    }
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
        let color = rgb565_from_rgb888(color);
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
        let color = rgb565_from_rgb888(color);
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
            .into_styled(PrimitiveStyle::with_fill(rgb565_from_rgb888(color)))
            .draw(self.buffer);
    }
}

pub fn draw_armatron<D: DrawTarget<Color = Rgb565>>(
    target: &mut D,
    params: &[f32; DOF],
    target_seed: u8,
    show_fps: bool,
    fps: Option<u32>,
    touch_cursor: Option<(f32, f32)>,
) -> Result<(), D::Error> {
    target.clear(rgb565_from_rgb888(SIM_BLACK))?;
    draw_linkage(LINKAGE, target, params)?;
    draw_sliders(target, params, target_seed)?;
    draw_report(target, params)?;
    draw_version(target)?;
    draw_fps(target, show_fps, fps)?;
    draw_touch_cursor(target, touch_cursor)?;
    Ok(())
}

fn draw_linkage<D: DrawTarget<Color = Rgb565>>(
    linkage: LinkageView<'_, 15, 4>,
    buffer: &mut D,
    params: &[f32; DOF],
) -> Result<(), D::Error> {
    let mut surface = ArmatronSurface {
        buffer,
        result: Ok(()),
    };
    render_draw_items_3d(&projection(), &mut surface, linkage.draw_items_3d(params));
    surface.result
}

fn draw_sliders<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
    params: &[f32; DOF],
    target_seed: u8,
) -> Result<(), D::Error> {
    let text_style = MonoTextStyle::new(&FONT_6X10, rgb565_from_rgb888(SIM_WHITE));
    let mut target_label = TargetLabel::new();

    Text::with_baseline("z", Point::new(11, 5), text_style, Baseline::Top).draw(buffer)?;
    Line::new(
        Point::new(TILT_X, TILT_TOP),
        Point::new(TILT_X, TILT_BOTTOM),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 2))
    .draw(buffer)?;
    let tilt_knob_y =
        TILT_TOP + round_to_i32((TILT_BOTTOM - TILT_TOP) as f32 * (1.0 - params[BASE_PITCH_PARAM]));
    Circle::with_center(Point::new(TILT_X, tilt_knob_y), 9)
        .into_styled(fill_style(SIM_YELLOW))
        .draw(buffer)?;

    Text::with_baseline("zoom", Point::new(29, 5), text_style, Baseline::Top).draw(buffer)?;
    Line::new(
        Point::new(DOLLY_X, DOLLY_TOP),
        Point::new(DOLLY_X, DOLLY_BOTTOM),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 2))
    .draw(buffer)?;
    let dolly_knob_y =
        DOLLY_TOP + round_to_i32((DOLLY_BOTTOM - DOLLY_TOP) as f32 * params[DOLLY_PARAM]);
    Circle::with_center(Point::new(DOLLY_X, dolly_knob_y), 9)
        .into_styled(fill_style(SIM_YELLOW))
        .draw(buffer)?;

    draw_reverse_kinematics_run_button(buffer)?;
    draw_reverse_kinematics_step_button(buffer)?;
    draw_calibrate_button(buffer)?;

    Rectangle::new(
        Point::new(PREV_BUTTON_LEFT, TARGET_CONTROL_TOP),
        Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 1))
    .draw(buffer)?;
    Text::with_baseline(
        "prev",
        Point::new(
            PREV_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
            TARGET_CONTROL_TOP + 2,
        ),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;
    Text::with_baseline(
        target_label.as_str(target_seed),
        Point::new(TARGET_LABEL_LEFT, TARGET_CONTROL_TOP + 2),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;
    Rectangle::new(
        Point::new(NEXT_BUTTON_LEFT, TARGET_CONTROL_TOP),
        Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 1))
    .draw(buffer)?;
    Text::with_baseline(
        "next",
        Point::new(
            NEXT_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
            TARGET_CONTROL_TOP + 2,
        ),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;

    for slider_offset in 0..ARM_PARAM_COUNT {
        let param_index = ARM_PARAM_START + slider_offset;
        let slider_y = SLIDER_TOP + slider_offset as i32 * SLIDER_STEP;
        let value = params[param_index];

        Text::with_baseline(
            LINKAGE.param(param_index).name(),
            Point::new(SLIDER_LEFT, slider_y - 12),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)?;

        Line::new(
            Point::new(SLIDER_TRACK_LEFT, slider_y + 8),
            Point::new(SLIDER_RIGHT, slider_y + 8),
        )
        .into_styled(stroke_style(LIGHT_SLATE_GRAY, 2))
        .draw(buffer)?;

        let knob_x =
            SLIDER_TRACK_LEFT + round_to_i32((SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32 * value);
        Circle::with_center(Point::new(knob_x, slider_y + 8), 9)
            .into_styled(fill_style(SIM_YELLOW))
            .draw(buffer)?;
    }

    Text::with_baseline(
        "x/y view",
        Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y - 15),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;
    Line::new(
        Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y),
        Point::new(VIEW_SLIDER_RIGHT, VIEW_SLIDER_Y),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 2))
    .draw(buffer)?;
    let view_knob_x = VIEW_SLIDER_LEFT
        + round_to_i32((VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT) as f32 * params[BASE_YAW_PARAM]);
    Circle::with_center(Point::new(view_knob_x, VIEW_SLIDER_Y), 9)
        .into_styled(fill_style(SIM_YELLOW))
        .draw(buffer)?;
    Ok(())
}

fn draw_reverse_kinematics_run_button<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
) -> Result<(), D::Error> {
    Triangle::new(
        Point::new(RK_RUN_LEFT, RK_CONTROL_TOP),
        Point::new(RK_RUN_LEFT, RK_CONTROL_TOP + RK_BUTTON_SIZE),
        Point::new(
            RK_RUN_LEFT + RK_BUTTON_SIZE,
            RK_CONTROL_TOP + RK_BUTTON_SIZE / 2,
        ),
    )
    .into_styled(fill_style(GREEN))
    .draw(buffer)?;
    Ok(())
}

fn draw_reverse_kinematics_step_button<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
) -> Result<(), D::Error> {
    Rectangle::new(
        Point::new(RK_STEP_LEFT, RK_CONTROL_TOP),
        Size::new(RK_BUTTON_SIZE as u32, RK_BUTTON_SIZE as u32),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 1))
    .draw(buffer)?;
    Rectangle::new(
        Point::new(
            RK_STEP_LEFT + RK_BUTTON_SIZE - 5,
            RK_CONTROL_TOP + RK_BUTTON_SIZE / 2 - 5,
        ),
        Size::new(2, 10),
    )
    .into_styled(fill_style(SIM_WHITE))
    .draw(buffer)?;
    Triangle::new(
        Point::new(RK_STEP_LEFT + 3, RK_CONTROL_TOP + 4),
        Point::new(RK_STEP_LEFT + 3, RK_CONTROL_TOP + RK_BUTTON_SIZE - 4),
        Point::new(
            RK_STEP_LEFT + RK_BUTTON_SIZE - 7,
            RK_CONTROL_TOP + RK_BUTTON_SIZE / 2,
        ),
    )
    .into_styled(fill_style(GREEN))
    .draw(buffer)?;
    Ok(())
}

fn draw_calibrate_button<D: DrawTarget<Color = Rgb565>>(buffer: &mut D) -> Result<(), D::Error> {
    let text_style = MonoTextStyle::new(&FONT_6X10, rgb565_from_rgb888(SIM_WHITE));
    Rectangle::new(
        Point::new(CALIBRATE_BUTTON_LEFT, CALIBRATE_BUTTON_TOP),
        Size::new(CALIBRATE_BUTTON_WIDTH, CALIBRATE_BUTTON_HEIGHT),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 1))
    .draw(buffer)?;
    Text::with_baseline(
        "cal",
        Point::new(CALIBRATE_BUTTON_LEFT + 6, CALIBRATE_BUTTON_TOP + 2),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;
    Ok(())
}

fn draw_report<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
    params: &[f32; DOF],
) -> Result<(), D::Error> {
    let text_style = MonoTextStyle::new(&FONT_6X10, rgb565_from_rgb888(SIM_WHITE));
    let mut report = DistanceReport::new();
    Text::with_baseline(
        report.as_str(target_distance(params)),
        Point::new(DISTANCE_REPORT_LEFT, 5),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;
    Ok(())
}

fn draw_fps<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
    show_fps: bool,
    fps: Option<u32>,
) -> Result<(), D::Error> {
    if !show_fps {
        return Ok(());
    }

    let text_style = MonoTextStyle::new(&FONT_6X10, rgb565_from_rgb888(LIGHT_SLATE_GRAY));
    let mut report = FpsReport::new();
    Text::with_baseline(
        report.as_str(fps),
        Point::new(FPS_REPORT_LEFT, FPS_REPORT_TOP),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;
    Ok(())
}

fn draw_version<D: DrawTarget<Color = Rgb565>>(buffer: &mut D) -> Result<(), D::Error> {
    let text_style = MonoTextStyle::new(&FONT_6X10, rgb565_from_rgb888(LIGHT_SLATE_GRAY));
    Text::with_baseline(
        VERSION_TEXT,
        Point::new(VERSION_REPORT_LEFT, VERSION_REPORT_TOP),
        text_style,
        Baseline::Top,
    )
    .draw(buffer)?;
    Ok(())
}

fn draw_touch_cursor<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
    touch_cursor: Option<(f32, f32)>,
) -> Result<(), D::Error> {
    if let Some((x, y)) = touch_cursor {
        let x = x as i32;
        let y = y as i32;
        let radius = 5;
        let cursor_style = PrimitiveStyle::with_fill(rgb565_from_rgb888(CYAN));
        Circle::new(Point::new(x - radius, y - radius), (radius * 2 + 1) as u32)
            .into_styled(cursor_style)
            .draw(buffer)?;
    }
    Ok(())
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

// ── Private helper types ───────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum ActiveControl {
    RightSlider(usize), // absolute param index
    Tilt,
    Dolly,
    XyView,
    PreviousTarget,
    NextTarget,
}

// ── Private helper functions ───────────────────────────────────────────────────

fn handle_touch_input_event(
    touch_input_event: TouchInputEvent,
    params: &mut [f32; DOF],
    target_seed: &mut u8,
    active_control: &mut Option<ActiveControl>,
    touch_cursor: &mut Option<(f32, f32)>,
) -> TouchInputOutcome {
    match touch_input_event {
        TouchInputEvent::Down { x, y } => {
            *touch_cursor = Some((x, y));
            touch_down(x, y, params, target_seed, active_control);
            TouchInputOutcome::Changed
        }
        TouchInputEvent::Move { x, y } => {
            *touch_cursor = Some((x, y));
            update_touch(x, y, params, active_control);
            TouchInputOutcome::Changed
        }
        TouchInputEvent::Up => {
            *touch_cursor = None;
            touch_up(active_control);
            TouchInputOutcome::Changed
        }
    }
}

fn touch_down(
    x: f32,
    y: f32,
    params: &mut [f32; DOF],
    target_seed: &mut u8,
    active_control: &mut Option<ActiveControl>,
) {
    *active_control = control_at(x, y);
    match *active_control {
        Some(ActiveControl::PreviousTarget) => {
            *target_seed = target_seed.wrapping_sub(1);
            let mut rng = WyRand::new_seed(u64::from(*target_seed));
            for param in params[TARGET_PARAM_START..].iter_mut() {
                *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
            }
            *active_control = None;
        }
        Some(ActiveControl::NextTarget) => {
            *target_seed = target_seed.wrapping_add(1);
            let mut rng = WyRand::new_seed(u64::from(*target_seed));
            for param in params[TARGET_PARAM_START..].iter_mut() {
                *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
            }
            *active_control = None;
        }
        _ => {
            update_touch(x, y, params, active_control);
        }
    }
}

fn touch_up(active_control: &mut Option<ActiveControl>) {
    *active_control = None;
}

fn update_touch(x: f32, y: f32, params: &mut [f32; DOF], active_control: &Option<ActiveControl>) {
    let Some(active_control) = *active_control else {
        return;
    };

    match active_control {
        ActiveControl::RightSlider(param_index) => {
            let value = ((x - SLIDER_TRACK_LEFT as f32)
                / (SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32)
                .clamp(0.0, 1.0);
            params[param_index] = value;
        }
        ActiveControl::Tilt => {
            params[BASE_PITCH_PARAM] =
                (1.0 - (y - TILT_TOP as f32) / (TILT_BOTTOM - TILT_TOP) as f32).clamp(0.0, 1.0);
        }
        ActiveControl::Dolly => {
            params[DOLLY_PARAM] =
                ((y - DOLLY_TOP as f32) / (DOLLY_BOTTOM - DOLLY_TOP) as f32).clamp(0.0, 1.0);
        }
        ActiveControl::XyView => {
            params[BASE_YAW_PARAM] = ((x - VIEW_SLIDER_LEFT as f32)
                / (VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT) as f32)
                .clamp(0.0, 1.0);
        }
        ActiveControl::PreviousTarget | ActiveControl::NextTarget => {}
    }
}

fn update_fps(
    show_fps: bool,
    previous_tick: Option<Instant>,
    now: Instant,
    fps: &mut Option<u32>,
) -> bool {
    if !show_fps {
        return false;
    }

    let Some(previous_tick) = previous_tick else {
        return false;
    };
    let frame_micros = now.saturating_duration_since(previous_tick).as_micros();
    if frame_micros == 0 {
        return false;
    }

    *fps = Some((1_000_000 / frame_micros).min(999) as u32);
    true
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

fn control_at(x: f32, y: f32) -> Option<ActiveControl> {
    if (x - TILT_X as f32).abs() <= 14.0 && (TILT_TOP as f32..=TILT_BOTTOM as f32).contains(&y) {
        return Some(ActiveControl::Tilt);
    }
    if (x - DOLLY_X as f32).abs() <= 14.0 && (DOLLY_TOP as f32..=DOLLY_BOTTOM as f32).contains(&y) {
        return Some(ActiveControl::Dolly);
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
    for slider_offset in 0..ARM_PARAM_COUNT {
        let slider_y = SLIDER_TOP + slider_offset as i32 * SLIDER_STEP;
        if x >= SLIDER_LEFT as f32 && (y - (slider_y + 8) as f32).abs() <= 13.0 {
            return Some(ActiveControl::RightSlider(ARM_PARAM_START + slider_offset));
        }
    }
    None
}

// todo000 review rgb565_from_rgb888 later.
fn rgb565_from_rgb888(color: Rgb888) -> Rgb565 {
    Rgb565::from(color)
}

fn fill_style(color: Rgb888) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_fill(rgb565_from_rgb888(color))
}

fn stroke_style(color: Rgb888, stroke_width: u32) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_stroke(rgb565_from_rgb888(color), stroke_width)
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

struct FpsReport {
    bytes: [u8; 7],
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

impl FpsReport {
    fn new() -> Self {
        Self {
            bytes: *b"--- fps",
            len: 7,
        }
    }

    fn as_str(&mut self, fps: Option<u32>) -> &str {
        if let Some(fps) = fps {
            let fps = fps.min(999);
            self.bytes[0] = if fps >= 100 {
                b'0' + (fps / 100) as u8
            } else {
                b' '
            };
            self.bytes[1] = if fps >= 10 {
                b'0' + ((fps / 10) % 10) as u8
            } else {
                b' '
            };
            self.bytes[2] = b'0' + (fps % 10) as u8;
        }

        core::str::from_utf8(&self.bytes[..self.len]).expect("fps report is ASCII")
    }
}
