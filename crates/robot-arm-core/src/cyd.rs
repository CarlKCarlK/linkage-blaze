use core::{convert::Infallible, f32::consts::TAU};

use embassy_time::Instant;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    text::{Baseline, Text},
};
use nanorand::{Rng, WyRand};
use static_cell::StaticCell;

use crate::{Linkage, Pose, Vec3};

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 240;
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

const HORIZONTAL_MIN: f32 = -8.0;
const HORIZONTAL_MAX: f32 = 8.0;
const Z_MIN: f32 = 0.0;
const Z_MAX: f32 = 10.0;
const TILT_X: i32 = 16;
const ZOOM_X: i32 = 42;
const TILT_TOP: i32 = 24;
const TILT_BOTTOM: i32 = 224;
const ZOOM_TOP: i32 = 24;
const ZOOM_BOTTOM: i32 = 74;
const RK_CONTROL_TOP: i32 = 86;
const RK_RUN_LEFT: i32 = 27;
const RK_STEP_LEFT: i32 = 55;
const RK_BUTTON_SIZE: i32 = 18;
const SLIDER_LEFT: i32 = 230;
const SLIDER_RIGHT: i32 = 312;
const SLIDER_TRACK_LEFT: i32 = 230;
const SLIDER_TOP: i32 = 24;
const SLIDER_STEP: i32 = 32;
const SLIDER_COUNT: usize = 6;
const VIEW_SLIDER_LEFT: i32 = 40;
const VIEW_SLIDER_RIGHT: i32 = 252;
const VIEW_SLIDER_Y: i32 = 226;
const CALIBRATE_BUTTON_LEFT: i32 = 288;
const CALIBRATE_BUTTON_TOP: i32 = 212;
const CALIBRATE_BUTTON_WIDTH: u32 = 30;
const CALIBRATE_BUTTON_HEIGHT: u32 = 14;
const TEXT_CHAR_WIDTH: i32 = 6;
const DISTANCE_REPORT_WIDTH: i32 = 14 * TEXT_CHAR_WIDTH;
const DISTANCE_REPORT_LEFT: i32 = ((SCREEN_WIDTH as i32 - DISTANCE_REPORT_WIDTH) / 2) - 16;
const FPS_REPORT_WIDTH: i32 = 7 * TEXT_CHAR_WIDTH;
const FPS_REPORT_LEFT: i32 = SCREEN_WIDTH as i32 - FPS_REPORT_WIDTH;
const FPS_REPORT_TOP: i32 = SCREEN_HEIGHT as i32 - 11;
const VERSION_TEXT: &str = concat!("v", env!("CARGO_PKG_VERSION"));
const VERSION_REPORT_LEFT: i32 =
    FPS_REPORT_LEFT - (VERSION_TEXT.len() as i32 * TEXT_CHAR_WIDTH) - TEXT_CHAR_WIDTH;
const VERSION_REPORT_TOP: i32 = FPS_REPORT_TOP;
const TARGET_CONTROL_TOP: i32 = 17;
const TARGET_BUTTON_WIDTH: u32 = 42;
const TARGET_BUTTON_HEIGHT: u32 = 14;
const TARGET_BUTTON_LABEL_WIDTH: i32 = 4 * TEXT_CHAR_WIDTH;
const TARGET_LABEL_WIDTH: i32 = 11 * TEXT_CHAR_WIDTH;
const TARGET_CONTROL_GAP: i32 = 4;
const TARGET_CONTROL_WIDTH: i32 =
    TARGET_BUTTON_WIDTH as i32 * 2 + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP * 2;
const PREV_BUTTON_LEFT: i32 = ((SCREEN_WIDTH as i32 - TARGET_CONTROL_WIDTH) / 2) - 16;
const TARGET_LABEL_LEFT: i32 = PREV_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32 + TARGET_CONTROL_GAP;
const NEXT_BUTTON_LEFT: i32 = TARGET_LABEL_LEFT + TARGET_LABEL_WIDTH + TARGET_CONTROL_GAP;
const TARGET_MIN_DIAMETER: f32 = 0.1;
const TARGET_MAX_DIAMETER: f32 = 0.9;
const HAND_CORNER_POSE_INDICES: [usize; 4] = [15, 17, 21, 23];
const PARAM_BEND_ELBOW: usize = 1;
const PARAM_SPIN_WHOLE_ARM: usize = 4;
const RK_INITIAL_STEP: f32 = 0.125;
const RK_MIN_STEP: f32 = 0.001;
const RK_VISIBLE_PARAM_POINTS_PER_SECOND: f32 = 0.6;
const RK_MAX_TICK_SECONDS: f32 = 0.1;
const RK_SINGLE_STEP_VISIBLE_PARAM_STEP: f32 = 0.01;
const RK_SEARCH_CANDIDATES_PER_TICK: usize = 4;
const RK_PAIRED_CANDIDATES: [(f32, f32); 4] = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)];
const RK_CANDIDATE_COUNT: usize = SLIDER_COUNT + RK_PAIRED_CANDIDATES.len();
const ARM_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::CSS_CYAN);
const TARGET_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::RED);
const SLIDER_TRACK_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 2);
const BUTTON_STROKE_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 1);
const YELLOW_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW);
const PLAY_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::GREEN);
const STOP_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::WHITE);

const PARAM_NAMES: [&str; SLIDER_COUNT] = [
    "lower hand",
    "bend elbow",
    "close hand",
    "lower arm",
    "spin whole",
    "spin hand",
];

const LINKAGE: Linkage<6, 24> = Linkage::start()
    .yaw(90.0)
    .yaw_param(4, 360.0, -360.0) // spin whole arm
    .pitch(90.0)
    .forward(2.5)
    .pitch(-90.0)
    .pitch_param(3, 30.0, 0.0) // lower arm
    .forward(3.0)
    .yaw_param(1, 90.0, -90.0) // bend elbow
    .forward(3.0)
    .pitch_param(0, 90.0, -90.0) // lower hand
    .forward(1.0)
    .roll_param(5, 360.0, -360.0) // spin hand
    .forward(0.5)
    .yaw(90.0)
    .move_param(2, 0.5, 0.0) // close hand
    .yaw(-90.0)
    .forward(1.0)
    .yaw(180.0)
    .forward(1.0)
    .yaw(90.0)
    .move_param(2, 1.0, 0.0) // close hand
    .yaw(90.0)
    .forward(1.0);

