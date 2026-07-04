//! Screen-space touch events produced by a calibrated [`Cyd`](crate::Cyd).

use embedded_graphics::geometry::Point;

/// A touch event in screen coordinates (already calibrated and mapped).
#[derive(Clone, Copy, Debug)]
pub enum TouchEvent {
    Down { point: Point },
    Move { point: Point },
    Up,
}
