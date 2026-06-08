#![no_std]
#![forbid(unsafe_code)]
#![doc = "No-allocation robot arm simulation and math primitives."]

/// A step in the robot arm linkage description.
///
/// - v0 = tail → nose (forward), v1 = right → left, v2 = belly → back
#[derive(Debug)]
pub enum Step<P> {
    /// Reset to the origin with the identity orientation.
    Start,
    /// Rotate around v2 (belly → back): turn left/right.
    Yaw(Arg<P>),
    /// Rotate around v1 (right → left): nose up/down.
    Pitch(Arg<P>),
    /// Rotate around v0 (tail → nose): right side down.
    Roll(Arg<P>),
    /// Advance along v0 by the given distance.
    Move(Arg<P>),
}

/// Runtime model parameters.
///
/// Angle fields are stored in radians. Distance fields are stored in linkage units.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub lower_hand: f32,
    pub bend_elbow: f32,
    pub close_hand: f32,
    pub lower_arm: f32,
    pub spin_wrist: f32,
    pub spin_hand: f32,
}

impl Params {
    /// Create model parameters from user-facing degree values and distances.
    pub const fn from_degrees(
        lower_hand: f32,
        bend_elbow: f32,
        close_hand: f32,
        lower_arm: f32,
        spin_wrist: f32,
        spin_hand: f32,
    ) -> Self {
        Self {
            lower_hand: degrees_to_radians(lower_hand),
            bend_elbow: degrees_to_radians(bend_elbow),
            close_hand,
            lower_arm: degrees_to_radians(lower_arm),
            spin_wrist: degrees_to_radians(spin_wrist),
            spin_hand: degrees_to_radians(spin_hand),
        }
    }
}

/// A fixed argument or a runtime parameter accessor.
#[derive(Debug)]
pub enum Arg<P> {
    Fixed(f32),
    Param(fn(&P) -> f32),
}

impl<P> Arg<P> {
    fn resolve(&self, params: &P) -> f32 {
        match self {
            Self::Fixed(value) => *value,
            Self::Param(accessor) => accessor(params),
        }
    }
}

/// A fixed-size linkage description.
pub struct Linkage<const N: usize, P> {
    steps: [Step<P>; N],
}

impl<const N: usize, P> Linkage<N, P> {
    /// Start building a fixed-size linkage.
    pub const fn builder() -> LinkageBuilder<N, P> {
        assert!(N > 0, "linkage must have room for the implicit start step");
        LinkageBuilder {
            steps: [const { Step::Start }; N],
            len: 1,
        }
    }

    /// Return the linkage steps in evaluation order.
    #[must_use]
    pub const fn steps(&self) -> &[Step<P>; N] {
        &self.steps
    }

    /// Create a simulation iterator for this linkage.
    pub fn simulate<'a>(&'a self, params: &'a P) -> Simulate<'a, N, P> {
        Simulate::new(self, params)
    }
}

/// Const builder for [`Linkage`].
pub struct LinkageBuilder<const N: usize, P> {
    steps: [Step<P>; N],
    len: usize,
}