pub struct CydSim {
    params: [f32; 6],
    xy_mix: f32,
    z_mix: f32,
    zoom: f32,
    target_seed: u8,
    active_control: Option<ActiveControl>,
    reverse_kinematics_run: Option<ReverseKinematicsRun>,
    reverse_kinematics_playing: bool,
    previous_tick: Option<Instant>,
    show_fps: bool,
    fps: Option<u32>,
    calibration_requested: bool,
    rk_step_hold_active: bool,
    touch_cursor: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug)]
pub enum TouchInputEvent {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TouchInputOutcome {
    Unchanged,
    Changed,
    CalibrationRequested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TickOut {
    Calibrate,
    Draw,
    Nada,
}

impl CydSim {
    pub const WIDTH: usize = SCREEN_WIDTH;
    pub const HEIGHT: usize = SCREEN_HEIGHT;
    pub const WIDTH_U16: u16 = Self::WIDTH as u16;
    pub const HEIGHT_U16: u16 = Self::HEIGHT as u16;

    #[must_use]
    pub fn new() -> Self {
        Self::new_inner(false)
    }

    #[must_use]
    pub fn new_with_fps() -> Self {
        Self::new_inner(true)
    }

    fn new_inner(show_fps: bool) -> Self {
        Self {
            params: [0.5, 0.5, 0.0, 0.5, 0.5, 0.5],
            xy_mix: 0.5 + 30.0 / 360.0,
            z_mix: 0.3,
            zoom: 0.5,
            target_seed: 0,
            active_control: None,
            reverse_kinematics_run: None,
            reverse_kinematics_playing: false,
            previous_tick: None,
            show_fps,
            fps: None,
            calibration_requested: false,
            rk_step_hold_active: false,
            touch_cursor: None,
        }
    }

    #[must_use]
    pub const fn width(&self) -> usize {
        Self::WIDTH
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        Self::HEIGHT
    }

    pub fn take_calibration_request(&mut self) -> bool {
        let calibration_requested = self.calibration_requested;
        self.calibration_requested = false;
        calibration_requested
    }

    #[must_use]
    pub fn touch_cursor(&self) -> Option<(f32, f32)> {
        self.touch_cursor
    }

    fn touch_down(&mut self, x: f32, y: f32) {
        self.active_control = control_at(x, y);
        if matches!(self.active_control, Some(ActiveControl::Calibrate)) {
            self.calibration_requested = true;
            self.active_control = None;
            return;
        }
        if matches!(self.active_control, Some(ActiveControl::PreviousTarget)) {
            self.clear_reverse_kinematics();
            self.target_seed = self.target_seed.wrapping_sub(1);
            self.active_control = None;
            return;
        }
        if matches!(self.active_control, Some(ActiveControl::NextTarget)) {
            self.clear_reverse_kinematics();
            self.target_seed = self.target_seed.wrapping_add(1);
            self.active_control = None;
            return;
        }
        if matches!(
            self.active_control,
            Some(ActiveControl::ToggleReverseKinematics)
        ) {
            self.toggle_reverse_kinematics();
            self.active_control = None;
            return;
        }
        if matches!(
            self.active_control,
            Some(ActiveControl::StepReverseKinematics)
        ) {
            self.rk_step_hold_active = true;
            self.step_reverse_kinematics();
            return;
        }
        self.update_touch(x, y);
    }

    fn touch_move(&mut self, x: f32, y: f32) {
        self.update_touch(x, y);
    }

    fn touch_up(&mut self) {
        self.active_control = None;
        self.rk_step_hold_active = false;
    }

    pub fn handle_touch_input_event(
        &mut self,
        touch_input_event: TouchInputEvent,
    ) -> TouchInputOutcome {
        match touch_input_event {
            TouchInputEvent::Down { x, y } => {
                self.touch_cursor = Some((x, y));
                self.touch_down(x, y);
                if self.take_calibration_request() {
                    self.touch_up();
                    self.touch_cursor = None;
                    TouchInputOutcome::CalibrationRequested
                } else {
                    TouchInputOutcome::Changed
                }
            }
            TouchInputEvent::Move { x, y } => {
                self.touch_cursor = Some((x, y));
                self.touch_move(x, y);
                TouchInputOutcome::Changed
            }
            TouchInputEvent::Up => {
                self.touch_cursor = None;
                self.touch_up();
                TouchInputOutcome::Changed
            }
        }
    }

    pub fn tick(&mut self, now: Instant, touch_input_event: Option<TouchInputEvent>) -> TickOut {
        let previous_tick = self.previous_tick;
        let first_tick = previous_tick.is_none();
        let reverse_kinematics_changed = self.tick_reverse_kinematics_at(now);
        let fps_draw_requested = self.update_fps(previous_tick, now);
        let touch_input_outcome = touch_input_event.map_or(TouchInputOutcome::Unchanged, |event| {
            self.handle_touch_input_event(event)
        });

        match touch_input_outcome {
            TouchInputOutcome::CalibrationRequested => {
                self.previous_tick = None;
                self.fps = None;
                TickOut::Calibrate
            }
            TouchInputOutcome::Changed
                if first_tick || reverse_kinematics_changed || fps_draw_requested =>
            {
                TickOut::Draw
            }
            TouchInputOutcome::Changed => TickOut::Draw,
            TouchInputOutcome::Unchanged
                if first_tick || reverse_kinematics_changed || fps_draw_requested =>
            {
                TickOut::Draw
            }
            TouchInputOutcome::Unchanged => TickOut::Nada,
        }
    }

    pub fn start_reverse_kinematics(&mut self) {
        self.ensure_reverse_kinematics_run();
        self.reverse_kinematics_playing = true;
        self.previous_tick = None;
    }

    pub fn stop_reverse_kinematics(&mut self) {
        self.reverse_kinematics_playing = false;
        self.previous_tick = None;
    }

    fn clear_reverse_kinematics(&mut self) {
        self.reverse_kinematics_run = None;
        self.reverse_kinematics_playing = false;
        self.previous_tick = None;
    }

    fn ensure_reverse_kinematics_run(&mut self) {
        if self.reverse_kinematics_run.is_none() {
            self.reverse_kinematics_run = Some(ReverseKinematicsRun::new(
                &self.params,
                target_from_seed(self.target_seed),
            ));
        }
    }

    pub fn toggle_reverse_kinematics(&mut self) {
        if self.is_reverse_kinematics_running() {
            self.stop_reverse_kinematics();
        } else {
            self.start_reverse_kinematics();
        }
    }

