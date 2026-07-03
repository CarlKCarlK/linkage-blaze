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

use core::{
    convert::Infallible,
    fmt::{self, Write},
};

use embassy_time::Instant;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{IntoStorage, Rgb565, WebColors},
    prelude::*,
    primitives::PrimitiveStyle,
    text::{Baseline, Text},
};
use linkage_blaze_core::{
    LinkageFixed, LinkageView, Projection, Rgb888, Vec3, linkage, linkage_fixed,
    rgb565_from_rgb888_components,
};
use linkage_blaze_cyd_core::{
    Cyd, CydDisplay, CydFrame, CydTouch, DrawItem3dExt, SCREEN_HEIGHT, SCREEN_PIXELS, SCREEN_WIDTH,
};
use nanorand::{Rng, WyRand};
use static_cell::StaticCell;

use controls::{ArmatronControls, PARAM_SLIDER_COUNT, TARGET_TEXT_POINT};

use crate::infallible::InfallibleResultExt;

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BACKGROUND: Rgb888 = Rgb888::new(0, 0, 0); // black
pub const BLACK: Rgb888 = BACKGROUND;
pub const WHITE: Rgb888 = Rgb888::new(255, 255, 255); // white
pub const YELLOW: Rgb888 = Rgb888::new(255, 255, 0); // yellow
const BACKGROUND_565: Rgb565 = rgb565_from_rgb888_components(0, 0, 0); // black

// ── Armatron state constants ─────────────────────────────────────────────────

const TEXT_CHAR_WIDTH: i32 = 6;
const DISTANCE_REPORT_WIDTH: i32 = 14 * TEXT_CHAR_WIDTH;
const DISTANCE_REPORT_LEFT: i32 = ((SCREEN_WIDTH as i32 - DISTANCE_REPORT_WIDTH) / 2) - 16;
const DISTANCE_REPORT_TOP: i32 = 5;
const DISTANCE_TEXT_POINT: Point = Point::new(DISTANCE_REPORT_LEFT, DISTANCE_REPORT_TOP);
const DISTANCE_TEXT_STYLE: MonoTextStyle<'static, Rgb565> =
    MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_WHITE);
const TARGET_TEXT_STYLE: MonoTextStyle<'static, Rgb565> =
    MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_WHITE);
const TEXT_BUFFER_LEN: usize = 14; // "distance 99.99", the longest of the status texts
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
const VERSION_TEXT_POINT: Point = Point::new(VERSION_REPORT_LEFT, VERSION_REPORT_TOP);
const VERSION_TEXT_STYLE: MonoTextStyle<'static, Rgb565> =
    MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_LIGHT_SLATE_GRAY);

// ---- world / display constants ----
const PIXELS_PER_UNIT: f32 = SCREEN_WIDTH as f32 / 16.0; // 16 world units span the screen width

// ---- parameter indices ----
const TARGET_PARAM_START: usize = 9;
const XY_VIEW_PARAM_NAME: &str = "x/y view";
const TILT_PARAM_NAME: &str = "z";
const DOLLY_PARAM_NAME: &str = "zoom";
const ARM_PARAM_NAMES: [&str; PARAM_SLIDER_COUNT] = [
    "raise hand",
    "bend elbow",
    "close hand",
    "lower arm",
    "spin whole arm",
    "spin hand",
];

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

const XY_VIEW_PARAM_INDEX: usize = LINKAGE.param_index(XY_VIEW_PARAM_NAME, 0);
const TILT_PARAM_INDEX: usize = LINKAGE.param_index(TILT_PARAM_NAME, 0);
const DOLLY_PARAM_INDEX: usize = LINKAGE.param_index(DOLLY_PARAM_NAME, 0);
const ARM_PARAM_INDEXES: [usize; PARAM_SLIDER_COUNT] = [
    LINKAGE.param_index(ARM_PARAM_NAMES[0], 0),
    LINKAGE.param_index(ARM_PARAM_NAMES[1], 0),
    LINKAGE.param_index(ARM_PARAM_NAMES[2], 0),
    LINKAGE.param_index(ARM_PARAM_NAMES[3], 0),
    LINKAGE.param_index(ARM_PARAM_NAMES[4], 0),
    LINKAGE.param_index(ARM_PARAM_NAMES[5], 0),
];

pub const DOF: usize = LINKAGE.dof();

const SHOW_FPS_TEXT: bool = true;

// ── Generic armatron loop ─────────────────────────────────────────────────────

