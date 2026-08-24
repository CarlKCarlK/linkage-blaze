//! Generic helpers for the armatron example.
//!
//! The device-agnostic game loop lives here.
//!
//! The generic loop redraws every frame, updates immediate-mode controls, and
//! flushes frames through [`CydDisplay`].

mod controlled;
mod controls;
pub mod reverse_kinematics;

use core::convert::Infallible;

use crate::render::Projection;
use crate::{
    DrawItem3dExt, Error as LinkageError, LinkageFixed, LinkageView, Rgb888, Step, linkage_file,
};
use device_envoy_core::{
    button::Button,
    cyd::{
        Cyd, CydDisplay, CydTouch,
        display::{CydFrame, Orientation},
    },
};
use embassy_time::Instant;
use embedded_graphics::{geometry::Point, pixelcolor::WebColors};
use nanorand::{Rng, WyRand};

use crate::examples::ui::{Error as UiError, UiFrame, UiState};
use controls::{
    CALIBRATE_BUTTON, DISTANCE_LABEL, DOLLY_SLIDER, FPS_LABEL, NEXT_TARGET_BUTTON,
    PARAM_SLIDER_COUNT, PARAM_SLIDERS, PREVIOUS_TARGET_BUTTON, RK_STEP_BUTTON, TARGET_LABEL,
    TILT_SLIDER, VERSION_LABEL, VERSION_TEXT, XY_VIEW_SLIDER,
};
use reverse_kinematics::ReverseKinematics;

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BACKGROUND_COLOR: Rgb888 = Rgb888::CSS_BLACK;
pub const FOREGROUND_COLOR: Rgb888 = Rgb888::CSS_WHITE;

// ---- linkages ----
//
// Build the displayed scene in layers:
// - `CAMERA_CONTROL` provides the view-control params shared by the scene and
//   the arm-tip distance helper linkage.
// - `SCENE_WITH_ARM` adds the static floor grid and articulated arm plus joint spheres.
//       The arm linkage ends with an invisible tip in the center of the hand.
// - `LINKAGE` appends a red ghost arm that shows the current target pose.
linkage_file! {
    camera_control {
        file: "../../assets/examples/armatron/camera_control.lb.rs",
    }
}
linkage_file! {
    grid9x9 {
        file: "../../assets/examples/armatron/grid_9x9.lb.rs",
    }
}
linkage_file! {
    armatron1 {
        file: "../../assets/examples/armatron/armatron1.lb.rs",
    }
}
const CAMERA_AND_GRID: LinkageFixed<
    { camera_control::DOF + grid9x9::DOF },
    { camera_control::MARKS + grid9x9::MARKS },
    { camera_control::STEP_COUNT + grid9x9::STEP_COUNT - 1 },
> = camera_control::fixed().combine(grid9x9::view());
const ARMATRON_WITH_JOINTS: LinkageFixed<
    { armatron1::DOF },
    { armatron1::MARKS },
    { joint_sphere_step_count(&armatron1::fixed()) },
> = with_joint_spheres(armatron1::fixed(), 0.15);
const SCENE_WITH_ARM: LinkageFixed<
    { CAMERA_AND_GRID.dof() + ARMATRON_WITH_JOINTS.dof() }, // Combined parameter count (DOF).
    { CAMERA_AND_GRID.mark_count() + ARMATRON_WITH_JOINTS.mark_count() }, // Combined mark-slot count.
    // `combine` omits the right-hand `Start`, leaving one spare slot for the later `restore`.
    { CAMERA_AND_GRID.step_count() + ARMATRON_WITH_JOINTS.step_count() },
> = CAMERA_AND_GRID.combine(ARMATRON_WITH_JOINTS.view());
// `pen_color` and `sphere_param` each append one step after the ghost arm.
const TARGET_SUFFIX_STEP_COUNT: usize = 2;
const LINKAGE_FIXED: LinkageFixed<
    { SCENE_WITH_ARM.dof() + armatron1::DOF },
    { SCENE_WITH_ARM.mark_count() + armatron1::MARKS },
    { SCENE_WITH_ARM.step_count() + armatron1::STEP_COUNT + TARGET_SUFFIX_STEP_COUNT },
> = SCENE_WITH_ARM
    .restore("scene origin")
    .combine(armatron1::view())
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0);
const LINKAGE: LinkageView<{ LINKAGE_FIXED.dof() }, { LINKAGE_FIXED.mark_count() }> =
    LINKAGE_FIXED.view();