    #[must_use]
    pub const fn is_reverse_kinematics_running(&self) -> bool {
        self.reverse_kinematics_playing
    }

    pub fn tick_reverse_kinematics_at(&mut self, now: Instant) -> bool {
        let dt_seconds = self.previous_tick.map_or(0.0, |previous_tick| {
            now.saturating_duration_since(previous_tick).as_micros() as f32 / 1_000_000.0
        });
        self.previous_tick = Some(now);
        self.tick_reverse_kinematics(dt_seconds)
    }

    fn update_fps(&mut self, previous_tick: Option<Instant>, now: Instant) -> bool {
        if !self.show_fps {
            return false;
        }

        let Some(previous_tick) = previous_tick else {
            return false;
        };
        let frame_micros = now.saturating_duration_since(previous_tick).as_micros();
        if frame_micros == 0 {
            return false;
        }

        let fps = (1_000_000 / frame_micros).min(999) as u32;
        self.fps = Some(fps);
        true
    }

    fn tick_reverse_kinematics(&mut self, dt_seconds: f32) -> bool {
        if !self.reverse_kinematics_playing && !self.rk_step_hold_active {
            return false;
        }

        let Some(mut run) = self.reverse_kinematics_run.take() else {
            self.reverse_kinematics_playing = false;
            return false;
        };

        let mut search_running = false;
        for _ in 0..RK_SEARCH_CANDIDATES_PER_TICK {
            if !run.tick_search_candidate() {
                break;
            }
            search_running = true;
        }
        let visible_moving = move_params_toward(
            &mut self.params,
            &run.best_params,
            reverse_kinematics_visible_param_step(dt_seconds),
        );
        let running = search_running || visible_moving;
        if running {
            self.reverse_kinematics_run = Some(run);
        } else {
            self.reverse_kinematics_playing = false;
        }
        running
    }

    pub fn step_reverse_kinematics(&mut self) -> bool {
        self.ensure_reverse_kinematics_run();
        self.reverse_kinematics_playing = false;

        let Some(mut run) = self.reverse_kinematics_run.take() else {
            return false;
        };

        let search_running = run.tick_search_candidate();
        let visible_moving = move_params_toward(
            &mut self.params,
            &run.best_params,
            RK_SINGLE_STEP_VISIBLE_PARAM_STEP,
        );
        let running = search_running || visible_moving;
        if running {
            self.reverse_kinematics_run = Some(run);
        }
        running
    }

    /// Run a no-allocation local reverse-kinematics search over the six robot parameters.
    ///
    /// This tries each parameter in both directions, keeps improvements, and
    /// shrinks the step when stuck.
    pub fn reverse_kinematics(&mut self) -> f32 {
        let target = target_from_seed(self.target_seed);
        let distance = reverse_kinematics(&mut self.params, target);
        distance
    }

