#![no_std]
#![forbid(unsafe_code)]
#![doc = "No-allocation robot arm simulation and math primitives."]

/// A step in the robot arm linkage description. Angle arguments are in degrees.
///
/// - v0 = tail → nose (forward), v1 = right → left, v2 = belly → back
#[derive(Debug, PartialEq)]
pub enum Step {
    /// Rotate around v2 (belly → back): turn left/right.
    Yaw(f64),
    /// Rotate around v1 (right → left): nose up/down.
    Pitch(f64),
    /// Rotate around v0 (tail → nose): right side down.
    Roll(f64),
    /// Advance along v0 by the given distance.
    Move(f64),
}

/// 3D position [x, y, z].
pub type Vec3 = [f64; 3];

// 3×3 rotation matrix, row-major: mat[row][col].
// Columns are body-frame axes: col 0 = v0 (forward), col 1 = v1 (left), col 2 = v2 (up/back).
type Mat3 = [[f64; 3]; 3];

const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mat_mul(a: Mat3, b: Mat3) -> Mat3 {
    let mut out = [[0.0f64; 3]; 3];
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
    let deg = match step {
        Step::Yaw(a) | Step::Pitch(a) | Step::Roll(a) => *a,
        Step::Move(_) => return IDENTITY,
    };
    let rad = deg * (core::f64::consts::PI / 180.0);
    let c = libm::cos(rad);
    let s = libm::sin(rad);
    match step {
        Step::Yaw(_)   => [[c, -s, 0.0], [s,  c,  0.0], [0.0, 0.0, 1.0]],
        Step::Pitch(_) => [[c, 0.0, -s], [0.0, 1.0, 0.0], [s,  0.0,  c ]],
        Step::Roll(_)  => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0,  s,   c ]],
        Step::Move(_)  => IDENTITY,
    }
}

struct Turtle {
    orientation: Mat3,
    position: Vec3,
}

impl Turtle {
    fn new() -> Self {
        Self { orientation: IDENTITY, position: [0.0, 0.0, 0.0] }
    }

    fn apply(&mut self, step: &Step) {
        match step {
            Step::Move(dist) => {
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
/// Yields the initial position `[0,0,0]` and then the position after each [`Step::Move`].
pub struct Simulate<'a> {
    steps: &'a [Step],
    index: usize,
    turtle: Turtle,
    started: bool,
}

impl<'a> Simulate<'a> {
    /// Create a new simulation iterator for the given linkage steps.
    pub fn new(steps: &'a [Step]) -> Self {
        Self { steps, index: 0, turtle: Turtle::new(), started: false }
    }
}

impl Iterator for Simulate<'_> {
    type Item = Vec3;

    fn next(&mut self) -> Option<Vec3> {
        if !self.started {
            self.started = true;
            return Some(self.turtle.position);
        }
        loop {
            let step = self.steps.get(self.index)?;
            self.index += 1;
            self.turtle.apply(step);
            if matches!(step, Step::Move(_)) {
                return Some(self.turtle.position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Simulate, Step};

    const LINKAGE: &[Step] = &[
        Step::Yaw(90.0),
        Step::Yaw(-45.15793644),
        Step::Pitch(90.0),
        Step::Move(2.5),
        Step::Pitch(-90.0),
        Step::Pitch(-6.66134e-15),
        Step::Move(3.0),
        Step::Yaw(-0.036069163),
        Step::Move(3.0),
        Step::Pitch(-45.26102633),
        Step::Move(1.0),
        Step::Roll(180.0),
        Step::Move(0.5),
        Step::Yaw(90.0),
        Step::Move(0.249940215),
        Step::Yaw(-90.0),
        Step::Move(1.0),
        Step::Yaw(180.0),
        Step::Move(1.0),
        Step::Yaw(90.0),
        Step::Move(0.49988043),
        Step::Yaw(90.0),
        Step::Move(1.0),
    ];

    fn assert_vec3_approx_eq(actual: [f64; 3], expected: [f64; 3]) {
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

    fn position_after_move(move_index: usize) -> [f64; 3] {
        Simulate::new(LINKAGE).nth(move_index).unwrap()
    }

    #[test]
    fn test_linkage_structure() {
        assert_eq!(LINKAGE.len(), 23);
        assert_eq!(LINKAGE[0], Step::Yaw(90.0));
        assert_eq!(LINKAGE[11], Step::Roll(180.0));
        assert_eq!(LINKAGE[22], Step::Move(1.0));
    }

    #[test]
    fn test_simulate_yields_initial_position() {
        let first = Simulate::new(LINKAGE).next().unwrap();
        assert_vec3_approx_eq(first, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_simulate_position_count() {
        // 10 Move steps → 11 positions (initial + one per Move)
        assert_eq!(Simulate::new(LINKAGE).count(), 11);
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
}
