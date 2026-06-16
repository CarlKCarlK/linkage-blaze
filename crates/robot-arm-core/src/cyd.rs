use core::convert::Infallible;

use embassy_time::Instant;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{IntoStorage, Rgb565, RgbColor, WebColors},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    text::{Baseline, Text},
};
use nanorand::{Rng, WyRand};
use static_cell::StaticCell;

use crate::{DrawItem, Linkage, Pose, Rgb888, Vec3};

pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 240;
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

// ---- layout constants ----
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

// ---- world / display constants ----
const PIXELS_PER_UNIT: f32 = SCREEN_WIDTH as f32 / 16.0; // 16 world units span the screen width

// ---- DOF and parameter indices ----
const DOF: usize = 14;
const ARM_PARAM_START: usize = 2;
const ARM_PARAM_COUNT: usize = 6;
const TARGET_PARAM_START: usize = 8;


// ---- linkage colors ----
const ARM_COLOR: Rgb888 = Rgb888::CSS_DARK_GREEN;
const ARM_WIDTH: u16 = 3;

// ---- RK constants ----
const RK_INITIAL_STEP: f32 = 0.125;
const RK_MIN_STEP: f32 = 0.001;
const RK_VISIBLE_PARAM_POINTS_PER_SECOND: f32 = 0.6;
const RK_MAX_TICK_SECONDS: f32 = 0.1;
const RK_SINGLE_STEP_VISIBLE_PARAM_STEP: f32 = 0.01;
const RK_SEARCH_CANDIDATES_PER_TICK: usize = 4;
const RK_PAIRED_CANDIDATES: [(f32, f32); 4] = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)];
const RK_CANDIDATE_COUNT: usize = ARM_PARAM_COUNT + RK_PAIRED_CANDIDATES.len();

// ---- styles ----
const SLIDER_TRACK_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 2);
const BUTTON_STROKE_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_stroke(Rgb565::CSS_LIGHT_SLATE_GRAY, 1);
const YELLOW_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::CSS_YELLOW);
const GREEN_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::GREEN);
const GRAY_FILL_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_fill(Rgb565::CSS_LIGHT_SLATE_GRAY);
const PLAY_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::GREEN);
const STOP_FILL_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::WHITE);