    fn update_touch(&mut self, x: f32, y: f32) {
        let Some(active_control) = self.active_control else {
            return;
        };

        match active_control {
            ActiveControl::RightSlider(slider_index) => {
                self.clear_reverse_kinematics();
                let value = ((x - SLIDER_TRACK_LEFT as f32)
                    / (SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32)
                    .clamp(0.0, 1.0);
                self.params[slider_index] = value;
            }
            ActiveControl::Tilt => {
                self.z_mix =
                    (1.0 - (y - TILT_TOP as f32) / (TILT_BOTTOM - TILT_TOP) as f32).clamp(0.0, 1.0);
            }
            ActiveControl::Zoom => {
                self.zoom =
                    (1.0 - (y - ZOOM_TOP as f32) / (ZOOM_BOTTOM - ZOOM_TOP) as f32).clamp(0.0, 1.0);
            }
            ActiveControl::XyView => {
                self.xy_mix = ((x - VIEW_SLIDER_LEFT as f32)
                    / (VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT) as f32)
                    .clamp(0.0, 1.0);
            }
            ActiveControl::PreviousTarget => {}
            ActiveControl::NextTarget => {}
            ActiveControl::ToggleReverseKinematics => {}
            ActiveControl::StepReverseKinematics => {}
            ActiveControl::Calibrate => {}
        }
    }

    fn draw_grid(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let style = grid_stroke_style(self.zoom);
        for grid in -4..=4 {
            let grid = grid as f32;
            Line::new(
                self.world_to_screen(grid, -4.0, 0.0),
                self.world_to_screen(grid, 4.0, 0.0),
            )
            .into_styled(style)
            .draw(buffer)
            .ok();
            Line::new(
                self.world_to_screen(-4.0, grid, 0.0),
                self.world_to_screen(4.0, grid, 0.0),
            )
            .into_styled(style)
            .draw(buffer)
            .ok();
        }
    }

    fn draw_arm(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let rod_width = zoomed_pixels(3, self.zoom);
        let joint_diameter = zoomed_pixels(7, self.zoom);
        let mut previous: Option<Point> = None;
        for pose in LINKAGE.poses(&self.params) {
            let point = self.pose_to_screen(pose);
            if let Some(previous_point) = previous {
                Line::new(previous_point, point)
                    .into_styled(arm_stroke_style(rod_width))
                    .draw(buffer)
                    .ok();
            }
            Circle::with_center(point, joint_diameter)
                .into_styled(ARM_FILL_STYLE)
                .draw(buffer)
                .ok();
            previous = Some(point);
        }
    }

    fn draw_target(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let target = target_from_seed(self.target_seed);
        let Vec3([x, y, z]) = target.center;
        let diameter = world_diameter_to_screen(target.diameter, self.zoom);

        Circle::with_center(self.world_to_screen(x, y, z), diameter)
            .into_styled(TARGET_FILL_STYLE)
            .draw(buffer)
            .ok();
    }

    fn draw_sliders(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        let mut target_label = TargetLabel::new();
        Text::with_baseline("z", Point::new(11, 5), text_style, Baseline::Top)
            .draw(buffer)
            .ok();
        Line::new(
            Point::new(TILT_X, TILT_TOP),
            Point::new(TILT_X, TILT_BOTTOM),
        )
        .into_styled(SLIDER_TRACK_STYLE)
        .draw(buffer)
        .ok();
        let tilt_knob_y =
            TILT_TOP + round_to_i32((TILT_BOTTOM - TILT_TOP) as f32 * (1.0 - self.z_mix));
        Circle::with_center(Point::new(TILT_X, tilt_knob_y), 9)
            .into_styled(YELLOW_FILL_STYLE)
            .draw(buffer)
            .ok();

        Text::with_baseline("zoom", Point::new(29, 5), text_style, Baseline::Top)
            .draw(buffer)
            .ok();
        Line::new(
            Point::new(ZOOM_X, ZOOM_TOP),
            Point::new(ZOOM_X, ZOOM_BOTTOM),
        )
        .into_styled(SLIDER_TRACK_STYLE)
        .draw(buffer)
        .ok();
        let zoom_knob_y =
            ZOOM_TOP + round_to_i32((ZOOM_BOTTOM - ZOOM_TOP) as f32 * (1.0 - self.zoom));
        Circle::with_center(Point::new(ZOOM_X, zoom_knob_y), 9)
            .into_styled(YELLOW_FILL_STYLE)
            .draw(buffer)
            .ok();

        self.draw_reverse_kinematics_run_button(buffer);
        self.draw_reverse_kinematics_step_button(buffer);
        self.draw_calibrate_button(buffer);

        Rectangle::new(
            Point::new(PREV_BUTTON_LEFT, TARGET_CONTROL_TOP),
            Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
        )
        .into_styled(BUTTON_STROKE_STYLE)
        .draw(buffer)
        .ok();
        Text::with_baseline(
            "prev",
            Point::new(
                PREV_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
                TARGET_CONTROL_TOP + 2,
            ),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();
        Text::with_baseline(
            target_label.as_str(self.target_seed),
            Point::new(TARGET_LABEL_LEFT, TARGET_CONTROL_TOP + 2),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();
        Rectangle::new(
            Point::new(NEXT_BUTTON_LEFT, TARGET_CONTROL_TOP),
            Size::new(TARGET_BUTTON_WIDTH, TARGET_BUTTON_HEIGHT),
        )
        .into_styled(BUTTON_STROKE_STYLE)
        .draw(buffer)
        .ok();
        Text::with_baseline(
            "next",
            Point::new(
                NEXT_BUTTON_LEFT + (TARGET_BUTTON_WIDTH as i32 - TARGET_BUTTON_LABEL_WIDTH) / 2,
                TARGET_CONTROL_TOP + 2,
            ),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();

        for slider_index in 0..SLIDER_COUNT {
            let y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
            let value = self.params[slider_index];

            Text::with_baseline(
                PARAM_NAMES[slider_index],
                Point::new(SLIDER_LEFT, y - 12),
                text_style,
                Baseline::Top,
            )
            .draw(buffer)
            .ok();

            Line::new(
                Point::new(SLIDER_TRACK_LEFT, y + 8),
                Point::new(SLIDER_RIGHT, y + 8),
            )
            .into_styled(SLIDER_TRACK_STYLE)
            .draw(buffer)
            .ok();

            let knob_x =
                SLIDER_TRACK_LEFT + round_to_i32((SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32 * value);
            Circle::with_center(Point::new(knob_x, y + 8), 9)
                .into_styled(YELLOW_FILL_STYLE)
                .draw(buffer)
                .ok();
        }

        Text::with_baseline(
            "x/y view",
            Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y - 15),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();
        Line::new(
            Point::new(VIEW_SLIDER_LEFT, VIEW_SLIDER_Y),
            Point::new(VIEW_SLIDER_RIGHT, VIEW_SLIDER_Y),
        )
        .into_styled(SLIDER_TRACK_STYLE)
        .draw(buffer)
        .ok();
        let view_knob_x = VIEW_SLIDER_LEFT
            + round_to_i32((VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT) as f32 * self.xy_mix);
        Circle::with_center(Point::new(view_knob_x, VIEW_SLIDER_Y), 9)
            .into_styled(YELLOW_FILL_STYLE)
            .draw(buffer)
            .ok();
    }

    fn draw_calibrate_button(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        Rectangle::new(
            Point::new(CALIBRATE_BUTTON_LEFT, CALIBRATE_BUTTON_TOP),
            Size::new(CALIBRATE_BUTTON_WIDTH, CALIBRATE_BUTTON_HEIGHT),
        )
        .into_styled(BUTTON_STROKE_STYLE)
        .draw(buffer)
        .ok();
        Text::with_baseline(
            "cal",
            Point::new(CALIBRATE_BUTTON_LEFT + 6, CALIBRATE_BUTTON_TOP + 2),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();
    }

    fn draw_reverse_kinematics_run_button(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        if self.is_reverse_kinematics_running() {
            Rectangle::new(
                Point::new(RK_RUN_LEFT + 4, RK_CONTROL_TOP + 4),
                Size::new((RK_BUTTON_SIZE - 8) as u32, (RK_BUTTON_SIZE - 8) as u32),
            )
            .into_styled(STOP_FILL_STYLE)
            .draw(buffer)
            .ok();
        } else {
            Triangle::new(
                Point::new(RK_RUN_LEFT, RK_CONTROL_TOP),
                Point::new(RK_RUN_LEFT, RK_CONTROL_TOP + RK_BUTTON_SIZE),
                Point::new(
                    RK_RUN_LEFT + RK_BUTTON_SIZE,
                    RK_CONTROL_TOP + RK_BUTTON_SIZE / 2,
                ),
            )
            .into_styled(PLAY_FILL_STYLE)
            .draw(buffer)
            .ok();
        }
    }

    fn draw_reverse_kinematics_step_button(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        Rectangle::new(
            Point::new(RK_STEP_LEFT, RK_CONTROL_TOP),
            Size::new(RK_BUTTON_SIZE as u32, RK_BUTTON_SIZE as u32),
        )
        .into_styled(BUTTON_STROKE_STYLE)
        .draw(buffer)
        .ok();
        Rectangle::new(
            Point::new(
                RK_STEP_LEFT + RK_BUTTON_SIZE - 5,
                RK_CONTROL_TOP + RK_BUTTON_SIZE / 2 - 5,
            ),
            Size::new(2, 10),
        )
        .into_styled(STOP_FILL_STYLE)
        .draw(buffer)
        .ok();
        Triangle::new(
            Point::new(RK_STEP_LEFT + 3, RK_CONTROL_TOP + 4),
            Point::new(RK_STEP_LEFT + 3, RK_CONTROL_TOP + RK_BUTTON_SIZE - 4),
            Point::new(
                RK_STEP_LEFT + RK_BUTTON_SIZE - 7,
                RK_CONTROL_TOP + RK_BUTTON_SIZE / 2,
            ),
        )
        .into_styled(PLAY_FILL_STYLE)
        .draw(buffer)
        .ok();
    }

    fn draw_report(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        let mut report = DistanceReport::new();
        Text::with_baseline(
            report.as_str(self.target_distance()),
            Point::new(DISTANCE_REPORT_LEFT, 5),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();
    }

    fn draw_fps(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        if !self.show_fps {
            return;
        }

        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_LIGHT_SLATE_GRAY);
        let mut report = FpsReport::new();
        Text::with_baseline(
            report.as_str(self.fps),
            Point::new(FPS_REPORT_LEFT, FPS_REPORT_TOP),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();
    }

    fn draw_version(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_LIGHT_SLATE_GRAY);
        Text::with_baseline(
            VERSION_TEXT,
            Point::new(VERSION_REPORT_LEFT, VERSION_REPORT_TOP),
            text_style,
            Baseline::Top,
        )
        .draw(buffer)
        .ok();
    }

    fn draw_touch_cursor(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        if let Some((x, y)) = self.touch_cursor {
            let x = x as i32;
            let y = y as i32;
            let radius = 5;
            let cursor_style = PrimitiveStyle::with_fill(Rgb565::CYAN);
            Circle::new(Point::new(x - radius, y - radius), (radius * 2 + 1) as u32)
                .into_styled(cursor_style)
                .draw(buffer)
                .ok();
        }
    }

    fn pose_to_screen(&self, pose: Pose) -> Point {
        let Vec3([x, y, z]) = pose.position();
        self.world_to_screen(x, y, -z)
    }

    fn world_to_screen(&self, x: f32, y: f32, z: f32) -> Point {
        let projection = project(x, y, z, self.xy_mix, self.z_mix);
        Point::new(
            horizontal_to_screen(projection.horizontal, self.zoom),
            vertical_to_screen(projection.vertical, self.z_mix, self.zoom),
        )
    }

    fn target_distance(&self) -> f32 {
        target_distance(&self.params, target_from_seed(self.target_seed))
    }
}

impl Default for CydSim {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum ActiveControl {
    RightSlider(usize),
    Tilt,
    Zoom,
    XyView,
    PreviousTarget,
    NextTarget,
    ToggleReverseKinematics,
    StepReverseKinematics,
    Calibrate,
}

#[derive(Clone, Copy)]
struct Projection {
    horizontal: f32,
    vertical: f32,
}

fn project(x: f32, y: f32, z: f32, xy_mix: f32, z_mix: f32) -> Projection {
    let angle = (xy_mix - 0.5) * TAU;
    let cos = libm::cosf(angle);
    let sin = libm::sinf(angle);
    let horizontal = x * cos + y * sin;
    let depth = -x * sin + y * cos;
    Projection {
        horizontal,
        vertical: z * (1.0 - z_mix) + depth * z_mix,
    }
}

fn control_at(x: f32, y: f32) -> Option<ActiveControl> {
    if (x - TILT_X as f32).abs() <= 14.0 && (TILT_TOP as f32..=TILT_BOTTOM as f32).contains(&y) {
        return Some(ActiveControl::Tilt);
    }
    if (x - ZOOM_X as f32).abs() <= 14.0 && (ZOOM_TOP as f32..=ZOOM_BOTTOM as f32).contains(&y) {
        return Some(ActiveControl::Zoom);
    }
    if (RK_RUN_LEFT as f32..=(RK_RUN_LEFT + RK_BUTTON_SIZE) as f32).contains(&x)
        && (RK_CONTROL_TOP as f32..=(RK_CONTROL_TOP + RK_BUTTON_SIZE) as f32).contains(&y)
    {
        return Some(ActiveControl::ToggleReverseKinematics);
    }
    if (RK_STEP_LEFT as f32..=(RK_STEP_LEFT + RK_BUTTON_SIZE) as f32).contains(&x)
        && (RK_CONTROL_TOP as f32..=(RK_CONTROL_TOP + RK_BUTTON_SIZE) as f32).contains(&y)
    {
        return Some(ActiveControl::StepReverseKinematics);
    }
    if (PREV_BUTTON_LEFT as f32..=(PREV_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32) as f32)
        .contains(&x)
        && (TARGET_CONTROL_TOP as f32..=(TARGET_CONTROL_TOP + TARGET_BUTTON_HEIGHT as i32) as f32)
            .contains(&y)
    {
        return Some(ActiveControl::PreviousTarget);
    }
    if (NEXT_BUTTON_LEFT as f32..=(NEXT_BUTTON_LEFT + TARGET_BUTTON_WIDTH as i32) as f32)
        .contains(&x)
        && (TARGET_CONTROL_TOP as f32..=(TARGET_CONTROL_TOP + TARGET_BUTTON_HEIGHT as i32) as f32)
            .contains(&y)
    {
        return Some(ActiveControl::NextTarget);
    }
    if (VIEW_SLIDER_Y as f32 - y).abs() <= 14.0
        && (VIEW_SLIDER_LEFT as f32..=VIEW_SLIDER_RIGHT as f32).contains(&x)
    {
        return Some(ActiveControl::XyView);
    }
    if (CALIBRATE_BUTTON_LEFT as f32
        ..=(CALIBRATE_BUTTON_LEFT + CALIBRATE_BUTTON_WIDTH as i32) as f32)
        .contains(&x)
        && (CALIBRATE_BUTTON_TOP as f32
            ..=(CALIBRATE_BUTTON_TOP + CALIBRATE_BUTTON_HEIGHT as i32) as f32)
            .contains(&y)
    {
        return Some(ActiveControl::Calibrate);
    }
    for slider_index in 0..SLIDER_COUNT {
        let slider_y = SLIDER_TOP + slider_index as i32 * SLIDER_STEP;
        if x >= SLIDER_LEFT as f32 && (y - (slider_y + 8) as f32).abs() <= 13.0 {
            return Some(ActiveControl::RightSlider(slider_index));
        }
    }
    None
}

fn horizontal_to_screen(horizontal: f32, zoom: f32) -> i32 {
    scale_to_screen(
        horizontal,
        HORIZONTAL_MIN,
        HORIZONTAL_MAX,
        SCREEN_WIDTH,
        zoom,
    )
}

fn vertical_to_screen(vertical: f32, z_mix: f32, zoom: f32) -> i32 {
    let low = Z_MIN * (1.0 - z_mix) + HORIZONTAL_MIN * z_mix;
    let high = Z_MAX * (1.0 - z_mix) + HORIZONTAL_MAX * z_mix;
    (SCREEN_HEIGHT as i32 - 1) - scale_to_screen(vertical, low, high, SCREEN_HEIGHT, zoom)
}

fn scale_to_screen(value: f32, low: f32, high: f32, pixels: usize, zoom: f32) -> i32 {
    let scale = zoom_to_scale(zoom);
    let origin_fraction = ((0.0 - low) / (high - low)).clamp(0.0, 1.0);
    let fraction = (origin_fraction + ((value - 0.0) / (high - low)) * scale).clamp(0.0, 1.0);
    round_to_i32(fraction * (pixels - 1) as f32)
}

fn zoom_to_scale(zoom: f32) -> f32 {
    0.5 + zoom
}

fn zoomed_pixels(base_pixels: u32, zoom: f32) -> u32 {
    round_to_u32(base_pixels as f32 * zoom_to_scale(zoom)).max(1)
}

fn world_diameter_to_screen(diameter: f32, zoom: f32) -> u32 {
    round_to_u32(
        diameter * (SCREEN_WIDTH - 1) as f32 / (HORIZONTAL_MAX - HORIZONTAL_MIN)
            * zoom_to_scale(zoom),
    )
    .max(1)
}

fn grid_stroke_style(zoom: f32) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_stroke(Rgb565::CSS_DARK_SLATE_GRAY, zoomed_pixels(2, zoom))
}

fn arm_stroke_style(width: u32) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_stroke(Rgb565::CSS_DARK_CYAN, width)
}

#[derive(Clone, Copy)]
struct HandMeasurement {
    center: Vec3,
    width: f32,
}

#[derive(Clone, Copy)]
struct Target {
    center: Vec3,
    diameter: f32,
}

#[derive(Clone, Copy)]
struct ReverseKinematicsRun {
    search_params: [f32; 6],
    best_params: [f32; 6],
    target: Target,
    best_distance: f32,
    step: f32,
    candidate_index: usize,
    sweep_improved: bool,
    phase: ReverseKinematicsPhase,
}

#[derive(Clone, Copy)]
enum ReverseKinematicsPhase {
    BeginCandidate,
    EvaluateSingleHigh {
        index: usize,
        original: f32,
    },
    EvaluateSingleLow {
        index: usize,
        original: f32,
    },
    EvaluatePair {
        bend_original: f32,
        spin_original: f32,
    },
}

impl ReverseKinematicsRun {
    fn new(params: &[f32; 6], target: Target) -> Self {
        Self {
            search_params: *params,
            best_params: *params,
            target,
            best_distance: target_distance(params, target),
            step: RK_INITIAL_STEP,
            candidate_index: 0,
            sweep_improved: false,
            phase: ReverseKinematicsPhase::BeginCandidate,
        }
    }

    fn tick_search_candidate(&mut self) -> bool {
        let candidate_index = self.candidate_index;
        let mut searched = false;

        loop {
            if !self.tick_search() {
                return searched;
            }

            searched = true;
            if matches!(self.phase, ReverseKinematicsPhase::BeginCandidate)
                && self.candidate_index != candidate_index
            {
                return true;
            }
        }
    }

    fn tick_search(&mut self) -> bool {
        loop {
            match self.phase {
                ReverseKinematicsPhase::BeginCandidate => {
                    if !self.prepare_next_candidate() {
                        return false;
                    }

                    if self.candidate_index >= SLIDER_COUNT {
                        let bend_original = self.search_params[PARAM_BEND_ELBOW];
                        let spin_original = self.search_params[PARAM_SPIN_WHOLE_ARM];
                        let pair_index = self.candidate_index - SLIDER_COUNT;
                        if apply_paired_candidate(&mut self.search_params, pair_index, self.step) {
                            self.phase = ReverseKinematicsPhase::EvaluatePair {
                                bend_original,
                                spin_original,
                            };
                            return true;
                        }

                        self.finish_candidate();
                        continue;
                    }

                    let original = self.search_params[self.candidate_index];
                    let high = (original + self.step).min(1.0);
                    if high != original {
                        self.search_params[self.candidate_index] = high;
                        self.phase = ReverseKinematicsPhase::EvaluateSingleHigh {
                            index: self.candidate_index,
                            original,
                        };
                        return true;
                    }

                    self.phase = ReverseKinematicsPhase::EvaluateSingleHigh {
                        index: self.candidate_index,
                        original,
                    };
                }
                ReverseKinematicsPhase::EvaluateSingleHigh { index, original } => {
                    if self.keep_if_improved() {
                        self.finish_candidate();
                        return true;
                    }

                    self.search_params[index] = original;
                    let low = (original - self.step).max(0.0);
                    if low != original {
                        self.search_params[index] = low;
                        self.phase = ReverseKinematicsPhase::EvaluateSingleLow { index, original };
                        return true;
                    }

                    self.finish_candidate();
                    return true;
                }
                ReverseKinematicsPhase::EvaluateSingleLow { index, original } => {
                    if !self.keep_if_improved() {
                        self.search_params[index] = original;
                    }
                    self.finish_candidate();
                    return true;
                }
                ReverseKinematicsPhase::EvaluatePair {
                    bend_original,
                    spin_original,
                } => {
                    if !self.keep_if_improved() {
                        self.search_params[PARAM_BEND_ELBOW] = bend_original;
                        self.search_params[PARAM_SPIN_WHOLE_ARM] = spin_original;
                    }
                    self.finish_candidate();
                    return true;
                }
            }
        }
    }

    fn prepare_next_candidate(&mut self) -> bool {
        while self.candidate_index >= RK_CANDIDATE_COUNT {
            if self.sweep_improved {
                self.sweep_improved = false;
            } else {
                self.step *= 0.5;
                if self.step < RK_MIN_STEP {
                    return false;
                }
            }
            self.candidate_index = 0;
        }

        true
    }

    fn keep_if_improved(&mut self) -> bool {
        let distance = target_distance(&self.search_params, self.target);
        if distance < self.best_distance {
            self.best_distance = distance;
            self.best_params = self.search_params;
            self.sweep_improved = true;
            true
        } else {
            false
        }
    }

    fn finish_candidate(&mut self) {
        self.candidate_index += 1;
        self.phase = ReverseKinematicsPhase::BeginCandidate;
    }
}

fn move_params_toward(params: &mut [f32; 6], target_params: &[f32; 6], max_change: f32) -> bool {
    let mut moved = false;

    for param_index in 0..params.len() {
        let delta = target_params[param_index] - params[param_index];
        if delta == 0.0 {
            continue;
        }

        let change = delta.clamp(-max_change, max_change);
        params[param_index] = (params[param_index] + change).clamp(0.0, 1.0);
        moved = true;
    }

    moved
}

fn reverse_kinematics_visible_param_step(dt_seconds: f32) -> f32 {
    dt_seconds.clamp(0.0, RK_MAX_TICK_SECONDS) * RK_VISIBLE_PARAM_POINTS_PER_SECOND
}

fn apply_paired_candidate(params: &mut [f32; 6], pair_index: usize, step: f32) -> bool {
    let (bend_direction, spin_direction) = RK_PAIRED_CANDIDATES[pair_index];
    let bend_original = params[PARAM_BEND_ELBOW];
    let spin_original = params[PARAM_SPIN_WHOLE_ARM];

    params[PARAM_BEND_ELBOW] = (bend_original + bend_direction * step).clamp(0.0, 1.0);
    params[PARAM_SPIN_WHOLE_ARM] = (spin_original + spin_direction * step).clamp(0.0, 1.0);

    params[PARAM_BEND_ELBOW] != bend_original || params[PARAM_SPIN_WHOLE_ARM] != spin_original
}

fn hand_measurement(params: &[f32; 6]) -> HandMeasurement {
    let mut corners = [Vec3::ZERO; 4];
    for (pose_index, pose) in LINKAGE.poses(params).enumerate() {
        for (corner_index, hand_pose_index) in HAND_CORNER_POSE_INDICES.iter().enumerate() {
            if pose_index == *hand_pose_index {
                corners[corner_index] = display_world_position(pose);
            }
        }
    }

    let center = (corners[0] + corners[1] + corners[2] + corners[3]) * 0.25;
    let width = (distance(corners[0], corners[2]) + distance(corners[1], corners[3])) * 0.5;

    HandMeasurement { center, width }
}

fn display_world_position(pose: Pose) -> Vec3 {
    let Vec3([x, y, z]) = pose.position();
    Vec3([x, y, -z])
}

fn target_from_seed(seed: u8) -> Target {
    let mut rng = WyRand::new_seed(u64::from(seed));
    let mut target_params = [0.0; 6];
    for (param_index, param) in target_params.iter_mut().enumerate() {
        *param = if param_index == 2 {
            0.0
        } else {
            random_fraction(&mut rng)
        };
    }

    let diameter = TARGET_MIN_DIAMETER
        + random_fraction(&mut rng) * (TARGET_MAX_DIAMETER - TARGET_MIN_DIAMETER);

    Target {
        center: hand_measurement(&target_params).center,
        diameter,
    }
}

struct TargetLabel {
    bytes: [u8; 11],
    len: usize,
}

impl TargetLabel {
    fn new() -> Self {
        Self {
            bytes: *b"target #000",
            len: 11,
        }
    }

    fn as_str(&mut self, value: u8) -> &str {
        let hundreds = value / 100;
        let tens = (value / 10) % 10;
        let ones = value % 10;

        if hundreds > 0 {
            self.bytes[8] = b'0' + hundreds;
            self.bytes[9] = b'0' + tens;
            self.bytes[10] = b'0' + ones;
            self.len = 11;
        } else if tens > 0 {
            self.bytes[8] = b'0' + tens;
            self.bytes[9] = b'0' + ones;
            self.len = 10;
        } else {
            self.bytes[8] = b'0' + ones;
            self.len = 9;
        }

        core::str::from_utf8(&self.bytes[..self.len]).expect("target label is ASCII")
    }
}

fn reverse_kinematics(params: &mut [f32; 6], target: Target) -> f32 {
    let mut best_distance = target_distance(params, target);
    let mut step = RK_INITIAL_STEP;

    while step >= RK_MIN_STEP {
        let mut improved = false;

        for param_index in 0..params.len() {
            let original = params[param_index];

            let high = (original + step).min(1.0);
            if high != original {
                params[param_index] = high;
                let distance = target_distance(params, target);
                if distance < best_distance {
                    best_distance = distance;
                    improved = true;
                    continue;
                }
            }

            let low = (original - step).max(0.0);
            if low != original {
                params[param_index] = low;
                let distance = target_distance(params, target);
                if distance < best_distance {
                    best_distance = distance;
                    improved = true;
                    continue;
                }
            }

            params[param_index] = original;
        }

        for pair_index in 0..RK_PAIRED_CANDIDATES.len() {
            let bend_original = params[PARAM_BEND_ELBOW];
            let spin_original = params[PARAM_SPIN_WHOLE_ARM];
            if !apply_paired_candidate(params, pair_index, step) {
                continue;
            }

            let distance = target_distance(params, target);
            if distance < best_distance {
                best_distance = distance;
                improved = true;
            } else {
                params[PARAM_BEND_ELBOW] = bend_original;
                params[PARAM_SPIN_WHOLE_ARM] = spin_original;
            }
        }

        if !improved {
            step *= 0.5;
        }
    }

    best_distance
}

fn target_distance(params: &[f32; 6], target: Target) -> f32 {
    let hand = hand_measurement(params);
    let Vec3([hand_x, hand_y, hand_z]) = hand.center;
    let Vec3([target_x, target_y, target_z]) = target.center;
    let size_delta = hand.width - target.diameter;

    libm::sqrtf(
        square(hand_x - target_x)
            + square(hand_y - target_y)
            + square(hand_z - target_z)
            + square(size_delta),
    )
}

fn random_fraction(rng: &mut WyRand) -> f32 {
    rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0)
}

fn distance(left: Vec3, right: Vec3) -> f32 {
    let Vec3([left_x, left_y, left_z]) = left;
    let Vec3([right_x, right_y, right_z]) = right;
    libm::sqrtf(square(left_x - right_x) + square(left_y - right_y) + square(left_z - right_z))
}

fn square(value: f32) -> f32 {
    value * value
}

fn round_to_i32(value: f32) -> i32 {
    libm::roundf(value) as i32
}

fn round_to_u32(value: f32) -> u32 {
    libm::roundf(value) as u32
}

struct DistanceReport {
    bytes: [u8; 14],
    len: usize,
}

struct FpsReport {
    bytes: [u8; 7],
    len: usize,
}

impl DistanceReport {
    fn new() -> Self {
        Self {
            bytes: *b"distance 00.00",
            len: 14,
        }
    }

    fn as_str(&mut self, value: f32) -> &str {
        let hundredths = round_to_u32(value.clamp(0.0, 99.99) * 100.0);
        let whole = hundredths / 100;
        let fraction = hundredths % 100;

        self.bytes[9] = b'0' + (whole / 10) as u8;
        self.bytes[10] = b'0' + (whole % 10) as u8;
        self.bytes[12] = b'0' + (fraction / 10) as u8;
        self.bytes[13] = b'0' + (fraction % 10) as u8;

        core::str::from_utf8(&self.bytes[..self.len]).expect("distance report is ASCII")
    }
}

impl FpsReport {
    fn new() -> Self {
        Self {
            bytes: *b"--- fps",
            len: 7,
        }
    }

    fn as_str(&mut self, fps: Option<u32>) -> &str {
        if let Some(fps) = fps {
            let fps = fps.min(999);
            self.bytes[0] = if fps >= 100 {
                b'0' + (fps / 100) as u8
            } else {
                b' '
            };
            self.bytes[1] = if fps >= 10 {
                b'0' + ((fps / 10) % 10) as u8
            } else {
                b' '
            };
            self.bytes[2] = b'0' + (fps % 10) as u8;
        }

        core::str::from_utf8(&self.bytes[..self.len]).expect("fps report is ASCII")
    }
}

impl Drawable for CydSim {
    type Color = Rgb565;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        target.clear(Rgb565::BLACK)?; // without: 46.4 fps, 21.55 ms frame, inferred cost 20.64 ms
        self.draw_grid(target); // without: 26.8 fps, 37.31 ms frame, inferred cost 4.88 ms
        self.draw_target(target); // without: 24.2 fps, 41.32 ms frame, inferred cost 0.87 ms
        self.draw_sliders(target); // without: 31.4 fps, 31.85 ms frame, inferred cost 10.35 ms
        self.draw_arm(target); // without: 26.9 fps, 37.17 ms frame, inferred cost 5.02 ms
        self.draw_report(target); // without: 25.1 fps, 39.84 ms frame, inferred cost 2.35 ms
        self.draw_version(target); // without: 24.0 fps, 41.67 ms frame, inferred cost 0.53 ms
        self.draw_fps(target); // without: 23.9 fps, 41.84 ms frame, inferred cost 0.35 ms
        self.draw_touch_cursor(target); // without: 23.7 fps, 42.19 ms frame, inferred cost ~0 ms
        Ok(())
    }
}

pub struct FrameBuffer {
    pixels: [Rgb565; SCREEN_PIXELS],
}

impl FrameBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: [Rgb565::BLACK; SCREEN_PIXELS],
        }
    }

