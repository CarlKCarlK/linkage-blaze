use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    text::{Baseline, Text},
};
use linkage_blaze_cyd_core::{SCREEN_WIDTH, TouchEvent};

use super::{CYAN, GREEN, LIGHT_SLATE_GRAY, SIM_WHITE, SIM_YELLOW};

// Target selector strip: previous button, target text, next button.
const PREVIOUS_TARGET_BUTTON_RECTANGLE: Rectangle = rectangle(65, 17, 42, 14);
pub(super) const TARGET_TEXT_POINT: Point = point(111, 19);
const NEXT_TARGET_BUTTON_RECTANGLE: Rectangle = rectangle(181, 17, 42, 14);

// Left-side camera controls: tall z tilt slider and short zoom/dolly slider.
const TILT_SLIDER_LAYOUT: SliderLayout =
    SliderLayout::new(rectangle(2, 24, 29, 201), rectangle(16, 24, 1, 201));
const DOLLY_SLIDER_LAYOUT: SliderLayout =
    SliderLayout::new(rectangle(28, 24, 29, 51), rectangle(42, 24, 1, 51));

// Bottom x/y view slider spanning under the arm display.
const VIEW_SLIDER_LAYOUT: SliderLayout =
    SliderLayout::new(rectangle(40, 212, 213, 29), rectangle(40, 226, 213, 1));

// Reverse-kinematics buttons below the left-side camera controls.
const RK_RUN_BUTTON_RECTANGLE: Rectangle = rectangle(27, 86, 18, 18);
const RK_STEP_BUTTON_RECTANGLE: Rectangle = rectangle(55, 86, 18, 18);

// Tiny calibration button in the lower-right corner.
const CALIBRATE_BUTTON_RECTANGLE: Rectangle = rectangle(288, 212, 30, 14);

// Right-side parameter sliders, stacked vertically at a fixed row spacing.
const PARAM_SLIDER_TOP: i32 = 24;
const PARAM_SLIDER_STEP_Y: i32 = 32;

pub(super) const PARAM_SLIDER_COUNT: usize = 6;

pub(super) struct ArmatronControls {
    tilt: SliderControl,
    dolly: SliderControl,
    pub(super) previous_target: TextButton,
    pub(super) next_target: TextButton,
    reverse_kinematics_run: ShapeButton,
    reverse_kinematics_step: ShapeButton,
    calibrate: TextButton,
    param_sliders: [SliderControl; PARAM_SLIDER_COUNT],
    xy_view: SliderControl,
    active_control: Option<ActiveControl>,
    touch_cursor: Option<(f32, f32)>,
}