// ---- linkages ----
//
// Section 1: floor disk + axis lines.  Pen starts down; restart leaves pen down for arm section.
// Section 2: arm.  Pen down for strokes.
// Section 3: target traversal (pen up) then target disk.
const LINKAGE: Linkage<DOF, 90> = Linkage::start()
    .define_param("x/y view", 0.5 + 30.0 / 360.0)
    .define_param("z", (30.0 + 45.0) / 90.0)
    .define_param("raise hand", 0.5)
    .define_param("bend elbow", 0.5)
    .define_param("close hand", 0.0)
    .define_param("lower arm", 0.5)
    .define_param("spin whole", 0.5)
    .define_param("spin hand", 0.5)
    .define_param("target yaw 0", 0.5)
    .define_param("target pitch 1", 0.5)
    .define_param("target yaw 2", 0.5)
    .define_param("target pitch 3", 0.5)
    .define_param("target yaw 4", 0.5)
    .define_param("target roll 5", 0.5)
    // ---- floor ----
    // .pitch_param(BASE_PITCH_PARAM, -45.0, 45.0)
    // .yaw_param(BASE_YAW_PARAM, -180.0, 180.0)
    // .pen_color(FLOOR_COLOR)
    // .disk(FLOOR_RADIUS)
    // .pen_color(AXIS_COLOR)
    // .pen_width(AXIS_WIDTH)
    // // x-axis half-line (forward direction)
    // .forward(AXIS_RADIUS)
    // .pen_up()
    // .yaw(180.0)
    // .forward(AXIS_RADIUS) // silently return to origin
    // .yaw(90.0) // now facing +col1 (y axis)
    // .pen_down()
    // .forward(AXIS_RADIUS)
    // .restart()
    // ---- arm ----
    .pen_color(ARM_COLOR) // todo0000 default to color white and width 1
    .pen_width(ARM_WIDTH)
    .yaw(90.0)
    .pitch(-90.0)
    .yaw_param("x/y view", 90.0, -90.0)
    .pitch_param("z", -45.0, 45.0)
    .yaw_param("spin whole", 360.0, -360.0)
    .pitch(90.0)
    .forward(2.5)
    .pitch(-90.0)
    .pitch_param("lower arm", 30.0, 0.0)
    .forward(3.0)
    .yaw_param("bend elbow", 90.0, -90.0)
    .forward(3.0)
    .pitch_param("raise hand", 90.0, -90.0)
    .forward(1.0)
    .roll_param("spin hand", -360.0, 360.0)
    .forward(0.5)
    .yaw(90.0)
    .forward_param("close hand", 0.5, 0.0)
    .yaw(-90.0)
    .forward(1.0)
    .yaw(180.0)
    .forward(1.0)
    .yaw(90.0)
    .forward_param("close hand", 1.0, 0.0)
    .yaw(90.0)
    .forward(1.0)
    .restart()
    // // ---- target (pen up: no arm strokes, only the disk at the end) ----
    // .pen_up()
    // .pitch_param(BASE_PITCH_PARAM, -45.0, 45.0)
    // .yaw_param(BASE_YAW_PARAM, -180.0, 180.0)
    // .pen_color(TARGET_COLOR)
    // .yaw(90.0)
    // .yaw_param("spin whole", 360.0, -360.0)
    // .pitch(90.0)
    // .forward(2.5)
    // .pitch(-90.0)
    // .pitch_param("lower arm", 30.0, 0.0)
    // .forward(3.0)
    // .yaw_param("bend elbow", 90.0, -90.0)
    // .forward(3.0)
    // .pitch_param("raise hand", 90.0, -90.0)
    // .forward(1.0)
    // .roll_param("spin hand", -360.0, 360.0)
    // .forward(0.5)
    // .yaw(90.0)
    // .forward_param("close hand", 0.5, 0.0)
    // .yaw(-90.0)
    // .forward(1.0)
    // .yaw(180.0)
    // .forward(1.0)
    // .yaw(90.0)
    // .forward_param("close hand", 1.0, 0.0)
    // .yaw(90.0)
    // .forward(1.0)
    // // 6 random extension joints → target position
    // .yaw_param(TARGET_PARAM_START, -180.0, 180.0)
    // .forward(2.0)
    // .pitch_param(TARGET_PARAM_START + 1, -60.0, 60.0)
    // .forward(2.0)
    // .yaw_param(TARGET_PARAM_START + 2, -180.0, 180.0)
    // .forward(2.0)
    // .pitch_param(TARGET_PARAM_START + 3, -60.0, 60.0)
    // .forward(2.0)
    // .yaw_param(TARGET_PARAM_START + 4, -180.0, 180.0)
    // .forward(2.0)
    // .roll_param(TARGET_PARAM_START + 5, -180.0, 180.0)
    // .forward(2.0)
    // .disk_param("close hand", TARGET_MIN_RADIUS, TARGET_MAX_RADIUS);
    ;

// Arm-only linkage used for RK distance computation (same base + arm, no floor/target).
const ARM_LINKAGE: Linkage<8, 30> = Linkage::start()
    .define_param("x/y view", 0.5 + 30.0 / 360.0)
    .define_param("z", (30.0 + 45.0) / 90.0)
    .define_param("raise hand", 0.5)
    .define_param("bend elbow", 0.5)
    .define_param("close hand", 0.0)
    .define_param("lower arm", 0.5)
    .define_param("spin whole", 0.5)
    .define_param("spin hand", 0.5)
    .pitch_param("z", -45.0, 45.0)
    .yaw_param("x/y view", -180.0, 180.0)
    .yaw(90.0)
    .yaw_param("spin whole", 360.0, -360.0)
    .pitch(90.0)
    .forward(2.5)
    .pitch(-90.0)
    .pitch_param("lower arm", 30.0, 0.0)
    .forward(3.0)
    .yaw_param("bend elbow", 90.0, -90.0)
    .forward(3.0)
    .pitch_param("raise hand", 90.0, -90.0)
    .forward(1.0)
    .roll_param("spin hand", -360.0, 360.0)
    .forward(0.5)
    .yaw(90.0)
    .forward_param("close hand", 0.5, 0.0)
    .yaw(-90.0)
    .forward(1.0)
    .yaw(180.0)
    .forward(1.0)
    .yaw(90.0)
    .forward_param("close hand", 1.0, 0.0)
    .yaw(90.0)
    .forward(1.0);

