//! Reverse-kinematics controller for the armatron example.
//!
//! The solver performs a greedy coordinate search with step decay. It keeps a
//! private search state, spreads candidate evaluation across frames, and
//! interpolates the visible arm params toward the current best solution while a
//! solve is playing.
//!
//! Solver ownership is one-way: the live game loop owns exactly one
//! [`ReverseKinematics`] controller and calls its small API, but the internal
//! search state never leaks back out into `main.rs`. The manual
//! distance-to-target label remains owned by `main.rs`, because it is part of
//! the manual game even when the solver is idle.

use crate::ui::{HoldButtonState, IconButton};

use super::controls::{RK_RUN_BUTTON, RK_STOP_BUTTON};

const INITIAL_STEP: f32 = 0.125;
const MIN_STEP: f32 = 0.001;
const VISIBLE_PARAM_POINTS_PER_SECOND: f32 = 0.6;
const MAX_TICK_SECONDS: f32 = 0.1;
const SINGLE_STEP_VISIBLE_PARAM_STEP: f32 = 0.01;
const SEARCH_CANDIDATES_PER_SECOND: f32 = 36.0;
const HOLD_STEPS_PER_SECOND: f32 = 9.0;
const PAIRED_CANDIDATES: [(f32, f32); 4] = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)];
const CANDIDATE_COUNT: usize = super::ARM_PARAM_INDEXES.len() + PAIRED_CANDIDATES.len();
const BEND_ELBOW_PARAM_INDEX: usize = super::LINKAGE.param_index("bend elbow", 0);
const SPIN_WHOLE_ARM_PARAM_INDEX: usize = super::LINKAGE.param_index("spin whole arm", 0);

/// Per-arm reverse-kinematics controller.
pub(super) struct ReverseKinematics {
    run: Option<Run>,
    playing: bool,
    search_candidate_budget: f32,
    hold_step_budget: f32,
}

const _: fn(&mut ReverseKinematics, &[f32; super::DOF]) = ReverseKinematics::toggle;

impl ReverseKinematics {
    /// Idle controller: no run state, not playing.
    pub(super) const fn new() -> Self {
        Self {
            run: None,
            playing: false,
            search_candidate_budget: 0.0,
            hold_step_budget: 0.0,
        }
    }

    /// Play/stop. Starting seeds the search from the current params.
    pub(super) fn toggle(&mut self, params: &[f32; super::DOF]) {
        self.search_candidate_budget = 0.0;
        self.hold_step_budget = 0.0;

        if self.playing {
            self.playing = false;
            return;
        }

        self.ensure_run(params);
        self.playing = true;
    }

    /// Called once per frame with the step button's hold state.
    pub(super) fn hold_step(
        &mut self,
        params: &mut [f32; super::DOF],
        hold: HoldButtonState,
        dt_seconds: f32,
    ) {
        match hold {
            HoldButtonState::Idle => {
                self.hold_step_budget = 0.0;
            }
            HoldButtonState::Pressed => {
                self.playing = false;
                self.ensure_run(params);
                self.hold_step_budget = 1.0;
                self.consume_hold_steps(params);
            }
            HoldButtonState::Held => {
                let dt_seconds = dt_seconds.clamp(0.0, MAX_TICK_SECONDS);
                self.hold_step_budget += dt_seconds * HOLD_STEPS_PER_SECOND;
                self.consume_hold_steps(params);
            }
        }
    }

    /// Forget the run and stop playing (manual interference, target change).
    pub(super) fn clear(&mut self) {
        self.run = None;
        self.playing = false;
        self.search_candidate_budget = 0.0;
        self.hold_step_budget = 0.0;
    }

    /// Per-frame advance while playing.
    pub(super) fn tick(&mut self, params: &mut [f32; super::DOF], dt_seconds: f32) {
        if !self.playing {
            return;
        }

        let Some(run) = self.run.as_mut() else {
            self.playing = false;
            return;
        };

        let dt_seconds = dt_seconds.clamp(0.0, MAX_TICK_SECONDS);
        self.search_candidate_budget += dt_seconds * SEARCH_CANDIDATES_PER_SECOND;

        while self.search_candidate_budget >= 1.0 {
            self.search_candidate_budget -= 1.0;
            if !run.tick_search_candidate() {
                self.search_candidate_budget = 0.0;
                break;
            }
        }

        let visible_moving = move_params_toward(
            params,
            &run.best_params,
            dt_seconds * VISIBLE_PARAM_POINTS_PER_SECOND,
        );
        if run.search_exhausted && !visible_moving {
            self.playing = false;
        }
    }

