use core::fmt::Write;

use device_envoy_core::flash_block::FlashBlock;
use heapless::String;

use crate::{Cyd, CydDisplay, CydFrame, CydRawTouch};

use super::{
    CalibrationConfig, CalibrationCorner, CalibrationFlow, CalibrationSolveError,
    draw_calibration_captured_cross, draw_calibration_cross, draw_calibration_instruction,
    draw_calibration_rejected_cross, draw_calibration_verify_target,
    flow::CalibrationFlowEvent,
    flow::{ReleaseTouchCapture, ReleaseTouchCaptureEvent},
    validate_calibration_points,
};

const CAPTURE_ACK_FRAME_COUNT: usize = 8;
const REJECTED_FRAME_COUNT: usize = 30;
const VERIFY_TIMEOUT_POLLS: usize = 600;
const CONFIRMATION_FRAME_COUNT: usize = 60;

/// Result of ensuring calibration at startup.
#[derive(Clone, Copy, Debug)]
pub enum EnsureCalibrationOutcome {
    Loaded(CalibrationConfig),
    Saved(CalibrationConfig),
}

impl EnsureCalibrationOutcome {
    #[must_use]
    pub const fn calibration_config(self) -> CalibrationConfig {
        match self {
            Self::Loaded(calibration_config) | Self::Saved(calibration_config) => {
                calibration_config
            }
        }
    }

    #[must_use]
    pub const fn was_saved(self) -> bool {
        matches!(self, Self::Saved(_))
    }
}

/// Error from the shared calibration driver.
#[derive(Debug)]
pub enum EnsureCalibrationError<DeviceError, FlashError> {
    Device(DeviceError),
    Flash(FlashError),
}

enum CalibrationDriverState {
    Capturing,
    ShowCaptured {
        calibration_corner: CalibrationCorner,
        next_corner: CalibrationCorner,
        frames_remaining: usize,
    },
    ShowRejected {
        worst_residual_pixels: Option<f32>,
        frames_remaining: usize,
    },
    Verifying {
        candidate_config: CalibrationConfig,
        release_touch_capture: ReleaseTouchCapture,
        polls_remaining: usize,
    },
    Confirming {
        calibration_config: CalibrationConfig,
        frames_remaining: usize,
    },
}