impl ArmatronControls {
    pub(super) fn new(
        xy_view: f32,
        tilt: f32,
        dolly: f32,
        param_sliders: [f32; PARAM_SLIDER_COUNT],
        param_slider_labels: [&'static str; PARAM_SLIDER_COUNT],
    ) -> Self {
        Self {
            tilt: SliderControl::vertical("z", TILT_SLIDER_LAYOUT, tilt, 1.0, 0.0),
            dolly: SliderControl::vertical("zoom", DOLLY_SLIDER_LAYOUT, dolly, 0.0, 1.0),
            previous_target: TextButton::new(PREVIOUS_TARGET_BUTTON_RECTANGLE, "prev"),
            next_target: TextButton::new(NEXT_TARGET_BUTTON_RECTANGLE, "next"),
            reverse_kinematics_run: ShapeButton::new(ShapeButtonKind::ReverseKinematicsRun),
            reverse_kinematics_step: ShapeButton::new(ShapeButtonKind::ReverseKinematicsStep),
            calibrate: TextButton::new(CALIBRATE_BUTTON_RECTANGLE, "cal"),
            param_sliders: [
                Self::param_slider(0, param_slider_labels[0], param_sliders[0]),
                Self::param_slider(1, param_slider_labels[1], param_sliders[1]),
                Self::param_slider(2, param_slider_labels[2], param_sliders[2]),
                Self::param_slider(3, param_slider_labels[3], param_sliders[3]),
                Self::param_slider(4, param_slider_labels[4], param_sliders[4]),
                Self::param_slider(5, param_slider_labels[5], param_sliders[5]),
            ],
            xy_view: SliderControl::horizontal("x/y view", VIEW_SLIDER_LAYOUT, xy_view, 0.0, 1.0),
            active_control: None,
            touch_cursor: None,
        }
    }

    pub(super) fn set_event(&mut self, touch_event: Option<TouchEvent>) {
        self.begin_frame();
        match touch_event {
            Some(TouchEvent::Down { x, y }) => self.handle_touch_down(x, y),
            Some(TouchEvent::Move { x, y }) => self.handle_touch_move(x, y),
            Some(TouchEvent::Up) => self.handle_touch_up(),
            None => {}
        }
    }

    pub(super) fn tilt(&self) -> f32 {
        self.tilt.value()
    }

    pub(super) fn dolly(&self) -> f32 {
        self.dolly.value()
    }

    pub(super) fn xy_view(&self) -> f32 {
        self.xy_view.value()
    }

    pub(super) fn param_sliders(&self) -> [f32; PARAM_SLIDER_COUNT] {
        self.param_sliders.each_ref().map(SliderControl::value)
    }

    pub(super) fn draw<D: DrawTarget<Color = Rgb565>>(
        &self,
        target: &mut D,
    ) -> Result<(), D::Error> {
        self.tilt.draw(target)?;
        self.dolly.draw(target)?;
        self.reverse_kinematics_run.draw(target)?;
        self.reverse_kinematics_step.draw(target)?;
        self.calibrate.draw(target)?;
        self.previous_target.draw(target)?;
        self.next_target.draw(target)?;
        for slider in &self.param_sliders {
            slider.draw(target)?;
        }
        self.xy_view.draw(target)?;
        Ok(())
    }

    pub(super) fn draw_touch_cursor<D: DrawTarget<Color = Rgb565>>(
        &self,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let Some((x, y)) = self.touch_cursor else {
            return Ok(());
        };
        let x = x as i32;
        let y = y as i32;
        let radius = 5;
        Circle::new(Point::new(x - radius, y - radius), (radius * 2 + 1) as u32)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::from(CYAN)))
            .draw(target)?;
        Ok(())
    }

    fn param_slider(slider_offset: usize, label: &'static str, value: f32) -> SliderControl {
        let slider_y = PARAM_SLIDER_TOP + slider_offset as i32 * PARAM_SLIDER_STEP_Y;
        SliderControl::horizontal(
            label,
            SliderLayout::new(
                rectangle(230, slider_y - 13, (SCREEN_WIDTH - 230) as u32, 27),
                rectangle(230, slider_y + 8, 83, 1),
            ),
            value,
            0.0,
            1.0,
        )
    }

    fn begin_frame(&mut self) {
        self.previous_target.begin_frame();
        self.next_target.begin_frame();
        self.reverse_kinematics_run.begin_frame();
        self.reverse_kinematics_step.begin_frame();
        self.calibrate.begin_frame();
    }

    fn handle_touch_down(&mut self, x: f32, y: f32) {
        self.touch_cursor = Some((x, y));
        self.active_control = self.control_at(Point::new(x as i32, y as i32));

        match self.active_control {
            Some(ActiveControl::Tilt)
            | Some(ActiveControl::Dolly)
            | Some(ActiveControl::XyView)
            | Some(ActiveControl::Slider(_)) => self.update_active_slider(x, y),
            None => {
                let touch_point = Point::new(x as i32, y as i32);
                self.previous_target.handle_touch_down(touch_point);
                self.next_target.handle_touch_down(touch_point);
                self.reverse_kinematics_run.handle_touch_down(touch_point);
                self.reverse_kinematics_step.handle_touch_down(touch_point);
                self.calibrate.handle_touch_down(touch_point);
            }
        }
    }

    fn handle_touch_move(&mut self, x: f32, y: f32) {
        self.touch_cursor = Some((x, y));
        self.update_active_slider(x, y);
    }

    fn handle_touch_up(&mut self) {
        self.touch_cursor = None;
        self.active_control = None;
        self.previous_target.handle_touch_up();
        self.next_target.handle_touch_up();
        self.reverse_kinematics_run.handle_touch_up();
        self.reverse_kinematics_step.handle_touch_up();
        self.calibrate.handle_touch_up();
    }

    fn control_at(&self, touch_point: Point) -> Option<ActiveControl> {
        if self.tilt.contains(touch_point) {
            return Some(ActiveControl::Tilt);
        }
        if self.dolly.contains(touch_point) {
            return Some(ActiveControl::Dolly);
        }
        if self.xy_view.contains(touch_point) {
            return Some(ActiveControl::XyView);
        }
        for (slider_offset, slider) in self.param_sliders.iter().enumerate() {
            if slider.contains(touch_point) {
                return Some(ActiveControl::Slider(slider_offset));
            }
        }
        None
    }

    fn update_active_slider(&mut self, x: f32, y: f32) {
        match self.active_control {
            Some(ActiveControl::Tilt) => self.tilt.set_value_from_touch(x, y),
            Some(ActiveControl::Dolly) => self.dolly.set_value_from_touch(x, y),
            Some(ActiveControl::XyView) => self.xy_view.set_value_from_touch(x, y),
            Some(ActiveControl::Slider(slider_offset)) => {
                self.param_sliders[slider_offset].set_value_from_touch(x, y);
            }
            None => {}
        }
    }
}