impl<const N: usize, P> LinkageBuilder<N, P> {
    /// Add a yaw step from a user-facing angle in degrees.
    pub const fn yaw_deg(self, degrees: f32) -> Self {
        self.push(Step::Yaw(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a yaw step from a runtime parameter.
    pub const fn yaw_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Yaw(Arg::Param(accessor)))
    }

    /// Add a pitch step from a user-facing angle in degrees.
    pub const fn pitch_deg(self, degrees: f32) -> Self {
        self.push(Step::Pitch(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a pitch step from a runtime parameter.
    pub const fn pitch_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Pitch(Arg::Param(accessor)))
    }

    /// Add a roll step from a user-facing angle in degrees.
    pub const fn roll_deg(self, degrees: f32) -> Self {
        self.push(Step::Roll(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a roll step from a runtime parameter.
    pub const fn roll_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Roll(Arg::Param(accessor)))
    }

    /// Add a fixed move step.
    pub const fn move_forward(self, distance: f32) -> Self {
        self.push(Step::Move(Arg::Fixed(distance)))
    }

    /// Add a move step from a runtime parameter.
    pub const fn move_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Move(Arg::Param(accessor)))
    }

    /// Finish building the linkage.
    pub const fn seal(self) -> Linkage<N, P> {
        assert!(self.len == N, "linkage step count must match N");
        Linkage { steps: self.steps }
    }

    const fn push(mut self, step: Step<P>) -> Self {
        assert!(self.len < N, "linkage has more steps than N");
        self.steps[self.len] = step;
        self.len += 1;
        self
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
fn rotation_matrix<P>(step: &Step<P>, params: &P) -> Mat3 {
    let radians = match step {
        Step::Yaw(arg) | Step::Pitch(arg) | Step::Roll(arg) => arg.resolve(params),
        Step::Start | Step::Move(_) => return IDENTITY,
    };
    let cos = libm::cosf(radians);
    let sin = libm::sinf(radians);
    match step {
        Step::Yaw(_) => [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]],
        Step::Pitch(_) => [[cos, 0.0, -sin], [0.0, 1.0, 0.0], [sin, 0.0, cos]],
        Step::Roll(_) => [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]],
        Step::Start | Step::Move(_) => IDENTITY,
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

    fn apply<P>(&mut self, step: &Step<P>, params: &P) {
        match step {
            Step::Start => {
                *self = Self::new();
            }
            Step::Move(arg) => {
                let dist = arg.resolve(params);
                // advance along v0 = col 0 of orientation
                self.position[0] += dist * self.orientation[0][0];
                self.position[1] += dist * self.orientation[1][0];
                self.position[2] += dist * self.orientation[2][0];
            }
            _ => {
                self.orientation = mat_mul(self.orientation, rotation_matrix(step, params));
            }
        }
    }
}

/// Iterator over joint positions produced by simulating a linkage.
///
/// Yields the [`Step::Start`] position `[0,0,0]` and then the position after each [`Step::Move`].
pub struct Simulate<'a, const N: usize, P> {
    linkage: &'a Linkage<N, P>,
    params: &'a P,
    index: usize,
    turtle: Turtle,
}

impl<'a, const N: usize, P> Simulate<'a, N, P> {
    /// Create a new simulation iterator for the given linkage.
    pub fn new(linkage: &'a Linkage<N, P>, params: &'a P) -> Self {
        Self {
            linkage,
            params,
            index: 0,
            turtle: Turtle::new(),
        }
    }
}

impl<const N: usize, P> Iterator for Simulate<'_, N, P> {
    type Item = Vec3;

    fn next(&mut self) -> Option<Vec3> {
        loop {
            let step = self.linkage.steps.get(self.index)?;
            self.index += 1;
            self.turtle.apply(step, self.params);
            if matches!(step, Step::Start | Step::Move(_)) {
                return Some(self.turtle.position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Linkage, Params, Step};

    const LINKAGE_MAX_STEPS: usize = 24;
    const EXCEL_PARAMS: Params = Params::from_degrees(
        -45.15793644,
        -45.26102633,
        0.249940215,
        -0.036069163,
        90.0,
        180.0,
    );

    fn lower_hand(params: &Params) -> f32 {
        params.lower_hand
    }

    fn bend_elbow(params: &Params) -> f32 {
        params.bend_elbow
    }

    fn close_hand(params: &Params) -> f32 {
        params.close_hand
    }

    fn lower_arm(params: &Params) -> f32 {
        params.lower_arm
    }

    fn spin_wrist(params: &Params) -> f32 {
        params.spin_wrist
    }

    fn spin_hand(params: &Params) -> f32 {
        params.spin_hand
    }

    const LINKAGE: Linkage<LINKAGE_MAX_STEPS, Params> = Linkage::builder()
        .yaw_deg(90.0)
        .yaw_param(lower_hand)
        .pitch_deg(90.0)
        .move_forward(2.5)
        .pitch_deg(-90.0)
        .pitch_deg(-6.66134e-15)
        .move_forward(3.0)
        .yaw_param(lower_arm)
        .move_forward(3.0)
        .pitch_param(bend_elbow)
        .move_forward(1.0)
        .roll_deg(180.0)
        .move_forward(0.5)
        .yaw_param(spin_wrist)
        .move_param(close_hand)
        .yaw_deg(-90.0)
        .move_forward(1.0)
        .yaw_param(spin_hand)
        .move_forward(1.0)
        .yaw_deg(90.0)
        .move_forward(0.49988043)
        .yaw_deg(90.0)
        .move_forward(1.0)
        .seal();

    #[test]
    fn test_linkage_structure() {
        assert_eq!(LINKAGE.steps().len(), LINKAGE_MAX_STEPS);
        assert!(matches!(LINKAGE.steps()[0], Step::Start));
        assert!(matches!(LINKAGE.steps()[12], Step::Roll(_)));
        assert!(matches!(LINKAGE.steps()[23], Step::Move(_)));
    }

    #[test]
    fn test_simulate_yields_initial_position() {
        let first = LINKAGE.simulate(&EXCEL_PARAMS).next().unwrap();
        assert_vec3_approx_eq(first, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_simulate_position_count() {
        // 10 Move steps → 11 positions (initial + one per Move)
        assert_eq!(LINKAGE.simulate(&EXCEL_PARAMS).count(), 11);
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
        LINKAGE.simulate(&EXCEL_PARAMS).nth(move_index).unwrap()
    }
}