/// Ensure that `cyd` has a calibration, running the shared four-tap flow when
/// the flash block does not currently deserialize as a valid configuration.
///
/// Invalid, corrupt, or absent flash content is treated as "not calibrated"
/// instead of bricking boot. The driver simply reruns the calibration flow and
/// overwrites the block with a fresh solve after the candidate is validated and
/// the user confirms it by hitting the center verify target.
pub async fn ensure_calibration<C, F, R, E>(
    cyd: &mut C,
    calibration_flash_block: &mut F,
    mut recalibration_requested: R,
) -> Result<EnsureCalibrationOutcome, EnsureCalibrationError<E, F::Error>>
where
    C: Cyd<Error = E> + CydRawTouch<Error = E>,
    F: FlashBlock,
    R: FnMut() -> bool,
{
    if let Some(calibration_config) = calibration_flash_block
        .load::<CalibrationConfig>()
        .unwrap_or(None)
    {
        return Ok(EnsureCalibrationOutcome::Loaded(calibration_config));
    }

    let mut calibration_flow = CalibrationFlow::new();
    let mut calibration_driver_state = CalibrationDriverState::Capturing;

    loop {
        if recalibration_requested() {
            calibration_flow.restart();
            calibration_driver_state = CalibrationDriverState::Capturing;
        }

        loop {
            let raw_touch_event = cyd
                .read_raw_touch_event()
                .map_err(EnsureCalibrationError::Device)?;
            let Some(raw_touch_event) = raw_touch_event else {
                advance_driver_state_after_idle(
                    &mut calibration_flow,
                    &mut calibration_driver_state,
                );
                break;
            };

            match &mut calibration_driver_state {
                CalibrationDriverState::Capturing => {
                    let Some(calibration_flow_event) =
                        calibration_flow.handle_raw_touch_event(Some(raw_touch_event))
                    else {
                        continue;
                    };

                    match calibration_flow_event {
                        CalibrationFlowEvent::PointCaptured {
                            calibration_corner,
                            next_corner,
                            ..
                        } => {
                            calibration_driver_state = CalibrationDriverState::ShowCaptured {
                                calibration_corner,
                                next_corner,
                                frames_remaining: CAPTURE_ACK_FRAME_COUNT,
                            };
                        }
                        CalibrationFlowEvent::Completed { raw_points, .. } => {
                            match validate_calibration_points(raw_points) {
                                Ok(calibration_validation) => {
                                    calibration_driver_state = CalibrationDriverState::Verifying {
                                        candidate_config: calibration_validation
                                            .calibration_config(),
                                        release_touch_capture: ReleaseTouchCapture::new(),
                                        polls_remaining: VERIFY_TIMEOUT_POLLS,
                                    };
                                }
                                Err(CalibrationSolveError::ResidualTooLarge {
                                    worst_residual_pixels,
                                }) => {
                                    calibration_flow.restart();
                                    calibration_driver_state =
                                        CalibrationDriverState::ShowRejected {
                                            worst_residual_pixels: Some(worst_residual_pixels),
                                            frames_remaining: REJECTED_FRAME_COUNT,
                                        };
                                }
                                Err(CalibrationSolveError::DegenerateGeometry) => {
                                    calibration_flow.restart();
                                    calibration_driver_state =
                                        CalibrationDriverState::ShowRejected {
                                            worst_residual_pixels: None,
                                            frames_remaining: REJECTED_FRAME_COUNT,
                                        };
                                }
                            }
                        }
                    }
                }
                CalibrationDriverState::Verifying {
                    candidate_config,
                    release_touch_capture,
                    ..
                } => {
                    let Some(ReleaseTouchCaptureEvent::Captured { raw_point, .. }) =
                        release_touch_capture.handle_raw_touch_event(Some(raw_touch_event))
                    else {
                        continue;
                    };
                    let (mapped_x, mapped_y) =
                        candidate_config.map_raw_to_screen(raw_point.x, raw_point.y);
                    if hit_verify_target(mapped_x, mapped_y) {
                        calibration_flash_block
                            .save(candidate_config)
                            .map_err(EnsureCalibrationError::Flash)?;
                        calibration_driver_state = CalibrationDriverState::Confirming {
                            calibration_config: *candidate_config,
                            frames_remaining: CONFIRMATION_FRAME_COUNT,
                        };
                    } else {
                        calibration_flow.restart();
                        calibration_driver_state = CalibrationDriverState::ShowRejected {
                            worst_residual_pixels: None,
                            frames_remaining: REJECTED_FRAME_COUNT,
                        };
                    }
                }
                CalibrationDriverState::ShowCaptured { .. }
                | CalibrationDriverState::ShowRejected { .. }
                | CalibrationDriverState::Confirming { .. } => {}
            }
        }

        draw_calibration_screen(cyd, &calibration_flow, &calibration_driver_state)
            .await
            .map_err(EnsureCalibrationError::Device)?;

        if let CalibrationDriverState::Confirming {
            calibration_config,
            frames_remaining: 0,
        } = calibration_driver_state
        {
            return Ok(EnsureCalibrationOutcome::Saved(calibration_config));
        }
    }
}