#[derive(Clone, Copy)]
enum ActiveControl {
    Slider(usize),
    Tilt,
    Dolly,
    XyView,
}

pub(super) struct TextButton {
    touch_rectangle: Rectangle,
    label: &'static str,
    label_position: Point,
    is_pressed: bool,
    was_clicked: bool,
}

impl TextButton {
    fn new(touch_rectangle: Rectangle, label: &'static str) -> Self {
        Self {
            touch_rectangle,
            label,
            label_position: centered_text_position(touch_rectangle, label),
            is_pressed: false,
            was_clicked: false,
        }
    }

    fn begin_frame(&mut self) {
        self.was_clicked = false;
    }

    fn handle_touch_down(&mut self, touch_point: Point) {
        if self.touch_rectangle.contains(touch_point) {
            self.is_pressed = true;
            self.was_clicked = true;
        }
    }

    fn handle_touch_up(&mut self) {
        self.is_pressed = false;
    }

    pub(super) fn was_clicked(&self) -> bool {
        self.was_clicked
    }

    fn draw<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::from(SIM_WHITE));
        self.touch_rectangle
            .into_styled(stroke_style(LIGHT_SLATE_GRAY, 1))
            .draw(target)?;
        Text::with_baseline(self.label, self.label_position, text_style, Baseline::Top)
            .draw(target)?;
        Ok(())
    }
}

struct ShapeButton {
    kind: ShapeButtonKind,
    is_pressed: bool,
    was_clicked: bool,
}

impl ShapeButton {
    fn new(kind: ShapeButtonKind) -> Self {
        Self {
            kind,
            is_pressed: false,
            was_clicked: false,
        }
    }

    fn begin_frame(&mut self) {
        self.was_clicked = false;
    }

    fn handle_touch_down(&mut self, touch_point: Point) {
        if self.touch_rectangle().contains(touch_point) {
            self.is_pressed = true;
            self.was_clicked = true;
        }
    }

    fn handle_touch_up(&mut self) {
        self.is_pressed = false;
    }

    fn touch_rectangle(&self) -> Rectangle {
        match self.kind {
            ShapeButtonKind::ReverseKinematicsRun => RK_RUN_BUTTON_RECTANGLE,
            ShapeButtonKind::ReverseKinematicsStep => RK_STEP_BUTTON_RECTANGLE,
        }
    }

