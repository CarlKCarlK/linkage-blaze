//! Generic helpers for the armatron example.
//!
//! Device-agnostic calibration geometry used by the ESP32 armatron example
//! in `linkage-blaze-classic`.

use embedded_graphics::{
    Drawable,
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Primitive},
    primitives::{Circle, Line, PrimitiveStyle},
};
use linkage_blaze_core::Rgb888;

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BLACK: Rgb888 = Rgb888::new(0, 0, 0); // black
pub const WHITE: Rgb888 = Rgb888::new(255, 255, 255); // white
pub const YELLOW: Rgb888 = Rgb888::new(255, 255, 0); // yellow

// ── Calibration cross geometry ────────────────────────────────────────────────

pub const CALIBRATION_CROSS_MARGIN: i32 = 28;
pub const CALIBRATION_CROSS_HALF_SIZE: i32 = 18;
pub const CALIBRATION_CENTER_DOT_RADIUS: i32 = 3;

#[derive(Clone, Copy)]
pub enum CalibrationCorner {
    UpperLeft,
    UpperRight,
    LowerRight,
    LowerLeft,
}

pub fn calibration_corner_for_index(calibration_index: usize) -> Option<CalibrationCorner> {
    match calibration_index {
        0 => Some(CalibrationCorner::UpperLeft),
        1 => Some(CalibrationCorner::UpperRight),
        2 => Some(CalibrationCorner::LowerRight),
        3 => Some(CalibrationCorner::LowerLeft),
        _ => None,
    }
}

pub fn calibration_corner_center(
    calibration_corner: CalibrationCorner,
    width: u16,
    height: u16,
) -> Point {
    let width = width as i32;
    let height = height as i32;
    match calibration_corner {
        CalibrationCorner::UpperLeft => {
            Point::new(CALIBRATION_CROSS_MARGIN, CALIBRATION_CROSS_MARGIN)
        }
        CalibrationCorner::UpperRight => Point::new(
            width - 1 - CALIBRATION_CROSS_MARGIN,
            CALIBRATION_CROSS_MARGIN,
        ),
        CalibrationCorner::LowerRight => Point::new(
            width - 1 - CALIBRATION_CROSS_MARGIN,
            height - 1 - CALIBRATION_CROSS_MARGIN,
        ),
        CalibrationCorner::LowerLeft => Point::new(
            CALIBRATION_CROSS_MARGIN,
            height - 1 - CALIBRATION_CROSS_MARGIN,
        ),
    }
}

/// Draw a calibration crosshair with a center dot onto `target`.
pub fn draw_calibration_cross<E>(
    target: &mut impl DrawTarget<Color = Rgb565, Error = E>,
    calibration_corner: CalibrationCorner,
    width: u16,
    height: u16,
) -> Result<(), E> {
    let center = calibration_corner_center(calibration_corner, width, height);
    let left = Point::new(center.x - CALIBRATION_CROSS_HALF_SIZE, center.y);
    let right = Point::new(center.x + CALIBRATION_CROSS_HALF_SIZE, center.y);
    let top = Point::new(center.x, center.y - CALIBRATION_CROSS_HALF_SIZE);
    let bottom = Point::new(center.x, center.y + CALIBRATION_CROSS_HALF_SIZE);

    Line::new(left, right)
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::from(YELLOW), 4))
        .draw(target)?;
    Line::new(top, bottom)
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::from(YELLOW), 4))
        .draw(target)?;

    Circle::new(
        Point::new(
            center.x - CALIBRATION_CENTER_DOT_RADIUS,
            center.y - CALIBRATION_CENTER_DOT_RADIUS,
        ),
        (CALIBRATION_CENTER_DOT_RADIUS * 2 + 1) as u32,
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::from(WHITE)))
    .draw(target)?;

    Ok(())
}