// Minimal linkage used only to measure arm-tip distance to the target.
const ARM_TIP_LINKAGE_FIXED: LinkageFixed<
    { camera_control::DOF + armatron1::DOF },
    { camera_control::MARKS + armatron1::MARKS },
    { camera_control::STEP_COUNT + armatron1::STEP_COUNT - 1 },
> = camera_control::fixed().combine(armatron1::view());
const ARM_TIP_LINKAGE: LinkageView<
    { ARM_TIP_LINKAGE_FIXED.dof() },
    { ARM_TIP_LINKAGE_FIXED.mark_count() },
> = ARM_TIP_LINKAGE_FIXED.view();

// The ghost arm's params begin immediately after the displayed scene's params.
const TARGET_PARAM_START: usize = SCENE_WITH_ARM.dof();
const ORIENTATION: Orientation = Orientation::Landscape;

const XY_VIEW_PARAM_INDEX: usize = LINKAGE.param_index(XY_VIEW_SLIDER.label(), 0);
const TILT_PARAM_INDEX: usize = LINKAGE.param_index(TILT_SLIDER.label(), 0);
const DOLLY_PARAM_INDEX: usize = LINKAGE.param_index(DOLLY_SLIDER.label(), 0);
// Resolve arm sliders to linkage indexes at compile time for controls and search.
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

const PROJECTION: Projection = Projection::front_perspective(
    Point::new(
        ORIENTATION.width() as i32 / 2,
        ORIENTATION.height() as i32 / 2,
    ),
    ORIENTATION.width() as f32 / 16.0, // 16 world units span the screen width
    30.0,
);

// ── Generic armatron loop ─────────────────────────────────────────────────────

/// Run the Armatron example until physical or on-screen input requests calibration.
pub async fn run<CydDevice, ButtonDevice>(
    cyd: &mut CydDevice,
    button: &mut ButtonDevice,
) -> Result<Exit, Error<CydDevice::Error>>
where
    CydDevice: Cyd,
    ButtonDevice: Button,
{
    // Set the initial params including a random target.
    let mut params = LINKAGE.param_defaults();
    let mut target_seed: u8 = 0;
    randomize_target_from_seed(target_seed, &mut params);

    // Set up state.
    let mut ui_state = UiState::new();
    let mut reverse_kinematics = ReverseKinematics::new();
    let mut previous_tick = None;

    loop {
        if button.is_pressed() {
            return Ok(Exit::CalibrationRequested);
        }

        let (display, touch) = cyd.parts();
        let touch_event = touch.read().map_err(Error::Cyd)?;
        let mut frame = display.full_frame_mut();
        let current_tick = Instant::now();
        frame.clear();

        // Draw the scene before widgets so the UI appears on top.
        for draw_item_3d in LINKAGE.draw_items_3d(&params)? {
            draw_item_3d.project(&PROJECTION).draw(&mut frame);
        }

        let mut ui_frame = UiFrame::new(&mut ui_state, touch_event, &mut frame);

        ui_frame.slider(&TILT_SLIDER, &mut params[TILT_PARAM_INDEX])?;
        ui_frame.slider(&DOLLY_SLIDER, &mut params[DOLLY_PARAM_INDEX])?;
        ui_frame.slider(&XY_VIEW_SLIDER, &mut params[XY_VIEW_PARAM_INDEX])?;
        for (param_slider, param_index) in PARAM_SLIDERS.iter().zip(ARM_PARAM_INDEXES) {
            if ui_frame.slider(param_slider, &mut params[param_index])? {
                reverse_kinematics.clear();
            }
        }

        if ui_frame.button(&PREVIOUS_TARGET_BUTTON)? {
            reverse_kinematics.clear();
            target_seed = target_seed.wrapping_sub(1);
            randomize_target_from_seed(target_seed, &mut params);
        }
        if ui_frame.button(&NEXT_TARGET_BUTTON)? {
            reverse_kinematics.clear();
            target_seed = target_seed.wrapping_add(1);
            randomize_target_from_seed(target_seed, &mut params);
        }
        if ui_frame.icon_button(reverse_kinematics.run_button())? {
            reverse_kinematics.toggle(&params)?;
        }
        let hold_button_state = ui_frame.hold_button(&RK_STEP_BUTTON)?;

        if ui_frame.button(&CALIBRATE_BUTTON)? {
            return Ok(Exit::CalibrationRequested);
        }

        // Explicit per-frame solver schedule slot.
        let dt_seconds = previous_tick.map_or(0.0, |previous_tick| {
            current_tick
                .saturating_duration_since(previous_tick)
                .as_micros() as f32
                / 1_000_000.0
        });
        reverse_kinematics.hold_step(&mut params, hold_button_state, dt_seconds)?;
        reverse_kinematics.tick(&mut params, dt_seconds)?;

        ui_frame.label(&TARGET_LABEL, format_args!("target #{target_seed}"))?;
        let distance_hundredths = target_distance_hundredths(&params)?;
        ui_frame.label(
            &DISTANCE_LABEL,
            format_args!(
                "distance {:02}.{:02}",
                distance_hundredths / 100,
                distance_hundredths % 100
            ),
        )?;
        if let Some((fps_whole, fps_fraction)) =
            previous_tick.and_then(|previous_tick| display_fps_since(previous_tick, current_tick))
        {
            ui_frame.label(
                &FPS_LABEL,
                format_args!("{fps_whole:>2}.{fps_fraction} fps"),
            )?;
        }
        ui_frame.label(&VERSION_LABEL, format_args!("{VERSION_TEXT}"))?;

        ui_frame.draw_touch_cursor()?;

        frame.flush().await.map_err(Error::Cyd)?;
        previous_tick = Some(current_tick);
    }
}

