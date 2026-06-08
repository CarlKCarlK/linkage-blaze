#![no_std]
#![forbid(unsafe_code)]
#![doc = "No-allocation robot arm simulation and math primitives."]

/// A step in the robot arm linkage description.
///
/// - v0 = tail → nose (forward), v1 = right → left, v2 = belly → back
#[derive(Debug)]
pub enum Step {
    /// Reset to the origin with the identity orientation.
    Start,
    /// Rotate around v2 (belly → back): turn left/right.
    Yaw(f32, &'static Step),
    /// Rotate around v1 (right → left): nose up/down.
    Pitch(f32, &'static Step),
    /// Rotate around v0 (tail → nose): right side down.
    Roll(f32, &'static Step),
    /// Advance along v0 by the given distance.
    Move(f32, &'static Step),
}

impl Step {
    /// Create a yaw step from a user-facing angle in degrees.
    pub const fn yaw(degrees: f32, previous: &'static Step) -> Self {
        Self::Yaw(degrees_to_radians(degrees), previous)
    }

    /// Create a pitch step from a user-facing angle in degrees.
    pub const fn pitch(degrees: f32, previous: &'static Step) -> Self {
        Self::Pitch(degrees_to_radians(degrees), previous)
    }

    /// Create a roll step from a user-facing angle in degrees.
    pub const fn roll(degrees: f32, previous: &'static Step) -> Self {
        Self::Roll(degrees_to_radians(degrees), previous)
    }

    /// Create a move step.
    pub const fn move_forward(distance: f32, previous: &'static Step) -> Self {
        Self::Move(distance, previous)
    }

    fn previous(&self) -> Option<&'static Step> {
        match self {
            Self::Start => None,
            Self::Yaw(_, previous)
            | Self::Pitch(_, previous)
            | Self::Roll(_, previous)
            | Self::Move(_, previous) => Some(previous),
        }
    }
}

/// 3D position [x, y, z].
pub type Vec3 = [f32; 3];

// 3×3 rotation matrix, row-major: mat[row][col].
// Columns are body-frame axes: col 0 = v0 (forward), col 1 = v1 (left), col 2 = v2 (up/back).
type Mat3 = [[f32; 3]; 3];

const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

const fn degrees_to_radians(degrees: f32) -> f32 {
    degrees * (core::f32::consts::PI / 180.0)
}

fn mat_mul(a: Mat3, b: Mat3) -> Mat3 {
    let mut out = [[0.0f32; 3]; 3];
    for row in 0..3 {
        for col in 0..3 {
            for k in 0..3 {
                out[row][col] += a[row][k] * b[k][col];
            }
        }
    }
    out
}

// Rotation matrices match the Excel SWITCH formulas exactly.
// Yaw  = Rz: [[c,-s,0],[s,c,0],[0,0,1]]
// Pitch = Ry (Excel convention): [[c,0,-s],[0,1,0],[s,0,c]]
// Roll  = Rx: [[1,0,0],[0,c,-s],[0,s,c]]
fn rotation_matrix(step: &Step) -> Mat3 {
    let radians = match step {
        Step::Yaw(radians, _) | Step::Pitch(radians, _) | Step::Roll(radians, _) => *radians,
        Step::Start | Step::Move(_, _) => return IDENTITY,
    };
    let c = libm::cosf(radians);
    let s = libm::sinf(radians);
    match step {
        Step::Yaw(_, _) => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
        Step::Pitch(_, _) => [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]],
        Step::Roll(_, _) => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        Step::Start | Step::Move(_, _) => IDENTITY,
    }
}

struct Turtle {
    orientation: Mat3,
    position: Vec3,
}

impl Turtle {
    fn new() -> Self {
        Self {
            orientation: IDENTITY,
            position: [0.0, 0.0, 0.0],
        }
    }

    fn apply(&mut self, step: &Step) {
        match step {
            Step::Start => {
                *self = Self::new();
            }
            Step::Move(dist, _) => {
                // advance along v0 = col 0 of orientation
                self.position[0] += dist * self.orientation[0][0];
                self.position[1] += dist * self.orientation[1][0];
                self.position[2] += dist * self.orientation[2][0];
            }
            _ => {
                self.orientation = mat_mul(self.orientation, rotation_matrix(step));
            }
        }
    }
}

/// Iterator over joint positions produced by simulating a linkage.
///
/// Yields the [`Step::Start`] position `[0,0,0]` and then the position after each [`Step::Move`].
pub struct Simulate<const MAX_STEPS: usize> {
    steps: [&'static Step; MAX_STEPS],
    index: usize,
    turtle: Turtle,
}

impl<const MAX_STEPS: usize> Simulate<MAX_STEPS> {
    /// Create a new simulation iterator for the given final linkage step.
    pub fn new(end: &'static Step) -> Self {
        let mut steps = [end; MAX_STEPS];
        let mut len = 0;
        let mut step = Some(end);
        while let Some(current_step) = step {
            assert!(len < MAX_STEPS, "linkage has more steps than MAX_STEPS");
            steps[len] = current_step;
            len += 1;
            step = current_step.previous();
        }
        Self {
            steps,
            index: len,
            turtle: Turtle::new(),
        }
    }
}

impl<const MAX_STEPS: usize> Iterator for Simulate<MAX_STEPS> {
    type Item = Vec3;

    fn next(&mut self) -> Option<Vec3> {
        loop {
            self.index = self.index.checked_sub(1)?;
            let step = self.steps[self.index];
            self.turtle.apply(step);
            if matches!(step, Step::Start | Step::Move(_, _)) {
                return Some(self.turtle.position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Simulate, Step};

    const LINKAGE_MAX_STEPS: usize = 24;

    static START: Step = Step::Start;
    static STEP_01: Step = Step::yaw(90.0, &START);
    static STEP_02: Step = Step::yaw(-45.15793644, &STEP_01);
    static STEP_03: Step = Step::pitch(90.0, &STEP_02);
    static STEP_04: Step = Step::move_forward(2.5, &STEP_03);
    static STEP_05: Step = Step::pitch(-90.0, &STEP_04);
    static STEP_06: Step = Step::pitch(-6.66134e-15, &STEP_05);
    static STEP_07: Step = Step::move_forward(3.0, &STEP_06);
    static STEP_08: Step = Step::yaw(-0.036069163, &STEP_07);
    static STEP_09: Step = Step::move_forward(3.0, &STEP_08);
    static STEP_10: Step = Step::pitch(-45.26102633, &STEP_09);
    static STEP_11: Step = Step::move_forward(1.0, &STEP_10);
    static STEP_12: Step = Step::roll(180.0, &STEP_11);
    static STEP_13: Step = Step::move_forward(0.5, &STEP_12);
    static STEP_14: Step = Step::yaw(90.0, &STEP_13);
    static STEP_15: Step = Step::move_forward(0.249940215, &STEP_14);
    static STEP_16: Step = Step::yaw(-90.0, &STEP_15);
    static STEP_17: Step = Step::move_forward(1.0, &STEP_16);
    static STEP_18: Step = Step::yaw(180.0, &STEP_17);
    static STEP_19: Step = Step::move_forward(1.0, &STEP_18);
    static STEP_20: Step = Step::yaw(90.0, &STEP_19);
    static STEP_21: Step = Step::move_forward(0.49988043, &STEP_20);
    static STEP_22: Step = Step::yaw(90.0, &STEP_21);
    static STEP_23: Step = Step::move_forward(1.0, &STEP_22);
    const LINKAGE: &Step = &STEP_23;

    #[test]
    fn test_linkage_structure() {
        assert!(matches!(START, Step::Start));
        assert!(matches!(STEP_12, Step::Roll(_, _)));
        assert!(matches!(STEP_23, Step::Move(1.0, _)));
    }

    #[test]
    fn test_simulate_yields_initial_position() {
        let first = Simulate::<LINKAGE_MAX_STEPS>::new(LINKAGE).next().unwrap();
        assert_vec3_approx_eq(first, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_simulate_position_count() {
        // 10 Move steps → 11 positions (initial + one per Move)
        assert_eq!(Simulate::<LINKAGE_MAX_STEPS>::new(LINKAGE).count(), 11);
    }

    #[test]
    fn test_simulate_first_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(1), [0.0, 0.0, 2.5]);
    }

    #[test]
    fn test_simulate_second_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(2), [2.12716, 2.115, 2.5]);
    }

    #[test]
    fn test_simulate_third_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(3), [4.25565, 4.23, 2.5]);
    }

    #[test]
    fn test_simulate_fourth_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(4), [4.75565, 4.726, 1.79]);
    }

    #[test]
    fn test_simulate_fifth_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(5), [5.00475, 4.974, 1.435]);
    }

    #[test]
    fn test_simulate_last_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(10), [5.32801, 5.647, 0.724]);
    }

    fn assert_vec3_approx_eq(actual: [f32; 3], expected: [f32; 3]) {
        let close_enough = actual
            .iter()
            .zip(expected.iter())
            .all(|(x, y)| (x - y).abs() < 1e-3);
        assert!(
            close_enough,
            "expected ({:.5},{:.5},{:.5}), got ({:.5},{:.5},{:.5})",
            expected[0], expected[1], expected[2], actual[0], actual[1], actual[2]
        );
    }

    fn position_after_move(move_index: usize) -> [f32; 3] {
        Simulate::<LINKAGE_MAX_STEPS>::new(LINKAGE)
            .nth(move_index)
            .unwrap()
    }
}
