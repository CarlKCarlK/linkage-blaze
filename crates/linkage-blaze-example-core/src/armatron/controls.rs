use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    text::{Baseline, Text},
};
use linkage_blaze_cyd_core::TouchEvent;

use super::{CYAN, GREEN, LIGHT_SLATE_GRAY, SCREEN_WIDTH, SIM_WHITE, SIM_YELLOW};

const TILT_X: i32 = 16;
const DOLLY_X: i32 = 42;
const TILT_TOP: i32 = 24;
const TILT_BOTTOM: i32 = 224;
const DOLLY_TOP: i32 = 24;
const DOLLY_BOTTOM: i32 = 74;
const RK_CONTROL_TOP: i32 = 86;
const RK_RUN_LEFT: i32 = 27;
const RK_STEP_LEFT: i32 = 55;
const RK_BUTTON_SIZE: i32 = 18;
const SLIDER_LEFT: i32 = 230;
const SLIDER_RIGHT: i32 = 312;
const SLIDER_TRACK_LEFT: i32 = 230;
const SLIDER_TOP: i32 = 24;
const SLIDER_STEP: i32 = 32;
const VIEW_SLIDER_LEFT: i32 = 40;
const VIEW_SLIDER_RIGHT: i32 = 252;
const VIEW_SLIDER_Y: i32 = 226;
const CALIBRATE_BUTTON_LEFT: i32 = 288;
const CALIBRATE_BUTTON_TOP: i32 = 212;
const CALIBRATE_BUTTON_WIDTH: u32 = 30;
const CALIBRATE_BUTTON_HEIGHT: u32 = 14;
const CONTROL_TEXT_CHAR_WIDTH: i32 = 6;
const TARGET_CONTROL_TOP: i32 = 17;
const TARGET_BUTTON_WIDTH: u32 = 42;
const TARGET_BUTTON_HEIGHT: u32 = 14;
const TARGET_BUTTON_LABEL_WIDTH: i32 = 4 * CONTROL_TEXT_CHAR_WIDTH;
const TARGET_LABEL_WIDTH: i32 = 11 * CONTROL_TEXT_CHAR_WIDTH;
const TARGET_CONTROL_GAP: i32 = 4;
const TARGET_CONTROL_WIDTH: i32 =
    TARGET_BUTTON_WIDTH as i32 * 2 + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP * 2;
const PREV_BUTTON_LEFT: i32 = ((SCREEN_WIDTH as i32 - TARGET_CONTROL_WIDTH) / 2) - 16;
const TARGET_LABEL_LEFT: i32 = PREV_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32 + TARGET_CONTROL_GAP;
const NEXT_BUTTON_LEFT: i32 = TARGET_LABEL_LEFT + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP;

pub(super) const EXTRA_SLIDER_COUNT: usize = 6;

pub(super) struct ArmatronControlValues {
    pub(super) xy_view: f32,
    pub(super) tilt: f32,
    pub(super) dolly: f32,
    pub(super) extra_sliders: [f32; EXTRA_SLIDER_COUNT],
}

pub(super) struct ArmatronControls {
    tilt: SliderControl,
    dolly: SliderControl,
    pub(super) previous_target: TextButton,
    pub(super) next_target: TextButton,
    reverse_kinematics_run: ShapeButton,
    reverse_kinematics_step: ShapeButton,
    calibrate: TextButton,
    extra_sliders: [SliderControl; EXTRA_SLIDER_COUNT],
    xy_view: SliderControl,
    active_control: Option<ActiveControl>,
    touch_cursor: Option<(f32, f32)>,
}