/// Error from the generic armatron loop, generic over the CYD device error `CydError`.
///
/// Local UI errors such as [`UiError`] get a derived `From`, so they propagate
/// with a plain `?`. The CYD device error `CydError` is the one exception: it
/// is converted explicitly with `.map_err(Error::Cyd)` at the call site,
/// because a blanket `From<CydError>` would overlap with those concrete `From`s under
/// coherence.
#[derive(Debug, derive_more::From)]
pub enum Error<CydError> {
    /// A runtime linkage parameter was invalid.
    Linkage(LinkageError),
    /// A UI widget failed (text formatting; draw is infallible here).
    Ui(UiError<Infallible>),
    /// Reading touch events or flushing a frame failed.
    #[from(ignore)]
    Cyd(CydError),
}

#[derive(Debug)]
pub enum Exit {
    CalibrationRequested,
}

#[cfg(test)]
mod tests {
    use device_envoy_core::cyd::touch::TouchEvent;
    use device_envoy_core::memory::{
        CydMemory, Error as CydMemoryError, assert_framebuffer_matches_expected_png,
    };
    use embedded_graphics::mono_font::ascii::FONT_9X15_BOLD;
    use futures_executor::block_on;

    use super::controls::CALIBRATE_BUTTON;
    use super::{Error, Exit, run};

    fn test_memory_cyd() -> CydMemory {
        CydMemory::new(
            embedded_graphics::geometry::Size::new(320, 240),
            super::BACKGROUND_COLOR,
            super::FOREGROUND_COLOR,
            &FONT_9X15_BOLD,
        )
    }

    #[test]
    fn tapping_the_calibrate_button_requests_calibration() -> Result<(), Error<CydMemoryError>> {
        let mut memory_cyd = test_memory_cyd();
        let touch_rectangle = CALIBRATE_BUTTON.touch_rectangle();
        let touch_center = touch_rectangle.top_left
            + embedded_graphics::geometry::Point::new(
                touch_rectangle.size.width as i32 / 2,
                touch_rectangle.size.height as i32 / 2,
            );
        memory_cyd.push_touch_event(TouchEvent::Down {
            point: touch_center,
        });
        let mut memory_button = memory_cyd.button_memory();

        let armatron_exit = block_on(run(&mut memory_cyd, &mut memory_button))?;

        assert!(matches!(armatron_exit, Exit::CalibrationRequested));
        assert_eq!(
            memory_cyd.flush_count(),
            0,
            "the calibrate-button exit happens before the frame is flushed"
        );
        Ok(())
    }

    #[test]
    fn boot_requests_calibration() -> Result<(), Error<CydMemoryError>> {
        let mut memory_cyd = test_memory_cyd();
        memory_cyd.set_frame_budget(1);
        let mut memory_button = memory_cyd.button_memory();
        memory_button.set_pressed_for_frame(0, true);

        let armatron_exit = block_on(run(&mut memory_cyd, &mut memory_button))?;

        assert!(matches!(armatron_exit, Exit::CalibrationRequested));
        assert_eq!(memory_cyd.flush_count(), 0);
        Ok(())
    }