    /// The play or stop `IconButton` matching the current playing state.
    pub(super) const fn run_button(&self) -> &'static IconButton {
        if self.playing {
            &RK_STOP_BUTTON
        } else {
            &RK_RUN_BUTTON
        }
    }

    fn ensure_run(&mut self, params: &[f32; super::DOF]) {
        if self.run.is_none() {
            self.run = Some(Run::new(params));
        }
    }

    fn consume_hold_steps(&mut self, params: &mut [f32; super::DOF]) {
        let Some(run) = self.run.as_mut() else {
            self.hold_step_budget = 0.0;
            return;
        };

        while self.hold_step_budget >= 1.0 {
            self.hold_step_budget -= 1.0;
            run.tick_search_candidate();
            let visible_moving =
                move_params_toward(params, &run.best_params, SINGLE_STEP_VISIBLE_PARAM_STEP);
            if run.search_exhausted && !visible_moving {
                self.run = None;
                self.hold_step_budget = 0.0;
                break;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Run {
    search_params: [f32; super::DOF],
    best_params: [f32; super::DOF],
    best_distance: f32,
    step: f32,
    search_exhausted: bool,
    candidate_index: usize,
    sweep_improved: bool,
    phase: Phase,
}

#[derive(Clone, Copy)]
enum Phase {
    BeginCandidate,
    EvaluateSingleHigh { index: usize, original: f32 },
    EvaluateSingleLow { index: usize, original: f32 },
    EvaluatePair { bend_original: f32, spin_original: f32 },
}

impl Run {
    fn new(params: &[f32; super::DOF]) -> Self {
        Self {
            search_params: *params,
            best_params: *params,
            best_distance: super::compute_target_distance(
                super::ARM_TIP_LINKAGE,
                super::LINKAGE,
                params,
            ),
            step: INITIAL_STEP,
            search_exhausted: false,
            candidate_index: 0,
            sweep_improved: false,
            phase: Phase::BeginCandidate,
        }
    }

    fn tick_search_candidate(&mut self) -> bool {
        let candidate_index = self.candidate_index;
        let mut searched = false;

        loop {
            if !self.tick_search() {
                self.search_exhausted = true;
                return searched;
            }

            searched = true;
            if matches!(self.phase, Phase::BeginCandidate) && self.candidate_index != candidate_index
            {
                return true;
            }
        }
    }

    fn tick_search(&mut self) -> bool {
        loop {
            match self.phase {
                Phase::BeginCandidate => {
                    if !self.prepare_next_candidate() {
                        return false;
                    }

                    if self.candidate_index >= super::ARM_PARAM_INDEXES.len() {
                        let bend_original = self.search_params[BEND_ELBOW_PARAM_INDEX];
                        let spin_original = self.search_params[SPIN_WHOLE_ARM_PARAM_INDEX];
                        let pair_index = self.candidate_index - super::ARM_PARAM_INDEXES.len();
                        if apply_paired_candidate(&mut self.search_params, pair_index, self.step) {
                            self.phase = Phase::EvaluatePair {
                                bend_original,
                                spin_original,
                            };
                            return true;
                        }

                        self.finish_candidate();
                        continue;
                    }

                    let index = super::ARM_PARAM_INDEXES[self.candidate_index];
                    let original = self.search_params[index];
                    let high = (original + self.step).min(1.0);
                    if high != original {
                        self.search_params[index] = high;
                        self.phase = Phase::EvaluateSingleHigh { index, original };
                        return true;
                    }

                    self.phase = Phase::EvaluateSingleHigh { index, original };
                }
                Phase::EvaluateSingleHigh { index, original } => {
                    if self.keep_if_improved() {
                        self.finish_candidate();
                        return true;
                    }

                    self.search_params[index] = original;
                    let low = (original - self.step).max(0.0);
                    if low != original {
                        self.search_params[index] = low;
                        self.phase = Phase::EvaluateSingleLow { index, original };
                        return true;
                    }

                    self.finish_candidate();
                    return true;
                }
                Phase::EvaluateSingleLow { index, original } => {
                    if !self.keep_if_improved() {
                        self.search_params[index] = original;
                    }
                    self.finish_candidate();
                    return true;
                }
                Phase::EvaluatePair {
                    bend_original,
                    spin_original,
                } => {
                    if !self.keep_if_improved() {
                        self.search_params[BEND_ELBOW_PARAM_INDEX] = bend_original;
                        self.search_params[SPIN_WHOLE_ARM_PARAM_INDEX] = spin_original;
                    }
                    self.finish_candidate();
                    return true;
                }
            }
        }
    }

    fn prepare_next_candidate(&mut self) -> bool {
        while self.candidate_index >= CANDIDATE_COUNT {
            if self.sweep_improved {
                self.sweep_improved = false;
            } else {
                self.step *= 0.5;
                if self.step < MIN_STEP {
                    return false;
                }
            }
            self.candidate_index = 0;
        }

        true
    }

    fn keep_if_improved(&mut self) -> bool {
        let distance = super::compute_target_distance(
            super::ARM_TIP_LINKAGE,
            super::LINKAGE,
            &self.search_params,
        );
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
        self.phase = Phase::BeginCandidate;
    }
}

fn move_params_toward(
    params: &mut [f32; super::DOF],
    target_params: &[f32; super::DOF],
    max_change: f32,
) -> bool {
    let mut moved = false;

    for param_index in super::ARM_PARAM_INDEXES {
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

fn apply_paired_candidate(
    params: &mut [f32; super::DOF],
    pair_index: usize,
    step: f32,
) -> bool {
    let (bend_direction, spin_direction) = PAIRED_CANDIDATES[pair_index];
    let bend_original = params[BEND_ELBOW_PARAM_INDEX];
    let spin_original = params[SPIN_WHOLE_ARM_PARAM_INDEX];

    params[BEND_ELBOW_PARAM_INDEX] = (bend_original + bend_direction * step).clamp(0.0, 1.0);
    params[SPIN_WHOLE_ARM_PARAM_INDEX] = (spin_original + spin_direction * step).clamp(0.0, 1.0);

    params[BEND_ELBOW_PARAM_INDEX] != bend_original
        || params[SPIN_WHOLE_ARM_PARAM_INDEX] != spin_original
}