fn advance_driver_state_after_idle(
    calibration_flow: &mut CalibrationFlow,
    calibration_driver_state: &mut CalibrationDriverState,
) {
    match calibration_driver_state {
        CalibrationDriverState::Capturing => {
            calibration_flow.handle_raw_touch_event(None);
        }
        CalibrationDriverState::ShowCaptured {
            frames_remaining, ..
        } => {
            if *frames_remaining > 0 {
                *frames_remaining -= 1;
            }
            if *frames_remaining == 0 {
                *calibration_driver_state = CalibrationDriverState::Capturing;
            }
        }
        CalibrationDriverState::ShowRejected {
            frames_remaining, ..
        } => {
            if *frames_remaining > 0 {
                *frames_remaining -= 1;
            }
            if *frames_remaining == 0 {
                *calibration_driver_state = CalibrationDriverState::Capturing;
            }
        }
        CalibrationDriverState::Verifying {
            release_touch_capture,
            polls_remaining,
            ..
        } => {
            release_touch_capture.handle_raw_touch_event(None);
            if *polls_remaining > 0 {
                *polls_remaining -= 1;
            }
            if *polls_remaining == 0 {
                calibration_flow.restart();
                *calibration_driver_state = CalibrationDriverState::ShowRejected {
                    worst_residual_pixels: None,
                    frames_remaining: REJECTED_FRAME_COUNT,
                };
            }
        }
        CalibrationDriverState::Confirming {
            frames_remaining, ..
        } => {
            if *frames_remaining > 0 {
                *frames_remaining -= 1;
            }
        }
    }
}

async fn draw_calibration_screen<C>(
    cyd: &mut C,
    calibration_flow: &CalibrationFlow,
    calibration_driver_state: &CalibrationDriverState,
) -> Result<(), C::Error>
where
    C: Cyd,
{
    let (mut display, _touch) = cyd.parts();
    let mut frame = display.full_frame_mut();
    frame.clear();

    match calibration_driver_state {
        CalibrationDriverState::Capturing => {
            if let Some(calibration_corner) = calibration_flow.next_corner() {
                match draw_calibration_cross(&mut frame, calibration_corner) {
                    Ok(()) => {}
                    Err(infallible) => match infallible {},
                }
            }
            match draw_calibration_instruction(&mut frame, "Tap cross, then lift") {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
        }
        CalibrationDriverState::ShowCaptured {
            calibration_corner,
            next_corner,
            ..
        } => {
            match draw_calibration_captured_cross(&mut frame, *calibration_corner) {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
            match draw_calibration_cross(&mut frame, *next_corner) {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
            match draw_calibration_instruction(&mut frame, "Corner captured") {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
        }
        CalibrationDriverState::ShowRejected {
            worst_residual_pixels,
            ..
        } => {
            if let Some(calibration_corner) = calibration_flow.next_corner() {
                match draw_calibration_rejected_cross(&mut frame, calibration_corner) {
                    Ok(()) => {}
                    Err(infallible) => match infallible {},
                }
            }
            let mut message = String::<48>::new();
            match worst_residual_pixels {
                Some(worst_residual_pixels) => {
                    match write!(&mut message, "Try again ({worst_residual_pixels:.1}px)") {
                        Ok(()) => {}
                        Err(_) => unreachable!("heapless message capacity is sufficient"),
                    }
                }
                None => match message.push_str("Try again") {
                    Ok(()) => {}
                    Err(_) => unreachable!("heapless message capacity is sufficient"),
                },
            }
            match draw_calibration_instruction(&mut frame, message.as_str()) {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
        }
        CalibrationDriverState::Verifying { .. } => {
            match draw_calibration_verify_target(&mut frame) {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
            match draw_calibration_instruction(&mut frame, "Tap center to save") {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
        }
        CalibrationDriverState::Confirming { .. } => {
            match draw_calibration_verify_target(&mut frame) {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
            match draw_calibration_instruction(&mut frame, "Calibrated") {
                Ok(()) => {}
                Err(infallible) => match infallible {},
            }
        }
    }

    frame.flush().await
}

fn hit_verify_target(mapped_x: f32, mapped_y: f32) -> bool {
    let verify_target_center = super::calibration_verify_target_center();
    let delta_x = mapped_x - verify_target_center.x as f32;
    let delta_y = mapped_y - verify_target_center.y as f32;
    delta_x * delta_x + delta_y * delta_y
        <= super::VERIFY_HIT_RADIUS_PIXELS * super::VERIFY_HIT_RADIUS_PIXELS
}