    #[test]
    fn armatron_renders_expected_frame() {
        let mut memory_cyd = test_memory_cyd();
        memory_cyd.set_frame_budget(1);
        let mut memory_button = memory_cyd.button_memory();

        let armatron_error = block_on(run(&mut memory_cyd, &mut memory_button))
            .expect_err("the free-running loop should stop at the frame budget");
        assert!(matches!(
            armatron_error,
            Error::Cyd(CydMemoryError::OutOfFrames)
        ));

        assert_framebuffer_matches_expected_png(
            &memory_cyd,
            env!("CARGO_MANIFEST_DIR"),
            "armatron.png",
        )
        .expect("rendered frame should match the golden image");
    }
}

// ── Private helper functions ───────────────────────────────────────────────────

const fn joint_sphere_step_count<const DOF: usize, const MARKS: usize, const N: usize>(
    linkage: &LinkageFixed<DOF, MARKS, N>,
) -> usize {
    let mut count = linkage.len;
    let mut step_index = 0;
    while step_index < linkage.len {
        if matches!(
            linkage.steps[step_index],
            Step::Forward(_) | Step::Left(_) | Step::Up(_)
        ) {
            count += 2;
        }
        step_index += 1;
    }
    count
}

const fn with_joint_spheres<
    const DOF: usize,
    const MARKS: usize,
    const N: usize,
    const N_OUT: usize,
>(
    linkage: LinkageFixed<DOF, MARKS, N>,
    joint_radius: f32,
) -> LinkageFixed<DOF, MARKS, N_OUT> {
    let mut output = LinkageFixed {
        steps: [const { Step::Start }; N_OUT],
        len: 0,
        params: linkage.params,
        param_len: linkage.param_len,
        mark_names: linkage.mark_names,
        mark_len: linkage.mark_len,
    };
    let mut step_index = 0;
    while step_index < linkage.len {
        let step = linkage.steps[step_index];
        let is_translation = matches!(step, Step::Forward(_) | Step::Left(_) | Step::Up(_));
        if is_translation {
            assert!(output.len < N_OUT, "joint-sphere output capacity too small");
            output.steps[output.len] = Step::Sphere(joint_radius);
            output.len += 1;
        }
        assert!(output.len < N_OUT, "joint-sphere output capacity too small");
        output.steps[output.len] = step;
        output.len += 1;
        if is_translation {
            assert!(output.len < N_OUT, "joint-sphere output capacity too small");
            output.steps[output.len] = Step::Sphere(joint_radius);
            output.len += 1;
        }
        step_index += 1;
    }
    output
}

fn randomize_target_from_seed(target_seed: u8, params: &mut [f32; DOF]) {
    let mut rng = WyRand::new_seed(u64::from(target_seed));
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0);
    }
}

fn target_distance_hundredths(params: &[f32; DOF]) -> Result<u32, LinkageError> {
    // Display bound: the label format only has room for "distance 99.99".
    Ok(libm::roundf(target_distance(params)?.clamp(0.0, 99.99) * 100.0) as u32)
}

fn display_fps_since(previous_tick: Instant, current_tick: Instant) -> Option<(u32, u32)> {
    let elapsed_micros = current_tick
        .saturating_duration_since(previous_tick)
        .as_micros();

    (elapsed_micros != 0).then(|| {
        // Convert microseconds/frame to tenths of frames/second, rounded.
        let fps_tenths = 10_000_000_u64.saturating_add(elapsed_micros / 2) / elapsed_micros;
        let fps_tenths = fps_tenths.min(999) as u32;
        (fps_tenths / 10, fps_tenths % 10)
    })
}

fn target_distance(params: &[f32; DOF]) -> Result<f32, LinkageError> {
    let mut arm_params = [0.0f32; TARGET_PARAM_START];
    arm_params.copy_from_slice(&params[..TARGET_PARAM_START]);
    let arm_tip = ARM_TIP_LINKAGE.final_pose(&arm_params)?.position();
    let target_center = LINKAGE.final_pose(params)?.position();
    Ok(arm_tip.distance_to(target_center))
}