fn param_index(name: &str) -> usize {
    LINKAGE
        .param_index(name)
        .expect("parameter must be defined")
}

fn base_yaw_param() -> usize {
    param_index("x/y view")
}

fn base_pitch_param() -> usize {
    param_index("z")
}

fn bend_elbow_param() -> usize {
    param_index("bend elbow")
}

fn lower_arm_param() -> usize {
    param_index("lower arm")
}

fn spin_whole_arm_param() -> usize {
    param_index("spin whole")
}

pub struct CydSim {
    params: [f32; DOF],
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
    controlled_knobs: [ControlledKnob; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlledKnob {
    Param(usize),
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
    pub fn param_count() -> usize {
        DOF
    }

    #[must_use]
    pub fn param_index(name: &str) -> Option<usize> {
        LINKAGE.param_index(name)
    }

    #[must_use]
    pub fn param_name(index: usize) -> &'static str {
        LINKAGE.param_name(index)
    }

    #[must_use]
    pub fn param_default(index: usize) -> f32 {
        LINKAGE.param_default(index)
    }

    #[must_use]
    pub fn get_param(&self, index: usize) -> f32 {
        assert!(index < DOF, "param index out of range");
        self.params[index]
    }

    pub fn set_param_by_index(&mut self, index: usize, value: f32) {
        assert!(index < DOF, "param index out of range");
        self.params[index] = value.clamp(0.0, 1.0);
    }

    pub fn draw_view_only<D: DrawTarget<Color = Rgb565>>(
        &self,
        target: &mut D,
    ) -> Result<(), D::Error> {
        target.clear(Rgb565::BLACK)?;
        self.draw_linkage(target);
        Ok(())
    }

    #[must_use]
    pub fn new() -> Self {
        Self::new_inner(false)
    }

    #[must_use]
    pub fn new_with_fps() -> Self {
        Self::new_inner(true)
    }

    fn new_inner(show_fps: bool) -> Self {
        let mut params = LINKAGE.param_defaults();
        randomize_target_params(&mut params, 0);

        Self {
            params,
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
            controlled_knobs: [
                ControlledKnob::Param(lower_arm_param()),
                ControlledKnob::Param(spin_whole_arm_param()),
            ],
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

    pub fn set_lower_arm_and_spin_whole(&mut self, lower_arm: f32, spin_whole: f32) -> bool {
        self.set_controlled_knobs(
            ControlledKnob::Param(lower_arm_param()),
            ControlledKnob::Param(spin_whole_arm_param()),
        );
        self.set_param_pair(
            lower_arm_param(),
            lower_arm,
            spin_whole_arm_param(),
            spin_whole,
        )
    }

    pub fn set_controlled_knobs(&mut self, first: ControlledKnob, second: ControlledKnob) {
        self.controlled_knobs = [first, second];
    }

    pub fn set_param_pair(
        &mut self,
        first_index: usize,
        first_value: f32,
        second_index: usize,
        second_value: f32,
    ) -> bool {
        assert!(first_index < DOF, "first_index out of range");
        assert!(second_index < DOF, "second_index out of range");

        let first_value = first_value.clamp(0.0, 1.0);
        let second_value = second_value.clamp(0.0, 1.0);

        let mut changed = false;
        if self.params[first_index] != first_value {
            self.params[first_index] = first_value;
            changed = true;
        }
        if self.params[second_index] != second_value {
            self.params[second_index] = second_value;
            changed = true;
        }

        changed
    }

    /// Set the base joint params that control the view orientation.
    ///
    /// `z_mix` maps directly to base pitch (-45..45 degrees), `xy_mix` to base yaw rotation.
    pub fn set_view_mixes(&mut self, z_mix: f32, xy_mix: f32) -> bool {
        let z_mix = z_mix.clamp(0.0, 1.0);
        let xy_mix = xy_mix.clamp(0.0, 1.0);

        let mut changed = false;
        let base_pitch_param = base_pitch_param();
        let base_yaw_param = base_yaw_param();

        if self.params[base_pitch_param] != z_mix {
            self.params[base_pitch_param] = z_mix;
            changed = true;
        }
        if self.params[base_yaw_param] != xy_mix {
            self.params[base_yaw_param] = xy_mix;
            changed = true;
        }

        changed
    }

    fn knob_fill_style(&self, knob: ControlledKnob) -> PrimitiveStyle<Rgb565> {
        if self.controlled_knobs[0] == knob || self.controlled_knobs[1] == knob {
            GREEN_FILL_STYLE
        } else {
            YELLOW_FILL_STYLE
        }
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
            randomize_target_params(&mut self.params, self.target_seed);
            self.active_control = None;
            return;
        }
        if matches!(self.active_control, Some(ActiveControl::NextTarget)) {
            self.clear_reverse_kinematics();
            self.target_seed = self.target_seed.wrapping_add(1);
            randomize_target_params(&mut self.params, self.target_seed);
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
            self.reverse_kinematics_run = Some(ReverseKinematicsRun::new(&self.params));
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

    /// Run a no-allocation local reverse-kinematics search over the six arm parameters.
    ///
    /// This tries each arm parameter in both directions, keeps improvements, and
    /// shrinks the step when stuck.
    pub fn reverse_kinematics(&mut self) -> f32 {
        reverse_kinematics(&mut self.params)
    }

    pub fn target_distance(&self) -> f32 {
        compute_target_distance(&self.params)
    }

    fn update_touch(&mut self, x: f32, y: f32) {
        let Some(active_control) = self.active_control else {
            return;
        };

        match active_control {
            ActiveControl::RightSlider(param_index) => {
                self.clear_reverse_kinematics();
                let value = ((x - SLIDER_TRACK_LEFT as f32)
                    / (SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32)
                    .clamp(0.0, 1.0);
                self.params[param_index] = value;
            }
            ActiveControl::Tilt => {
                self.params[base_pitch_param()] =
                    (1.0 - (y - TILT_TOP as f32) / (TILT_BOTTOM - TILT_TOP) as f32).clamp(0.0, 1.0);
            }
            ActiveControl::Zoom => {
                self.zoom =
                    (1.0 - (y - ZOOM_TOP as f32) / (ZOOM_BOTTOM - ZOOM_TOP) as f32).clamp(0.0, 1.0);
            }
            ActiveControl::XyView => {
                self.params[base_yaw_param()] = ((x - VIEW_SLIDER_LEFT as f32)
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

    fn draw_linkage(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        for draw_item in LINKAGE.draw_items(&self.params) {
            match draw_item {
                DrawItem::Stroke(segment) => {
                    if segment.width() == 0 {
                        continue;
                    }
                    let start = self.pose_to_screen(segment.start());
                    let end = self.pose_to_screen(segment.end());
                    let width = zoomed_pixels(segment.width() as u32, self.zoom).max(1);
                    let color = rgb565_from_rgb888(segment.color());
                    Line::new(start, end)
                        .into_styled(PrimitiveStyle::with_stroke(color, width))
                        .draw(buffer)
                        .ok();
                }
                DrawItem::Disk(disk) => {
                    self.draw_projected_disk(buffer, disk.pose(), disk.radius(), disk.color());
                }
                DrawItem::Ring(ring) => {
                    self.draw_projected_ring(
                        buffer,
                        ring.pose(),
                        ring.radius(),
                        ring.width(),
                        ring.color(),
                    );
                }
            }
        }
    }

    /// Draw a filled disk projected through the pose orientation onto the screen.
    ///
    /// Projects the disk's 3D axes (orientation columns 0 and 1) to 2D and rasterizes
    /// the resulting ellipse pixel-by-pixel so that tilted disks appear as ellipses.
    fn draw_projected_disk(
        &self,
        target: &mut impl DrawTarget<Color = Rgb565>,
        pose: Pose,
        radius: f32,
        color_raw: Rgb888,
    ) {
        let orient = pose.orientation();
        let Vec3([px, py, _]) = pose.position();
        let scale = self.scale();
        let r = radius * scale;
        let color = rgb565_from_rgb888(color_raw);

        let cx = round_to_i32(px * scale) + SCREEN_WIDTH as i32 / 2;
        let cy = SCREEN_HEIGHT as i32 / 2 - round_to_i32(py * scale);

        // Project disk semi-axes (orientation columns 0 and 1) to screen space.
        // Screen y is flipped relative to world y, hence the negation on ay/by.
        let ax = orient[0][0] * r;
        let ay = -orient[1][0] * r;
        let bx = orient[0][1] * r;
        let by = -orient[1][1] * r;

        let det = ax * by - bx * ay;
        let det_sq = det * det;

        if det_sq < 0.25 {
            return; // edge-on: disk too thin to rasterize
        }

        let hw = libm::sqrtf(ax * ax + bx * bx) as i32 + 1;
        let hh = libm::sqrtf(ay * ay + by * by) as i32 + 1;

        let x0 = (cx - hw).max(0);
        let y0 = (cy - hh).max(0);
        let x1 = (cx + hw).min(SCREEN_WIDTH as i32 - 1);
        let y1 = (cy + hh).min(SCREEN_HEIGHT as i32 - 1);

        target
            .draw_iter((y0..=y1).flat_map(move |y| {
                (x0..=x1).filter_map(move |x| {
                    let dx = x as f32 - cx as f32;
                    let dy = y as f32 - cy as f32;
                    let u = by * dx - bx * dy;
                    let v = ax * dy - ay * dx;
                    if u * u + v * v <= det_sq {
                        Some(Pixel(Point::new(x, y), color))
                    } else {
                        None
                    }
                })
            }))
            .ok();
    }

    fn draw_projected_ring(
        &self,
        target: &mut impl DrawTarget<Color = Rgb565>,
        pose: Pose,
        radius: f32,
        width: u16,
        color_raw: Rgb888,
    ) {
        let orient = pose.orientation();
        let Vec3([px, py, _]) = pose.position();
        let scale = self.scale();
        let r = radius * scale;
        let color = rgb565_from_rgb888(color_raw);
        let half_w = width as f32 * 0.5 * scale;

        let cx = round_to_i32(px * scale) + SCREEN_WIDTH as i32 / 2;
        let cy = SCREEN_HEIGHT as i32 / 2 - round_to_i32(py * scale);

        let ax = orient[0][0] * r;
        let ay = -orient[1][0] * r;
        let bx = orient[0][1] * r;
        let by = -orient[1][1] * r;

        let det = ax * by - bx * ay;
        let det_sq = det * det;

        if det_sq < 0.25 || r <= 0.0 {
            return;
        }

        let outer_scale_sq = ((r + half_w) / r) * ((r + half_w) / r);
        let inner_scale_sq = if r > half_w {
            ((r - half_w) / r) * ((r - half_w) / r)
        } else {
            0.0
        };

        let outer_scale = (r + half_w) / r;
        let hw = (libm::sqrtf(ax * ax + bx * bx) * outer_scale) as i32 + 2;
        let hh = (libm::sqrtf(ay * ay + by * by) * outer_scale) as i32 + 2;

        let x0 = (cx - hw).max(0);
        let y0 = (cy - hh).max(0);
        let x1 = (cx + hw).min(SCREEN_WIDTH as i32 - 1);
        let y1 = (cy + hh).min(SCREEN_HEIGHT as i32 - 1);

        target
            .draw_iter((y0..=y1).flat_map(move |y| {
                (x0..=x1).filter_map(move |x| {
                    let dx = x as f32 - cx as f32;
                    let dy = y as f32 - cy as f32;
                    let u = by * dx - bx * dy;
                    let v = ax * dy - ay * dx;
                    let dist_sq = u * u + v * v;
                    if dist_sq <= det_sq * outer_scale_sq && dist_sq > det_sq * inner_scale_sq {
                        Some(Pixel(Point::new(x, y), color))
                    } else {
                        None
                    }
                })
            }))
            .ok();
    }

    fn draw_sliders(&self, buffer: &mut impl DrawTarget<Color = Rgb565>) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        let mut target_label = TargetLabel::new();

        // z (base pitch) slider
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
        let tilt_knob_y = TILT_TOP
            + round_to_i32(
                (TILT_BOTTOM - TILT_TOP) as f32 * (1.0 - self.params[base_pitch_param()]),
            );
        Circle::with_center(Point::new(TILT_X, tilt_knob_y), 9)
            .into_styled(self.knob_fill_style(ControlledKnob::Param(base_pitch_param())))
            .draw(buffer)
            .ok();

        // zoom slider (disconnected — shown in gray)
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
            .into_styled(GRAY_FILL_STYLE)
            .draw(buffer)
            .ok();

        self.draw_reverse_kinematics_run_button(buffer);
        self.draw_reverse_kinematics_step_button(buffer);
        self.draw_calibrate_button(buffer);

        // target prev/next/label
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

        // 6 arm param sliders (right side)
        for slider_offset in 0..ARM_PARAM_COUNT {
            let param_index = ARM_PARAM_START + slider_offset;
            let slider_y = SLIDER_TOP + slider_offset as i32 * SLIDER_STEP;
            let value = self.params[param_index];

            Text::with_baseline(
                LINKAGE.param_name(param_index),
                Point::new(SLIDER_LEFT, slider_y - 12),
                text_style,
                Baseline::Top,
            )
            .draw(buffer)
            .ok();

            Line::new(
                Point::new(SLIDER_TRACK_LEFT, slider_y + 8),
                Point::new(SLIDER_RIGHT, slider_y + 8),
            )
            .into_styled(SLIDER_TRACK_STYLE)
            .draw(buffer)
            .ok();

            let knob_x =
                SLIDER_TRACK_LEFT + round_to_i32((SLIDER_RIGHT - SLIDER_TRACK_LEFT) as f32 * value);
            Circle::with_center(Point::new(knob_x, slider_y + 8), 9)
                .into_styled(self.knob_fill_style(ControlledKnob::Param(param_index)))
                .draw(buffer)
                .ok();
        }

        // x/y view (base yaw) slider
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
            + round_to_i32(
                (VIEW_SLIDER_RIGHT - VIEW_SLIDER_LEFT) as f32 * self.params[base_yaw_param()],
            );
        Circle::with_center(Point::new(view_knob_x, VIEW_SLIDER_Y), 9)
            .into_styled(self.knob_fill_style(ControlledKnob::Param(base_yaw_param())))
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
        let Vec3([x, y, _z]) = pose.position();
        self.xy_to_screen(x, y)
    }

    fn xy_to_screen(&self, x: f32, y: f32) -> Point {
        let scale = self.scale();
        Point::new(
            round_to_i32(x * scale) + SCREEN_WIDTH as i32 / 2,
            SCREEN_HEIGHT as i32 / 2 - round_to_i32(y * scale),
        )
    }

    fn scale(&self) -> f32 {
        zoom_to_scale(self.zoom) * PIXELS_PER_UNIT
    }
}

impl Default for CydSim {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum ActiveControl {
    RightSlider(usize), // absolute param index
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
struct ReverseKinematicsRun {
    search_params: [f32; DOF],
    best_params: [f32; DOF],
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
    fn new(params: &[f32; DOF]) -> Self {
        Self {
            search_params: *params,
            best_params: *params,
            best_distance: compute_target_distance(params),
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

                    if self.candidate_index >= ARM_PARAM_COUNT {
                        let bend_elbow_param = bend_elbow_param();
                        let spin_whole_arm_param = spin_whole_arm_param();
                        let bend_original = self.search_params[bend_elbow_param];
                        let spin_original = self.search_params[spin_whole_arm_param];
                        let pair_index = self.candidate_index - ARM_PARAM_COUNT;
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

                    let param_index = ARM_PARAM_START + self.candidate_index;
                    let original = self.search_params[param_index];
                    let high = (original + self.step).min(1.0);
                    if high != original {
                        self.search_params[param_index] = high;
                        self.phase = ReverseKinematicsPhase::EvaluateSingleHigh {
                            index: param_index,
                            original,
                        };
                        return true;
                    }

                    self.phase = ReverseKinematicsPhase::EvaluateSingleHigh {
                        index: param_index,
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
                        self.search_params[bend_elbow_param()] = bend_original;
                        self.search_params[spin_whole_arm_param()] = spin_original;
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
        let distance = compute_target_distance(&self.search_params);
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

fn move_params_toward(
    params: &mut [f32; DOF],
    target_params: &[f32; DOF],
    max_change: f32,
) -> bool {
    let mut moved = false;

    for param_index in ARM_PARAM_START..(ARM_PARAM_START + ARM_PARAM_COUNT) {
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

fn apply_paired_candidate(params: &mut [f32; DOF], pair_index: usize, step: f32) -> bool {
    let (bend_direction, spin_direction) = RK_PAIRED_CANDIDATES[pair_index];
    let bend_elbow_param = bend_elbow_param();
    let spin_whole_arm_param = spin_whole_arm_param();
    let bend_original = params[bend_elbow_param];
    let spin_original = params[spin_whole_arm_param];

    params[bend_elbow_param] = (bend_original + bend_direction * step).clamp(0.0, 1.0);
    params[spin_whole_arm_param] = (spin_original + spin_direction * step).clamp(0.0, 1.0);

    params[bend_elbow_param] != bend_original || params[spin_whole_arm_param] != spin_original
}

fn arm_tip(params: &[f32; DOF]) -> Vec3 {
    let mut arm_params = [0.0f32; 8];
    arm_params.copy_from_slice(&params[..8]);
    ARM_LINKAGE.final_pose(&arm_params).position()
}

fn target_center(params: &[f32; DOF]) -> Vec3 {
    LINKAGE.final_pose(params).position()
}

fn compute_target_distance(params: &[f32; DOF]) -> f32 {
    distance(arm_tip(params), target_center(params))
}

fn reverse_kinematics(params: &mut [f32; DOF]) -> f32 {
    let mut best_distance = compute_target_distance(params);
    let mut step = RK_INITIAL_STEP;

    while step >= RK_MIN_STEP {
        let mut improved = false;

        for candidate_index in 0..ARM_PARAM_COUNT {
            let param_index = ARM_PARAM_START + candidate_index;
            let original = params[param_index];

            let high = (original + step).min(1.0);
            if high != original {
                params[param_index] = high;
                let distance = compute_target_distance(params);
                if distance < best_distance {
                    best_distance = distance;
                    improved = true;
                    continue;
                }
            }

            let low = (original - step).max(0.0);
            if low != original {
                params[param_index] = low;
                let distance = compute_target_distance(params);
                if distance < best_distance {
                    best_distance = distance;
                    improved = true;
                    continue;
                }
            }

            params[param_index] = original;
        }

        for pair_index in 0..RK_PAIRED_CANDIDATES.len() {
            let bend_elbow_param = bend_elbow_param();
            let spin_whole_arm_param = spin_whole_arm_param();
            let bend_original = params[bend_elbow_param];
            let spin_original = params[spin_whole_arm_param];
            if !apply_paired_candidate(params, pair_index, step) {
                continue;
            }

            let distance = compute_target_distance(params);
            if distance < best_distance {
                best_distance = distance;
                improved = true;
            } else {
                params[bend_elbow_param] = bend_original;
                params[spin_whole_arm_param] = spin_original;
            }
        }

        if !improved {
            step *= 0.5;
        }
    }

    best_distance
}

fn randomize_target_params(params: &mut [f32; DOF], seed: u8) {
    let mut rng = WyRand::new_seed(u64::from(seed));
    for param in params[TARGET_PARAM_START..].iter_mut() {
        *param = random_fraction(&mut rng);
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
    for slider_offset in 0..ARM_PARAM_COUNT {
        let slider_y = SLIDER_TOP + slider_offset as i32 * SLIDER_STEP;
        if x >= SLIDER_LEFT as f32 && (y - (slider_y + 8) as f32).abs() <= 13.0 {
            return Some(ActiveControl::RightSlider(ARM_PARAM_START + slider_offset));
        }
    }
    None
}

fn zoom_to_scale(zoom: f32) -> f32 {
    0.5 + zoom
}

fn zoomed_pixels(base_pixels: u32, zoom: f32) -> u32 {
    round_to_u32(base_pixels as f32 * zoom_to_scale(zoom)).max(1)
}

fn rgb565_from_rgb888(color: Rgb888) -> Rgb565 {
    Rgb565::from(color)
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

fn random_fraction(rng: &mut WyRand) -> f32 {
    rng.generate::<u32>() as f32 / (u32::MAX as f32 + 1.0)
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
        target.clear(Rgb565::BLACK)?;
        self.draw_linkage(target);
        self.draw_sliders(target);
        self.draw_report(target);
        self.draw_version(target);
        self.draw_fps(target);
        self.draw_touch_cursor(target);
        Ok(())
    }
}

pub struct FrameBuffer {
    pixels: [u16; SCREEN_PIXELS],
}

impl FrameBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: [0; SCREEN_PIXELS],
        }
    }

    pub fn static_new() -> &'static mut Self {
        static FRAME_BUFFER: StaticCell<FrameBuffer> = StaticCell::new();
        FRAME_BUFFER.init_with(FrameBuffer::new)
    }

    pub fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color.into_storage());
    }

    pub fn raw_pixels_mut(&mut self) -> &mut [u16; SCREEN_PIXELS] {
        &mut self.pixels
    }

    #[must_use]
    pub fn raw_pixels(&self) -> &[u16; SCREEN_PIXELS] {
        &self.pixels
    }
}

impl DrawTarget for FrameBuffer {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }

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
            self.pixels[y * SCREEN_WIDTH + x] = color.into_storage();
        }
        Ok(())
    }
}

impl OriginDimensions for FrameBuffer {
    fn size(&self) -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CydSim, RK_MAX_TICK_SECONDS, RK_SINGLE_STEP_VISIBLE_PARAM_STEP,
        RK_VISIBLE_PARAM_POINTS_PER_SECOND, reverse_kinematics_visible_param_step,
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
        for _ in 0..100 {
            sim.step_reverse_kinematics();
        }
        let after = sim.target_distance();

        assert!(after <= before, "expected {after} <= {before}");
    }

    #[test]
    fn test_visible_param_step_at_zero_dt() {
        let step = reverse_kinematics_visible_param_step(0.0);
        assert_eq!(step, 0.0);
    }

    #[test]
    fn test_visible_param_step_at_max_dt() {
        let step = reverse_kinematics_visible_param_step(1.0);
        let expected = RK_MAX_TICK_SECONDS * RK_VISIBLE_PARAM_POINTS_PER_SECOND;
        assert!(
            (step - expected).abs() < 0.001,
            "expected {expected}, got {step}"
        );
    }

    #[test]
    fn test_single_step_visible_param_step() {
        assert!(
            RK_SINGLE_STEP_VISIBLE_PARAM_STEP > 0.0,
            "single-step amount must be positive"
        );
    }
}
