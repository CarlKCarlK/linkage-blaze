#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchEvent {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up,
}

pub trait TouchInput {
    fn read_touch_event(&mut self) -> Option<TouchEvent>;
}

pub struct NullTouchInput;

impl TouchInput for NullTouchInput {
    fn read_touch_event(&mut self) -> Option<TouchEvent> {
        None
    }
}