/// Run the armatron example forever.
///
/// Each iteration:
/// 1. Reads the next touch event from [`CydTouch::read`].
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
    let mut target_seed: u8 = 0;
    let mut rng = WyRand::new_seed(u64::from(target_seed));
    // todo00 how to we feel about "TARGET_PARAM_START"
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
    }

    // Set up state.
    let mut controls = ArmatronControls::new(
        params[XY_VIEW_PARAM_INDEX],
        params[TILT_PARAM_INDEX],
        params[DOLLY_PARAM_INDEX],
        ARM_PARAM_INDEXES.map(|param_index| params[param_index]),
        ARM_PARAM_NAMES,
    );
    let mut previous_tick = None;

    // Set up buffers
    let mut frame = display.full_frame_mut();
    let mut text_buf = heapless::String::<TEXT_BUFFER_LEN>::new();

    loop {
        // todo000 review CydFrame::clear; its name collision with DrawTarget::clear(color) makes
        // generic frame code use fill(...) instead, which makes the clear helper much less useful.
        frame.fill(BACKGROUND_565);

        // Update controls from the next touch event (if any).
        // todo It's weird this doesn't return an error of the right type already and needs to be converted
        controls.set_event(touch.read().map_err(Error::Cyd)?);

        // Update the main params from the controls.
        linkage_params(&controls, &mut params);

        // Update the seed and target params if requested.
        target_seed = update_target(&controls, target_seed, &mut params);

        // Draw the linkage (arm + target)
        for draw_item_3d in LINKAGE.draw_items_3d(&params) {
            draw_item_3d.project(&PROJECTION).draw(&mut frame);
        }

        // Draw the controls
        controls.draw(&mut frame).unwrap_infallible();

        // Display FPS if requested and available.
        previous_tick = draw_fps_text(&mut frame, &mut text_buf, previous_tick)?;

        // Draw the target #, distance to target, and version text.
        draw_text_info(&mut frame, &mut text_buf, target_seed, &params)?;

        controls.draw_touch_cursor(&mut frame).unwrap_infallible();

        frame.flush().await.map_err(Error::Cyd)?;
    }
}

/// Error from the generic armatron loop, generic over the CYD device error `F`.
///
/// Local errors such as [`fmt::Error`] get a derived `From`, so they
/// propagate with a plain `?`. The CYD device error `F` is the one exception:
/// it is converted explicitly with `.map_err(Error::Cyd)` at the call site,
/// because a blanket `From<F>` would overlap with those concrete `From`s under
/// coherence.
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// Formatting the FPS report failed.
    FpsReport(fmt::Error),
    /// Reading touch events or flushing a frame failed.
    #[from(ignore)]
    Cyd(F),
}

//todo0000 revisit Robot Ortho projection (+Z up, +Y left, drops X): reconsider after camera_control is updated
const PROJECTION: Projection = Projection::front_perspective(
    Point::new(SCREEN_WIDTH as i32 / 2, SCREEN_HEIGHT as i32 / 2),
    PIXELS_PER_UNIT,
    30.0,
);

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

fn linkage_params(controls: &ArmatronControls, params: &mut [f32; DOF]) {
    params[XY_VIEW_PARAM_INDEX] = controls.xy_view();
    params[TILT_PARAM_INDEX] = controls.tilt();
    params[DOLLY_PARAM_INDEX] = controls.dolly();
    let param_sliders = controls.param_sliders();
    for (slider_value, param_index) in param_sliders.into_iter().zip(ARM_PARAM_INDEXES) {
        params[param_index] = slider_value;
    }
}

/// Advance `target_seed` if a target button was clicked, randomizing the target params to match.
///
/// Returns the new `target_seed`, which is unchanged if neither button was clicked.
fn update_target(controls: &ArmatronControls, target_seed: u8, params: &mut [f32; DOF]) -> u8 {
    let target_seed = if controls.previous_target.was_clicked() {
        target_seed.wrapping_sub(1)
    } else if controls.next_target.was_clicked() {
        target_seed.wrapping_add(1)
    } else {
        return target_seed;
    };

    let mut rng = WyRand::new_seed(u64::from(target_seed));
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
    }
    target_seed
}

/// Draw the target #, distance to target, and version text into `frame`.
fn draw_text_info(
    frame: &mut impl CydFrame,
    text_buf: &mut heapless::String<TEXT_BUFFER_LEN>,
    target_seed: u8,
    params: &[f32; DOF],
) -> Result<(), fmt::Error> {
    text_buf.clear();
    write!(text_buf, "target #{target_seed}")?;
    Text::with_baseline(
        text_buf,
        TARGET_TEXT_POINT,
        TARGET_TEXT_STYLE,
        Baseline::Top,
    )
    .draw(frame)
    .unwrap_infallible();

    let distance_hundredths = round_to_u32(target_distance(params).clamp(0.0, 99.99) * 100.0);
    text_buf.clear();
    write!(
        text_buf,
        "distance {:02}.{:02}",
        distance_hundredths / 100,
        distance_hundredths % 100
    )?;
    Text::with_baseline(
        text_buf,
        DISTANCE_TEXT_POINT,
        DISTANCE_TEXT_STYLE,
        Baseline::Top,
    )
    .draw(frame)
    .unwrap_infallible();

    Text::with_baseline(
        VERSION_TEXT,
        VERSION_TEXT_POINT,
        VERSION_TEXT_STYLE,
        Baseline::Top,
    )
    .draw(frame)
    .unwrap_infallible();

    Ok(())
}

/// Draw the FPS report into `frame` if enabled and a previous tick is available.
///
/// Returns the current tick, for the caller to store as the next `previous_tick`.
fn draw_fps_text(
    frame: &mut impl CydFrame,
    text_buf: &mut heapless::String<TEXT_BUFFER_LEN>,
    previous_tick: Option<Instant>,
) -> Result<Option<Instant>, fmt::Error> {
    let current_tick = Instant::now();
    if SHOW_FPS_TEXT
        && let Some(previous_tick) = previous_tick
        && let Some((fps_whole, fps_fraction)) = display_fps_since(previous_tick, current_tick)
    {
        text_buf.clear();
        write!(text_buf, "{fps_whole:>2}.{fps_fraction} fps")?;
        Text::with_baseline(text_buf, FPS_TEXT_POINT, FPS_TEXT_STYLE, Baseline::Top)
            .draw(frame)
            .unwrap_infallible();
    }
    Ok(Some(current_tick))
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