    pub fn static_new() -> &'static mut Self {
        static FRAME_BUFFER: StaticCell<FrameBuffer> = StaticCell::new();
        FRAME_BUFFER.init_with(FrameBuffer::new)
    }

    pub fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color);
    }

    pub fn pixels_mut(&mut self) -> &mut [Rgb565; SCREEN_PIXELS] {
        &mut self.pixels
    }

    #[must_use]
    pub fn pixels(&self) -> &[Rgb565; SCREEN_PIXELS] {
        &self.pixels
    }
}

impl DrawTarget for FrameBuffer {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let x = point.x as usize;
            let y = point.y as usize;
            if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
                continue;
            }
            self.pixels[y * SCREEN_WIDTH + x] = color;
        }
        Ok(())
    }
}

impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CydSim, RK_SINGLE_STEP_VISIBLE_PARAM_STEP, RK_VISIBLE_PARAM_POINTS_PER_SECOND,
        reverse_kinematics_visible_param_step,
    };

    #[test]
    fn test_reverse_kinematics_does_not_increase_distance() {
        let mut sim = CydSim::new();
        let before = sim.target_distance();
        let after = sim.reverse_kinematics();

        assert!(after <= before, "expected {after} <= {before}");
    }

    #[test]
    fn test_stepped_reverse_kinematics_does_not_increase_distance() {
        let mut sim = CydSim::new();
        let before = sim.target_distance();
        sim.start_reverse_kinematics();

        for _ in 0..512 {
            if !sim.tick_reverse_kinematics(1.0 / 60.0) {
                break;
            }
        }

        let after = sim.target_distance();
        assert!(after <= before, "expected {after} <= {before}");
    }

    #[test]
    fn test_stepped_reverse_kinematics_limits_visible_param_change() {
        let mut sim = CydSim::new();
        let before = sim.params;
        sim.start_reverse_kinematics();
        let dt_seconds = 0.5;
        sim.tick_reverse_kinematics(dt_seconds);
        let max_change = reverse_kinematics_visible_param_step(dt_seconds);

        for (before, after) in before.iter().zip(sim.params.iter()) {
            assert!((after - before).abs() <= max_change + 1e-6);
        }
    }

    #[test]
    fn test_reverse_kinematics_toggle_starts_and_stops() {
        let mut sim = CydSim::new();

        sim.toggle_reverse_kinematics();
        assert!(sim.is_reverse_kinematics_running());

        sim.toggle_reverse_kinematics();
        assert!(!sim.is_reverse_kinematics_running());
    }

    #[test]
    fn test_single_reverse_kinematics_step_starts_and_advances() {
        let mut sim = CydSim::new();
        let before = sim.params;

        assert!(sim.step_reverse_kinematics());

        assert!(!sim.is_reverse_kinematics_running());
        assert_ne!(sim.params, before);
        assert!(max_param_delta(before, sim.params) <= RK_SINGLE_STEP_VISIBLE_PARAM_STEP + 1e-6);
    }

    #[test]
    fn test_reverse_kinematics_long_tick_uses_bounded_speed() {
        let mut sim = CydSim::new();
        let before = sim.params;
        sim.start_reverse_kinematics();

        sim.tick_reverse_kinematics(1.0);

        assert!(max_param_delta(before, sim.params) <= 0.1 * RK_VISIBLE_PARAM_POINTS_PER_SECOND);
    }

    fn max_param_delta(before: [f32; 6], after: [f32; 6]) -> f32 {
        let mut max_delta = 0.0;
        for (before, after) in before.iter().zip(after.iter()) {
            let delta = (after - before).abs();
            if delta > max_delta {
                max_delta = delta;
            }
        }
        max_delta
    }
}
