use device_envoy_core::flash_block::FlashBlock;
use embassy_futures::yield_now;

use crate::{Cyd, CydDisplay, CydFrame, CydRawTouch};

use super::{CalibrationConfig, CalibrationFlow, draw_calibration_cross, flow::CalibrationFlowEvent};

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

/// Ensure that `cyd` has a calibration, running the shared four-tap flow when
/// the flash block does not currently deserialize as a valid configuration.
///
/// Invalid, corrupt, or absent flash content is treated as "not calibrated"
/// instead of bricking boot. The driver simply reruns the calibration flow and
/// overwrites the block with a fresh solve.
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
    // A bad or empty block should just rerun calibration at boot.
    if let Some(calibration_config) = calibration_flash_block
        .load::<CalibrationConfig>()
        .unwrap_or(None)
    {
        return Ok(EnsureCalibrationOutcome::Loaded(calibration_config));
    }

    let mut calibration_flow = CalibrationFlow::new();
    let mut redraw_requested = true;

    loop {
        if redraw_requested {
            redraw_requested = false;
            draw_calibration_screen(cyd, &calibration_flow)
                .await
                .map_err(EnsureCalibrationError::Device)?;
        }

        if recalibration_requested() {
            calibration_flow.restart();
            redraw_requested = true;
            continue;
        }

        let raw_touch_event = cyd
            .read_raw_touch_event()
            .map_err(EnsureCalibrationError::Device)?;
        let Some(calibration_flow_event) = calibration_flow.handle_raw_touch_event(raw_touch_event)
        else {
            yield_now().await;
            continue;
        };

        match calibration_flow_event {
            CalibrationFlowEvent::PointCaptured { .. } => {
                redraw_requested = true;
            }
            CalibrationFlowEvent::Completed {
                calibration_config, ..
            } => {
                calibration_flash_block
                    .save(&calibration_config)
                    .map_err(EnsureCalibrationError::Flash)?;
                return Ok(EnsureCalibrationOutcome::Saved(calibration_config));
            }
        }
    }
}

async fn draw_calibration_screen<C>(
    cyd: &mut C,
    calibration_flow: &CalibrationFlow,
) -> Result<(), C::Error>
where
    C: Cyd,
{
    let (mut display, _touch) = cyd.parts();
    let mut frame = display.full_frame_mut();
    frame.clear();
    if let Some(calibration_corner) = calibration_flow.next_corner() {
        match draw_calibration_cross(&mut frame, calibration_corner) {
            Ok(()) => {}
            Err(infallible) => match infallible {},
        }
    }
    frame.flush().await
}