    fn draw<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
        match self.kind {
            ShapeButtonKind::ReverseKinematicsRun => draw_reverse_kinematics_run_button(target),
            ShapeButtonKind::ReverseKinematicsStep => draw_reverse_kinematics_step_button(target),
        }
    }
}

#[derive(Clone, Copy)]
enum ShapeButtonKind {
    ReverseKinematicsRun,
    ReverseKinematicsStep,
}

#[derive(Clone, Copy)]
struct SliderLayout {
    // Wider hit target for touch input.
    touch_rectangle: Rectangle,
    // Thin visual track used for drawing and value interpolation.
    track_rectangle: Rectangle,
}

impl SliderLayout {
    const fn new(touch_rectangle: Rectangle, track_rectangle: Rectangle) -> Self {
        Self {
            touch_rectangle,
            track_rectangle,
        }
    }

    fn track_start(self) -> Point {
        self.track_rectangle.top_left
    }

    fn horizontal_track_end(self) -> Point {
        Point::new(
            self.track_rectangle.top_left.x + self.track_rectangle.size.width as i32 - 1,
            self.track_rectangle.top_left.y,
        )
    }

    fn vertical_track_end(self) -> Point {
        Point::new(
            self.track_rectangle.top_left.x,
            self.track_rectangle.top_left.y + self.track_rectangle.size.height as i32 - 1,
        )
    }
}

struct SliderControl {
    label: &'static str,
    touch_rectangle: Rectangle,
    label_position: Point,
    track_start: Point,
    track_end: Point,
    orientation: SliderOrientation,
    value: f32,
    start: f32,
    last: f32,
}

impl SliderControl {
    fn horizontal(
        label: &'static str,
        slider_layout: SliderLayout,
        value: f32,
        start: f32,
        last: f32,
    ) -> Self {
        let track_start = slider_layout.track_start();
        Self {
            label,
            touch_rectangle: slider_layout.touch_rectangle,
            label_position: Point::new(track_start.x, track_start.y - 15),
            track_start,
            track_end: slider_layout.horizontal_track_end(),
            orientation: SliderOrientation::Horizontal,
            value,
            start,
            last,
        }
    }

    fn vertical(
        label: &'static str,
        slider_layout: SliderLayout,
        value: f32,
        start: f32,
        last: f32,
    ) -> Self {
        let track_start = slider_layout.track_start();
        Self {
            label,
            touch_rectangle: slider_layout.touch_rectangle,
            label_position: Point::new(track_start.x - 5, 5),
            track_start,
            track_end: slider_layout.vertical_track_end(),
            orientation: SliderOrientation::Vertical,
            value,
            start,
            last,
        }
    }

    fn contains(&self, touch_point: Point) -> bool {
        self.touch_rectangle.contains(touch_point)
    }

    fn value(&self) -> f32 {
        self.value
    }

    fn set_value_from_touch(&mut self, x: f32, y: f32) {
        let slider_position = match self.orientation {
            SliderOrientation::Horizontal => {
                (x - self.track_start.x as f32) / (self.track_end.x - self.track_start.x) as f32
            }
            SliderOrientation::Vertical => {
                (y - self.track_start.y as f32) / (self.track_end.y - self.track_start.y) as f32
            }
        };
        let slider_position = slider_position.clamp(0.0, 1.0);
        self.value = self.start + (self.last - self.start) * slider_position;
    }

