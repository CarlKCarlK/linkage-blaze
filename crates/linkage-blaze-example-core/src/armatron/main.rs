//! Generic helpers for the armatron example.
//!
//! The device-agnostic game loop lives here.
//!
//! The generic loop redraws every frame, updates immediate-mode controls, and
//! flushes frames through the [`Cyd`](linkage_blaze_cyd_core::Cyd) boundary.

pub mod calibration;
mod controlled;
mod controls;
pub mod reverse_kinematics;

use core::convert::Infallible;

use embassy_time::Instant;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::{IntoStorage, Rgb565, WebColors},
    prelude::*,
};
use linkage_blaze_core::{
    LinkageFixed, LinkageView, Projection, Rgb888, Vec3, linkage, linkage_fixed, rgb565_from_rgb888,
};
use linkage_blaze_cyd_core::{
    Cyd, CydDisplay, CydFrame, CydTouch, DrawItem3dExt, SCREEN_HEIGHT, SCREEN_PIXELS, SCREEN_WIDTH,
};
use nanorand::{Rng, WyRand};
use static_cell::StaticCell;

use crate::ui::{Ui, UiError};
use controls::{
    CALIBRATE_BUTTON, DISTANCE_LABEL, DOLLY_SLIDER, FPS_LABEL, NEXT_TARGET_BUTTON,
    PARAM_SLIDER_COUNT, PARAM_SLIDERS, PREVIOUS_TARGET_BUTTON, RK_RUN_BUTTON, RK_STEP_BUTTON,
    TARGET_LABEL, TILT_SLIDER, VERSION_LABEL, VERSION_TEXT, XY_VIEW_SLIDER,
};

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BACKGROUND: Rgb888 = Rgb888::CSS_BLACK;
pub const FOREGROUND: Rgb888 = Rgb888::CSS_WHITE;

// ── Armatron state constants ─────────────────────────────────────────────────

const FPS_TEXT_MAX_TENTHS: u32 = 990;

// ---- linkages ----
//
// Build the displayed scene in layers:
// - `CAMERA_CONTROL` provides the view-control params shared by the scene and
//   the arm-tip distance helper linkage.
// - `CAMERA_AND_GRID` adds the static floor grid under the arm.
// - `SCENE_WITH_ARM` adds the articulated arm plus joint spheres for display.
//       The arm linkage ends with an invisible tip in the center of the hand.
// - `LINKAGE_FIXED` appends a red ghost arm that shows the current target pose.
const CAMERA_CONTROL: LinkageFixed<3, 1, 8> =
    linkage_fixed!("../../../linkage-blaze-armatron-core/src/camera_control.lb.rs");
const GRID_9X9: LinkageFixed<0, 1, 81> =
    linkage_fixed!("../../../linkage-blaze-armatron-core/src/grid_9x9.lb.rs");
const CAMERA_AND_GRID: LinkageFixed<3, 2, 88> = CAMERA_CONTROL.combine(GRID_9X9);
const ARMATRON1: LinkageFixed<6, 1, 25> =
    linkage_fixed!("../../../linkage-blaze-armatron-core/src/armatron1.lb.rs");
const ARMATRON1_WITH_JOINTS: LinkageFixed<6, 1, 45> = ARMATRON1.with_joint_spheres(0.15);
const SCENE_WITH_ARM: LinkageFixed<9, 3, 133> = CAMERA_AND_GRID.combine(ARMATRON1_WITH_JOINTS);
const LINKAGE_FIXED: LinkageFixed<15, 4, 159> = SCENE_WITH_ARM
    .restore("scene origin")
    .combine(ARMATRON1) // Add a red ghost arm to hold the current target pose.
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0);
const LINKAGE: LinkageView<15, 4> = LINKAGE_FIXED.view();
// Minimal linkage used only to measure arm-tip distance to the target.
const ARM_TIP_LINKAGE_FIXED: LinkageFixed<9, 2, 32> = CAMERA_CONTROL.combine(ARMATRON1);
const ARM_TIP_LINKAGE: LinkageView<9, 2> = ARM_TIP_LINKAGE_FIXED.view();

// The ghost arm's params begin immediately after the displayed scene's params.
const TARGET_PARAM_START: usize = SCENE_WITH_ARM.view().dof();

const XY_VIEW_PARAM_INDEX: usize = LINKAGE.param_index(XY_VIEW_SLIDER.label(), 0);
const TILT_PARAM_INDEX: usize = LINKAGE.param_index(TILT_SLIDER.label(), 0);
const DOLLY_PARAM_INDEX: usize = LINKAGE.param_index(DOLLY_SLIDER.label(), 0);
const ARM_PARAM_INDEXES: [usize; PARAM_SLIDER_COUNT] = {
    let mut indexes = [0; PARAM_SLIDER_COUNT];
    let mut slider_index = 0;
    while slider_index < PARAM_SLIDER_COUNT {
        indexes[slider_index] = LINKAGE.param_index(PARAM_SLIDERS[slider_index].label(), 0);
        slider_index += 1;
    }
    indexes
};

pub const DOF: usize = LINKAGE.dof();

const SHOW_FPS_TEXT: bool = true;

const PROJECTION: Projection = Projection::front_perspective(
    Point::new(SCREEN_WIDTH as i32 / 2, SCREEN_HEIGHT as i32 / 2),
    SCREEN_WIDTH as f32 / 16.0, // 16 world units span the screen width
    30.0,
);

// ── Generic armatron loop ─────────────────────────────────────────────────────

