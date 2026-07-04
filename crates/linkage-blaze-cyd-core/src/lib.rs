#![no_std]

//! Platform-neutral core of the CYD device abstraction.
//!
//! See [`cyd`] for the [`Cyd`] device trait, its [`CydDisplay`] and [`CydTouch`]
//! parts, and [`CydFrame`].

pub mod calibration;
mod contiguous_pixels;
mod cyd;
mod draw_item_2d;
mod orientation;
mod tga;
pub mod tiling;
mod touch_event;

pub use calibration::{
    CALIBRATION_CENTER_DOT_RADIUS, CALIBRATION_CROSS_HALF_SIZE, CALIBRATION_CROSS_MARGIN,
    CALIBRATION_POINT_COUNT, CalibrationConfig, CalibrationCorner, CalibrationFlow,
    CalibrationSolveError, CalibrationValidation, EnsureCalibrationError, EnsureCalibrationOutcome,
    MAX_RESIDUAL_PIXELS, RawPoint, RawTouchEvent, VERIFY_HIT_RADIUS_PIXELS,
    calibration_corner_center, calibration_corner_for_index, calibration_verify_target_center,
    distort_demo_screen_to_raw, draw_calibration_ack_dot, draw_calibration_cross,
    draw_calibration_instruction, draw_calibration_rejected_cross, draw_calibration_verify_target,
    ensure_calibration, validate_calibration_points,
};
pub use contiguous_pixels::ContiguousPixels;
pub use cyd::{
    CopySizeError, Cyd, CydDisplay, CydFlushError, CydFrame, CydInfallibleError, CydRawTouch,
    CydTouch, RegionPixels, Tiles,
};
pub use draw_item_2d::{DrawItem2d, DrawItem3dExt, Image565View};
pub use orientation::Orientation;
pub use tga::{Image565Fixed, Image565Mask};
pub use touch_event::TouchEvent;

/// Native panel width in pixels (landscape). The CYD panel is fixed hardware.
pub const SCREEN_WIDTH: usize = 320;
/// Native panel height in pixels (landscape). The CYD panel is fixed hardware.
pub const SCREEN_HEIGHT: usize = 240;
/// Total panel pixel count (`SCREEN_WIDTH * SCREEN_HEIGHT`).
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