    fn draw<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::from(SIM_WHITE));
        Text::with_baseline(self.label, self.label_position, text_style, Baseline::Top)
            .draw(target)?;
        Line::new(self.track_start, self.track_end)
            .into_styled(stroke_style(LIGHT_SLATE_GRAY, 2))
            .draw(target)?;
        Circle::with_center(self.knob_center(), 9)
            .into_styled(fill_style(SIM_YELLOW))
            .draw(target)?;
        Ok(())
    }

    fn knob_center(&self) -> Point {
        let display_value = (self.value - self.start) / (self.last - self.start);
        match self.orientation {
            SliderOrientation::Horizontal => Point::new(
                self.track_start.x
                    + round_to_i32((self.track_end.x - self.track_start.x) as f32 * display_value),
                self.track_start.y,
            ),
            SliderOrientation::Vertical => Point::new(
                self.track_start.x,
                self.track_start.y
                    + round_to_i32((self.track_end.y - self.track_start.y) as f32 * display_value),
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum SliderOrientation {
    Horizontal,
    Vertical,
}

fn draw_reverse_kinematics_run_button<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
) -> Result<(), D::Error> {
    Triangle::new(
        RK_RUN_BUTTON_RECTANGLE.top_left,
        Point::new(
            RK_RUN_BUTTON_RECTANGLE.top_left.x,
            RK_RUN_BUTTON_RECTANGLE.top_left.y + RK_RUN_BUTTON_RECTANGLE.size.height as i32,
        ),
        Point::new(
            RK_RUN_BUTTON_RECTANGLE.top_left.x + RK_RUN_BUTTON_RECTANGLE.size.width as i32,
            RK_RUN_BUTTON_RECTANGLE.top_left.y + RK_RUN_BUTTON_RECTANGLE.size.height as i32 / 2,
        ),
    )
    .into_styled(fill_style(GREEN))
    .draw(buffer)?;
    Ok(())
}

fn draw_reverse_kinematics_step_button<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
) -> Result<(), D::Error> {
    RK_STEP_BUTTON_RECTANGLE
        .into_styled(stroke_style(LIGHT_SLATE_GRAY, 1))
        .draw(buffer)?;
    Rectangle::new(
        Point::new(
            RK_STEP_BUTTON_RECTANGLE.top_left.x + RK_STEP_BUTTON_RECTANGLE.size.width as i32 - 5,
            RK_STEP_BUTTON_RECTANGLE.top_left.y + RK_STEP_BUTTON_RECTANGLE.size.height as i32 / 2
                - 5,
        ),
        Size::new(2, 10),
    )
    .into_styled(fill_style(SIM_WHITE))
    .draw(buffer)?;
    Triangle::new(
        Point::new(
            RK_STEP_BUTTON_RECTANGLE.top_left.x + 3,
            RK_STEP_BUTTON_RECTANGLE.top_left.y + 4,
        ),
        Point::new(
            RK_STEP_BUTTON_RECTANGLE.top_left.x + 3,
            RK_STEP_BUTTON_RECTANGLE.top_left.y + RK_STEP_BUTTON_RECTANGLE.size.height as i32 - 4,
        ),
        Point::new(
            RK_STEP_BUTTON_RECTANGLE.top_left.x + RK_STEP_BUTTON_RECTANGLE.size.width as i32 - 7,
            RK_STEP_BUTTON_RECTANGLE.top_left.y + RK_STEP_BUTTON_RECTANGLE.size.height as i32 / 2,
        ),
    )
    .into_styled(fill_style(GREEN))
    .draw(buffer)?;
    Ok(())
}

fn fill_style(color: super::Rgb888) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_fill(Rgb565::from(color))
}

fn stroke_style(color: super::Rgb888, stroke_width: u32) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_stroke(Rgb565::from(color), stroke_width)
}

fn centered_text_position(touch_rectangle: Rectangle, label: &str) -> Point {
    let label_width = label.len() as i32 * 6;
    Point::new(
        touch_rectangle.top_left.x + (touch_rectangle.size.width as i32 - label_width) / 2,
        touch_rectangle.top_left.y + 2,
    )
}

const fn point(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

const fn rectangle(x: i32, y: i32, width: u32, height: u32) -> Rectangle {
    Rectangle::new(point(x, y), Size::new(width, height))
}

fn round_to_i32(value: f32) -> i32 {
    libm::roundf(value) as i32
}