/// Run the armatron example forever.
///
/// Each iteration:
/// 1. Reads the next touch event from [`CydTouch::read`].
/// 2. Draws the scene from the params produced by the previous frame's widgets.
/// 3. Runs immediate-mode widgets on top, which update params for the next
///    frame and therefore introduce a one-frame scene latency.
/// 4. Presents a full-screen CYD frame.
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
    randomize_target_from_seed(target_seed, &mut params);

    // Set up state.
    let mut ui = Ui::new();
    let mut previous_tick = None;

    // Set up buffers
    let mut frame = display.full_frame_mut();
    let background565 = rgb565_from_rgb888(BACKGROUND);

    loop {
        // todo000 review CydFrame::clear; its name collision with DrawTarget::clear(color) makes
        // generic frame code use fill(...) instead, which makes the clear helper much less useful.
        frame.fill(background565);

        // Scene first, UI on top: params here were updated by last frame's
        // widgets, so input affects the scene with the standard one-frame
        // immediate-mode latency.
        for draw_item_3d in LINKAGE.draw_items_3d(&params) {
            draw_item_3d.project(&PROJECTION).draw(&mut frame);
        }

        // todo It's weird this doesn't return an error of the right type already and needs to be converted
        ui.begin(touch.read().map_err(Error::Cyd)?);

        ui.slider(&mut frame, &TILT_SLIDER, &mut params[TILT_PARAM_INDEX])?;
        ui.slider(&mut frame, &DOLLY_SLIDER, &mut params[DOLLY_PARAM_INDEX])?;
        ui.slider(
            &mut frame,
            &XY_VIEW_SLIDER,
            &mut params[XY_VIEW_PARAM_INDEX],
        )?;
        for (param_slider, param_index) in PARAM_SLIDERS.iter().zip(ARM_PARAM_INDEXES) {
            ui.slider(&mut frame, param_slider, &mut params[param_index])?;
        }

        if ui.button(&mut frame, &PREVIOUS_TARGET_BUTTON)? {
            target_seed = target_seed.wrapping_sub(1);
            randomize_target_from_seed(target_seed, &mut params);
        }
        if ui.button(&mut frame, &NEXT_TARGET_BUTTON)? {
            target_seed = target_seed.wrapping_add(1);
            randomize_target_from_seed(target_seed, &mut params);
        }

        // Clicks not yet wired to actions (matches current behavior).
        ui.icon_button(&mut frame, &RK_RUN_BUTTON)?;
        ui.icon_button(&mut frame, &RK_STEP_BUTTON)?;
        ui.button(&mut frame, &CALIBRATE_BUTTON)?;

        ui.label(
            &mut frame,
            &TARGET_LABEL,
            format_args!("target #{target_seed}"),
        )?;
        let distance_hundredths = target_distance_hundredths(&params);
        ui.label(
            &mut frame,
            &DISTANCE_LABEL,
            format_args!(
                "distance {:02}.{:02}",
                distance_hundredths / 100,
                distance_hundredths % 100
            ),
        )?;
        if let Some((fps_whole, fps_fraction)) = next_fps_label(&mut previous_tick) {
            ui.label(
                &mut frame,
                &FPS_LABEL,
                format_args!("{fps_whole:>2}.{fps_fraction} fps"),
            )?;
        }
        ui.label(&mut frame, &VERSION_LABEL, format_args!("{VERSION_TEXT}"))?;

        ui.end(&mut frame)?;

        frame.flush().await.map_err(Error::Cyd)?;
    }
}

/// Error from the generic armatron loop, generic over the CYD device error `F`.
///
/// Local UI errors such as [`UiError`] get a derived `From`, so they propagate
/// with a plain `?`. The CYD device error `F` is the one exception: it is
/// converted explicitly with `.map_err(Error::Cyd)` at the call site, because
/// a blanket `From<F>` would overlap with those concrete `From`s under
/// coherence.
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// A UI widget failed (text formatting; draw is [`Infallible`] here).
    Ui(UiError<Infallible>),
    /// Reading touch events or flushing a frame failed.
    #[from(ignore)]
    Cyd(F),
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

// todo000 shared home question from SINGLE_SOURCE_SPEC.md: if both armatron UI
// paths stay live, move arm_tip/target_center/compute_target_distance/
// VERSION_TEXT into one shared module instead of duplicating them here.
fn randomize_target_from_seed(target_seed: u8, params: &mut [f32; DOF]) {
    let mut rng = WyRand::new_seed(u64::from(target_seed));
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
    }
}

fn target_distance_hundredths(params: &[f32; DOF]) -> u32 {
    // Display bound: the label format only has room for "distance 99.99".
    libm::roundf(target_distance(params).clamp(0.0, 99.99) * 100.0) as u32
}

fn next_fps_label(previous_tick: &mut Option<Instant>) -> Option<(u32, u32)> {
    let current_tick = Instant::now();
    let fps_label = if SHOW_FPS_TEXT
        && let Some(previous_tick) = *previous_tick
        && let Some((fps_whole, fps_fraction)) = display_fps_since(previous_tick, current_tick)
    {
        Some((fps_whole, fps_fraction))
    } else {
        None
    };
    *previous_tick = Some(current_tick);
    fps_label
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
    let mut arm_params = [0.0f32; TARGET_PARAM_START];
    arm_params.copy_from_slice(&params[..TARGET_PARAM_START]);
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
    arm_tip(rk_linkage, params).distance_to(target_center(linkage, params))
}

fn target_distance(params: &[f32; DOF]) -> f32 {
    compute_target_distance(ARM_TIP_LINKAGE, LINKAGE, params)
}
