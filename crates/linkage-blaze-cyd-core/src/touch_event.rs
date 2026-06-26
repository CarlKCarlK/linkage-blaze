//! Screen-space touch events produced by a calibrated [`Cyd`](crate::Cyd).

/// A touch event in screen coordinates (already calibrated and mapped).
#[derive(Clone, Copy, Debug)]
pub enum TouchInputEvent {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up,
}
