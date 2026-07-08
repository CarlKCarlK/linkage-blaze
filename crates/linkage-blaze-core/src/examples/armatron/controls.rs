//! Immediate-mode layout specs for the armatron controls.

use embedded_graphics::{
    geometry::{Point, Size},
    pixelcolor::{Rgb888, WebColors},
    primitives::Rectangle,
};

use crate::ui::{Button, Icon, IconButton, Label, Slider};

// Target selector strip: previous button, target text, next button.
pub(super) static PREVIOUS_TARGET_BUTTON: Button = Button::new(rectangle(65, 17, 42, 14), "prev");
pub(super) static NEXT_TARGET_BUTTON: Button = Button::new(rectangle(181, 17, 42, 14), "next");

pub(super) static TARGET_LABEL: Label = Label::new(point(111, 19), Rgb888::CSS_WHITE);
pub(super) static DISTANCE_LABEL: Label = Label::new(point(102, 5), Rgb888::CSS_WHITE);
pub(super) static FPS_LABEL: Label = Label::new(point(272, 229), Rgb888::CSS_LIGHT_SLATE_GRAY);

pub(super) const VERSION_TEXT: &str = concat!("v", env!("CARGO_PKG_VERSION"));
pub(super) static VERSION_LABEL: Label = Label::new(point(218, 229), Rgb888::CSS_LIGHT_SLATE_GRAY);

// Left-side camera controls: tall z tilt slider and short zoom/dolly slider.
pub(super) static TILT_SLIDER: Slider = Slider::vertical("z", 16, 24, 201, 1.0, 0.0);
pub(super) static DOLLY_SLIDER: Slider = Slider::vertical("zoom", 42, 24, 51, 0.0, 1.0);

// Bottom x/y view slider spanning under the arm display.
pub(super) static XY_VIEW_SLIDER: Slider = Slider::horizontal("x/y view", 40, 226, 213, 0.0, 1.0);

// Reverse-kinematics buttons below the left-side camera controls.
pub(super) static RK_RUN_BUTTON: IconButton =
    IconButton::new(rectangle(27, 86, 18, 18), Icon::Play);
pub(super) static RK_STOP_BUTTON: IconButton =
    IconButton::new(rectangle(27, 86, 18, 18), Icon::Stop);
pub(super) static RK_STEP_BUTTON: IconButton =
    IconButton::new(rectangle(55, 86, 18, 18), Icon::StepForward);

// Tiny calibration button in the lower-right corner.
pub(super) static CALIBRATE_BUTTON: Button = Button::new(rectangle(288, 212, 30, 14), "cal");

// Right-side parameter sliders, stacked vertically at a fixed row spacing.
const PARAM_SLIDER_TRACK_X: i32 = 230;
const PARAM_SLIDER_FIRST_TRACK_Y: i32 = 32;
const PARAM_SLIDER_STEP_Y: i32 = 32;
const PARAM_SLIDER_TRACK_LENGTH: u32 = 83;

pub(super) const PARAM_SLIDER_COUNT: usize = 6;
pub(super) const ARM_PARAM_NAMES: [&str; PARAM_SLIDER_COUNT] = [
    "raise hand",
    "bend elbow",
    "close hand",
    "lower arm",
    "spin whole arm",
    "spin hand",
];

pub(super) static PARAM_SLIDERS: [Slider; PARAM_SLIDER_COUNT] = Slider::column(
    PARAM_SLIDER_TRACK_X,
    PARAM_SLIDER_FIRST_TRACK_Y,
    PARAM_SLIDER_STEP_Y,
    PARAM_SLIDER_TRACK_LENGTH,
    ARM_PARAM_NAMES,
);

pub(super) const fn point(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

pub(super) const fn rectangle(x: i32, y: i32, width: u32, height: u32) -> Rectangle {
    Rectangle::new(point(x, y), Size::new(width, height))
}