impl ArmatronControls {
    pub(super) fn new(
        control_values: ArmatronControlValues,
        extra_slider_labels: [&'static str; EXTRA_SLIDER_COUNT],
    ) -> Self {
        Self {
            tilt: SliderControl::vertical(
                "z",
                Rectangle::new(Point::new(TILT_X - 14, TILT_TOP), Size::new(29, 201)),
                Point::new(TILT_X, TILT_TOP),
                Point::new(TILT_X, TILT_BOTTOM),
                control_values.tilt,
                true,
            ),
            dolly: SliderControl::vertical(
                "zoom",
                Rectangle::new(Point::new(DOLLY_X - 14, DOLLY_TOP), Size::new(29, 51)),
                Point::new(DOLLY_X, DOLLY_TOP),
                Point::new(DOLLY_X, DOLLY_BOTTOM),
                control_values.dolly,
                false,
            ),
            previous_target: TextButton::new(
                Rectangle::new(
                    Point::new(PREV_BUTTON_LEFT, TARGET_CONTROL_TOP),
                    Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
                ),
                "prev",
                Point::new(
                    PREV_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
                    TARGET_CONTROL_TOP + 2,
                ),
            ),
            next_target: TextButton::new(
                Rectangle::new(
                    Point::new(NEXT_BUTTON_LEFT, TARGET_CONTROL_TOP),
                    Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
                ),
                "next",
                Point::new(
                    NEXT_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
                    TARGET_CONTROL_TOP + 2,
                ),
            ),
            reverse_kinematics_run: ShapeButton::new(ShapeButtonKind::ReverseKinematicsRun),
            reverse_kinematics_step: ShapeButton::new(ShapeButtonKind::ReverseKinematicsStep),
            calibrate: TextButton::new(
                Rectangle::new(
                    Point::new(CALIBRATE_BUTTON_LEFT, CALIBRATE_BUTTON_TOP),
                    Size::new(CALIBRATE_BUTTON_WIDTH, CALIBRATE_BUTTON_HEIGHT),
                ),
                "cal",
                Point::new(CALIBRATE_BUTTON_LEFT + 6, CALIBRATE_BUTTON_TOP + 2),
            ),
            extra_sliders: [
                Self::slider(0, extra_slider_labels[0], control_values.extra_sliders[0]),
                Self::slider(1, extra_slider_labels[1], control_values.extra_sliders[1]),
                Self::slider(2, extra_slider_labels[2], control_values.extra_sliders[2]),
                Self::slider(3, extra_slider_labels[3], control_values.extra_sliders[3]),
                Self::slider(4, extra_slider_labels[4], control_values.extra_sliders[4]),
                Self::slider(5, extra_slider_labels[5], control_values.extra_sliders[5]),
            ],
            xy_view: SliderControl::horizontal(
                "x/y view",
                Rectangle::new(
                    Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y - 14),
                    Size::new((VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT + 1) as u32, 29),
                ),
                Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y),
                Point::new(VIEW_SLIDER_RIGHT, VIEW_SLIDER_Y),
                control_values.xy_view,
                false,
            ),
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

    pub(super) fn slider0(&self) -> f32 {
        self.extra_sliders[0].value()
    }

    pub(super) fn slider1(&self) -> f32 {
        self.extra_sliders[1].value()
    }

    pub(super) fn slider2(&self) -> f32 {
        self.extra_sliders[2].value()
    }

    pub(super) fn slider3(&self) -> f32 {
        self.extra_sliders[3].value()
    }

    pub(super) fn slider4(&self) -> f32 {
        self.extra_sliders[4].value()
    }

    pub(super) fn slider5(&self) -> f32 {
        self.extra_sliders[5].value()
    }

    pub(super) fn draw<D: DrawTarget<Color = Rgb565>>(
        &self,
        target: &mut D,
        target_label: &str,
    ) -> Result<(), D::Error> {
        self.tilt.draw(target)?;
        self.dolly.draw(target)?;
        self.reverse_kinematics_run.draw(target)?;
        self.reverse_kinematics_step.draw(target)?;
        self.calibrate.draw(target)?;
        self.previous_target.draw(target)?;
        self.next_target.draw(target)?;
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::from(SIM_WHITE));
        Text::with_baseline(
            target_label,
            Point::new(TARGET_LABEL_LEFT, TARGET_CONTROL_TOP + 2),
            text_style,
            Baseline::Top,
        )
        .draw(target)?;
        for slider in &self.extra_sliders {
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

    fn slider(slider_offset: usize, label: &'static str, value: f32) -> SliderControl {
        let slider_y = SLIDER_TOP + slider_offset as i32 * SLIDER_STEP;
        SliderControl::horizontal(
            label,
            Rectangle::new(
                Point::new(SLIDER_LEFT, slider_y - 13),
                Size::new((SCREEN_WIDTH as i32 - SLIDER_LEFT) as u32, 27),
            ),
            Point::new(SLIDER_TRACK_LEFT, slider_y + 8),
            Point::new(SLIDER_RIGHT, slider_y + 8),
            value,
            false,
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
        for (slider_offset, slider) in self.extra_sliders.iter().enumerate() {
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
                self.extra_sliders[slider_offset].set_value_from_touch(x, y);
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
    fn new(touch_rectangle: Rectangle, label: &'static str, label_position: Point) -> Self {
        Self {
            touch_rectangle,
            label,
            label_position,
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
            ShapeButtonKind::ReverseKinematicsRun => Rectangle::new(
                Point::new(RK_RUN_LEFT, RK_CONTROL_TOP),
                Size::new(RK_BUTTON_SIZE as u32, RK_BUTTON_SIZE as u32),
            ),
            ShapeButtonKind::ReverseKinematicsStep => Rectangle::new(
                Point::new(RK_STEP_LEFT, RK_CONTROL_TOP),
                Size::new(RK_BUTTON_SIZE as u32, RK_BUTTON_SIZE as u32),
            ),
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

struct SliderControl {
    label: &'static str,
    touch_rectangle: Rectangle,
    label_position: Point,
    track_start: Point,
    track_end: Point,
    orientation: SliderOrientation,
    value: f32,
    inverted: bool,
}

impl SliderControl {
    fn horizontal(
        label: &'static str,
        touch_rectangle: Rectangle,
        track_start: Point,
        track_end: Point,
        value: f32,
        inverted: bool,
    ) -> Self {
        Self {
            label,
            touch_rectangle,
            label_position: Point::new(track_start.x, track_start.y - 15),
            track_start,
            track_end,
            orientation: SliderOrientation::Horizontal,
            value,
            inverted,
        }
    }

    fn vertical(
        label: &'static str,
        touch_rectangle: Rectangle,
        track_start: Point,
        track_end: Point,
        value: f32,
        inverted: bool,
    ) -> Self {
        Self {
            label,
            touch_rectangle,
            label_position: Point::new(track_start.x - 5, 5),
            track_start,
            track_end,
            orientation: SliderOrientation::Vertical,
            value,
            inverted,
        }
    }

    fn contains(&self, touch_point: Point) -> bool {
        self.touch_rectangle.contains(touch_point)
    }

    fn value(&self) -> f32 {
        self.value
    }

    fn set_value_from_touch(&mut self, x: f32, y: f32) {
        let raw_value = match self.orientation {
            SliderOrientation::Horizontal => {
                (x - self.track_start.x as f32) / (self.track_end.x - self.track_start.x) as f32
            }
            SliderOrientation::Vertical => {
                (y - self.track_start.y as f32) / (self.track_end.y - self.track_start.y) as f32
            }
        };
        self.value = if self.inverted {
            1.0 - raw_value
        } else {
            raw_value
        }
        .clamp(0.0, 1.0);
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
        let display_value = if self.inverted {
            1.0 - self.value
        } else {
            self.value
        };
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
        Point::new(RK_RUN_LEFT, RK_CONTROL_TOP),
        Point::new(RK_RUN_LEFT, RK_CONTROL_TOP + RK_BUTTON_SIZE),
        Point::new(
            RK_RUN_LEFT + RK_BUTTON_SIZE,
            RK_CONTROL_TOP + RK_BUTTON_SIZE / 2,
        ),
    )
    .into_styled(fill_style(GREEN))
    .draw(buffer)?;
    Ok(())
}

fn draw_reverse_kinematics_step_button<D: DrawTarget<Color = Rgb565>>(
    buffer: &mut D,
) -> Result<(), D::Error> {
    Rectangle::new(
        Point::new(RK_STEP_LEFT, RK_CONTROL_TOP),
        Size::new(RK_BUTTON_SIZE as u32, RK_BUTTON_SIZE as u32),
    )
    .into_styled(stroke_style(LIGHT_SLATE_GRAY, 1))
    .draw(buffer)?;
    Rectangle::new(
        Point::new(
            RK_STEP_LEFT + RK_BUTTON_SIZE - 5,
            RK_CONTROL_TOP + RK_BUTTON_SIZE / 2 - 5,
        ),
        Size::new(2, 10),
    )
    .into_styled(fill_style(SIM_WHITE))
    .draw(buffer)?;
    Triangle::new(
        Point::new(RK_STEP_LEFT + 3, RK_CONTROL_TOP + 4),
        Point::new(RK_STEP_LEFT + 3, RK_CONTROL_TOP + RK_BUTTON_SIZE - 4),
        Point::new(
            RK_STEP_LEFT + RK_BUTTON_SIZE - 7,
            RK_CONTROL_TOP + RK_BUTTON_SIZE / 2,
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

fn round_to_i32(value: f32) -> i32 {
    libm::roundf(value) as i32
}
