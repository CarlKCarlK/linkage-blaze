#![no_std]
#![forbid(unsafe_code)]
#![doc = "No-allocation robot arm simulation and math primitives."]

//todo000 move some of that global static stuff to be const local.
//todo000 revisit the name Param and Args
//todo000 is the way that access functions are passed into parameters Yaw, etc, good? Can methods be used instead of stand-alone functions?
//todo00 allow splits/DAGs in the models.
//todo00 could have (compile-time?) optimizations that collapse adjacent steps of the same type into one step with a combined angle/distance. Would that be worth it? or even multiple moves if one doesn't have parameters.
//todo00 might be nice to have invisible or colored links, but that would be more turtle than linkage.
//todo00 if we did have colored links RGBA, could use a fluent command.

#[cfg(test)]
extern crate std;

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

    fn resolve_degrees_as_radians(&self, params: &P) -> f32 {
        match self {
            Self::Fixed(value) => *value,
            Self::Param(accessor) => degrees_to_radians(accessor(params)),
        }
    }
}

/// A fixed-size linkage description.
pub struct Linkage<P, const N: usize> {
    steps: [Step<P>; N],
    len: usize,
}

impl<P, const N: usize> Linkage<P, N> {
    /// Start a fixed-size linkage with an implicit origin row.
    pub const fn start() -> Self {
        assert!(N > 0, "linkage must have room for the implicit start step");
        Self {
            steps: [const { Step::Start }; N],
            len: 1,
        }
    }

    /// Return the linkage steps in evaluation order.
    #[must_use]
    pub const fn steps(&self) -> &[Step<P>; N] {
        &self.steps
    }

    /// Return the number of linkage steps, including the implicit start step.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Add a yaw step from a user-facing angle in degrees.
    pub const fn yaw(self, degrees: f32) -> Self {
        self.push(Step::Yaw(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a yaw step from a runtime parameter in degrees.
    pub const fn yaw_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Yaw(Arg::Param(accessor)))
    }

    /// Add a pitch step from a user-facing angle in degrees.
    pub const fn pitch(self, degrees: f32) -> Self {
        self.push(Step::Pitch(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a pitch step from a runtime parameter in degrees.
    pub const fn pitch_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Pitch(Arg::Param(accessor)))
    }

    /// Add a roll step from a user-facing angle in degrees.
    pub const fn roll(self, degrees: f32) -> Self {
        self.push(Step::Roll(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a roll step from a runtime parameter in degrees.
    pub const fn roll_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Roll(Arg::Param(accessor)))
    }

    /// Add a fixed forward move step.
    pub const fn forward(self, distance: f32) -> Self {
        self.push(Step::Move(Arg::Fixed(distance)))
    }

    /// Add a move step from a runtime parameter.
    pub const fn move_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Move(Arg::Param(accessor)))
    }

    const fn push(mut self, step: Step<P>) -> Self {
        assert!(self.len < N, "linkage has more steps than N");
        self.steps[self.len] = step;
        self.len += 1;
        self
    }

    /// Iterate over poses produced by evaluating this linkage.
    pub fn poses<'a>(&'a self, params: &'a P) -> Poses<'a, P, N> {
        Poses::new(self, params)
    }

    /// Return the pose produced after evaluating all steps in this linkage.
    ///
    /// This always returns a [`Pose`]. A [`Linkage`] contains an implicit start
    /// step, so the pose sequence is never empty.
    #[must_use]
    pub fn final_pose(&self, params: &P) -> Pose {
        self.poses(params)
            .last()
            .expect("linkage must yield at least the implicit start pose")
    }
}

/// 3D position [x, y, z].
pub type Vec3 = [f32; 3];

/// 3×3 rotation matrix, row-major: mat[row][col].
///
/// Columns are body-frame axes: col 0 = v0 (forward), col 1 = v1 (left), col 2 = v2 (up/back).
pub type Mat3 = [[f32; 3]; 3];

const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

const fn degrees_to_radians(degrees: f32) -> f32 {
    degrees * (core::f32::consts::PI / 180.0)
}

//todo0000 make inline? and elsewhere?
fn f32_is_close_to(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
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
//todo0000 address this non-standardness (fix excel?)
// Pitch = Ry: [[c,0,s],[0,1,0],[-s,0,c]]
// Roll  = Rx: [[1,0,0],[0,c,-s],[0,s,c]]
fn rotation_matrix<P>(step: &Step<P>, params: &P) -> Mat3 {
    let radians = match step {
        Step::Yaw(arg) | Step::Pitch(arg) | Step::Roll(arg) => {
            arg.resolve_degrees_as_radians(params)
        }
        Step::Start | Step::Move(_) => return IDENTITY,
    };
    let cos = libm::cosf(radians);
    let sin = libm::sinf(radians);
    match step {
        Step::Yaw(_) => [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]],
        Step::Pitch(_) => [[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]],
        Step::Roll(_) => [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]],
        Step::Start | Step::Move(_) => IDENTITY,
    }
}

/// Full pose after evaluating a linkage step.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub orientation: Mat3,
    pub position: Vec3,
}

impl Pose {
    fn start() -> Self {
        Self {
            orientation: IDENTITY,
            position: [0.0, 0.0, 0.0],
        }
    }

    /// Return true when all orientation and position components are within `tolerance`.
    #[must_use]
    pub fn is_close_to(&self, other: &Self, tolerance: f32) -> bool {
        mat3_is_close_to(self.orientation, other.orientation, tolerance)
            && vec3_is_close_to(self.position, other.position, tolerance)
    }

    fn apply<P>(&mut self, step: &Step<P>, params: &P) {
        match step {
            Step::Start => {
                *self = Self::start();
            }
            Step::Move(arg) => {
                let dist = arg.resolve(params);
                // advance along v0 = col 0 of orientation
                //todo000 can we define Vec3 and mat3 operations?
                self.position[0] += dist * self.orientation[0][0];
                self.position[1] += dist * self.orientation[1][0];
                self.position[2] += dist * self.orientation[2][0];
            }
            _ => {
                //todo000 can we define Vec3 and mat3 operations?
                self.orientation = mat_mul(self.orientation, rotation_matrix(step, params));
            }
        }
    }
}

//todo0000 why free functions?
fn vec3_is_close_to(a: Vec3, b: Vec3, tolerance: f32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(left, right)| f32_is_close_to(*left, *right, tolerance))
}

//todo0000 why free functions?
fn mat3_is_close_to(a: Mat3, b: Mat3, tolerance: f32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(left, right)| vec3_is_close_to(*left, *right, tolerance))
}

/// Iterator over poses produced by evaluating a linkage.
///
/// Yields one [`Pose`] after every linkage step, including the implicit [`Step::Start`].
pub struct Poses<'a, P, const N: usize> {
    linkage: &'a Linkage<P, N>,
    params: &'a P,
    index: usize,
    pose: Pose,
}

impl<'a, P, const N: usize> Poses<'a, P, N> {
    /// Create a new pose iterator for the given linkage.
    pub fn new(linkage: &'a Linkage<P, N>, params: &'a P) -> Self {
        Self {
            linkage,
            params,
            index: 0,
            pose: Pose::start(),
        }
    }
}

impl<P, const N: usize> Iterator for Poses<'_, P, N> {
    type Item = Pose;

    fn next(&mut self) -> Option<Pose> {
        if self.index >= self.linkage.len {
            return None;
        }
        let step = &self.linkage.steps[self.index];
        self.index += 1;
        self.pose.apply(step, self.params);
        Some(self.pose)
    }
}

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests {
    use super::{Linkage, Pose, Step};
    use crate::test_helpers::{
        assert_params_approx_eq, assert_png_matches_expected, assert_pose_approx_eq,
        assert_pose_trace_matches_expected, draw_linkage_xy_canvas,
    };
    use std::{boxed::Box, error::Error};

    const LINKAGE0: Linkage<[f32; 6], 24> = Linkage::start()
        .yaw(90.0)
        // params[4]: spin whole arm, -180 to +180 degrees.
        .yaw_param(|params: &[f32; 6]| params[4])
        .pitch(-90.0)
        .forward(2.5)
        .pitch(90.0)
        // params[3]: lower arm, 0 to 30 degrees. Negated to match model pitch direction.
        .pitch_param(|params: &[f32; 6]| -params[3])
        .forward(3.0)
        // params[1]: bend elbow, -90 to +90 degrees.
        .yaw_param(|params: &[f32; 6]| params[1])
        .forward(3.0)
        // params[0]: lower hand, -90 to +90 degrees. Negated to match model pitch direction.
        .pitch_param(|params: &[f32; 6]| -params[0])
        .forward(1.0)
        // params[5]: spin hand, -180 to +180 degrees.
        .roll_param(|params: &[f32; 6]| params[5])
        .forward(0.5)
        .yaw(90.0)
        // params[2]: close hand, scaled to 0 to 0.5 linkage units.
        .move_param(|params: &[f32; 6]| params[2] * 0.5)
        .yaw(-90.0)
        .forward(1.0)
        .yaw(180.0)
        .forward(1.0)
        .yaw(90.0)
        // params[2]: close hand, 0 to 1 linkage units.
        .move_param(|params: &[f32; 6]| params[2])
        .yaw(90.0)
        .forward(1.0);

    const LINKAGE1: Linkage<[f32; 3], 16> = Linkage::start()
        .yaw(90.0)
        // params[0]: spin whole arm, -180 to +180 degrees.
        .yaw_param(|params: &[f32; 3]| params[0])
        .forward(3.0)
        // params[1]: bend elbow, -90 to +90 degrees.
        .yaw_param(|params: &[f32; 3]| params[1])
        .forward(3.0)
        .yaw(90.0)
        // params[2]: close hand, scaled to 0 to 0.5 linkage units.
        .move_param(|params: &[f32; 3]| params[2] * 0.5)
        .yaw(-90.0)
        .forward(1.0)
        .yaw(-180.0)
        .forward(1.0)
        .yaw(90.0)
        // params[2]: close hand, 0 to 1 linkage units. A value of 1 is fully closed.
        .move_param(|params: &[f32; 3]| params[2])
        .yaw(90.0)
        .forward(1.0);

    // [Compile-time Test]
    // Force core linkage shape errors to fail compilation instead of a runtime test.
    const _: () = {
        assert!(LINKAGE0.len() == 24);
        match &LINKAGE0.steps()[0] {
            Step::Start => {}
            _ => panic!("expected start step"),
        }
        match &LINKAGE0.steps()[12] {
            Step::Roll(_) => {}
            _ => panic!("expected roll step"),
        }
        match &LINKAGE0.steps()[23] {
            Step::Move(_) => {}
            _ => panic!("expected move step"),
        }

        assert!(LINKAGE1.len() == 16);
        match &LINKAGE1.steps()[0] {
            Step::Start => {}
            _ => panic!("expected start step"),
        }
        match &LINKAGE1.steps()[2] {
            Step::Yaw(_) => {}
            _ => panic!("expected yaw step"),
        }
        match &LINKAGE1.steps()[15] {
            Step::Move(_) => {}
            _ => panic!("expected move step"),
        }
    };

    #[test]
    fn test_excel_pose_trace0_matches_expected() -> Result<(), Box<dyn Error>> {
        // [lower hand degrees, bend elbow degrees, close hand distance,
        //  lower arm degrees, spin whole arm degrees, spin hand degrees]
        let params = [-45.26102633, -0.036069163, 0.5, 0.0, -45.15793644, 180.0];
        assert_pose_trace_matches_expected("excel_pose_trace0.csv", LINKAGE0.poses(&params))
    }

    #[test]
    fn test_excel_pose_trace1_matches_expected() -> Result<(), Box<dyn Error>> {
        // [spin whole arm degrees, bend elbow degrees, close hand distance]
        let params = [72.0, 86.4, 0.9];
        assert_pose_trace_matches_expected("excel_pose_trace1.csv", LINKAGE1.poses(&params))
    }

    #[test]
    fn test_fraction_setting0_matches_excel_final_pose() -> Result<(), Box<dyn Error>> {
        //todo00000 yikes, this is way too ugly.
        let fractions = [
            0.7514501463, // lower hand
            0.49,         // bend elbow
            0.50011957,   // close hand
            1.0,          // lower arm
            0.6254387123, // spin whole arm
            1.0,          // spin hand
        ];
        // Angle fractions match the spreadsheet: 0 maps to max and 1 maps to min.
        // Distance fraction maps normally from 0.0 to 1.0 linkage units.
        let params0 = [
            90.0 + fractions[0] * (-90.0 - 90.0), // lower hand, -90 to +90 degrees
            90.0 + fractions[1] * (-90.0 - 90.0), // bend elbow, -90 to +90 degrees
            0.0 + fractions[2] * (1.0 - 0.0),     // close hand, 0 to 1 linkage units
            30.0 + fractions[3] * (0.0 - 30.0),   // lower arm, 0 to 30 degrees
            180.0 + fractions[4] * (-180.0 - 180.0), // spin whole arm, -180 to +180 degrees
            180.0 + fractions[5] * (-180.0 - 180.0), // spin hand, -180 to +180 degrees
        ];

        let pose = LINKAGE0.final_pose(&params0);
        let expected = Pose {
            orientation: [
                [0.483250222, 0.727078899, -0.487673557],
                [0.51177487, -0.686553913, -0.516459299],
                [-0.710320847, 0.0, -0.703878039],
            ],
            position: [5.213220756, 5.747736152, 0.724197882],
        };

        assert_pose_approx_eq(pose, expected);
        Ok(())
    }

    #[test]
    fn test_fraction_setting1_matches_excel_final_pose() -> Result<(), Box<dyn Error>> {
        let fractions = [
            0.30, // spin whole arm
            0.02, // bend elbow
            0.10, // close hand
        ];
        // Angle fractions match the spreadsheet: 0 maps to max and 1 maps to min.
        // Close-hand fraction is inverted: 0 is fully closed and 1 is fully open.
        let params1 = [
            180.0 + fractions[0] * (-180.0 - 180.0), // spin whole arm, -180 to +180 degrees
            90.0 + fractions[1] * (-90.0 - 90.0),    // bend elbow, -90 to +90 degrees
            1.0 + fractions[2] * (0.0 - 1.0),        // close hand, 0 to 1 linkage units
        ];

        let pose = LINKAGE1.final_pose(&params1);
        let expected = Pose {
            orientation: [
                [-0.368124515, 0.929776430, 0.0],
                [-0.929776430, -0.368124515, 0.0],
                [0.0, 0.0, 1.0],
            ],
            position: [-4.744067192, -2.626399040, 0.0],
        };

        assert_pose_approx_eq(pose, expected);
        Ok(())
    }

    #[test]
    fn test_mid_fraction_setting0_matches_excel_final_pose_and_png() -> Result<(), Box<dyn Error>> {
        let fractions = [
            0.5, // lower hand
            0.3, // bend elbow
            1.0, // close hand
            0.5, // lower arm
            0.5, // spin whole arm
            0.5, // spin hand
        ];
        // Angle fractions match the spreadsheet: 0 maps to max and 1 maps to min.
        // Distance fraction maps normally from 0.0 to 1.0 linkage units.
        let params0 = [
            90.0 + fractions[0] * (-90.0 - 90.0), // lower hand, -90 to +90 degrees
            90.0 + fractions[1] * (-90.0 - 90.0), // bend elbow, -90 to +90 degrees
            0.0 + fractions[2] * (1.0 - 0.0),     // close hand, 0 to 1 linkage units
            30.0 + fractions[3] * (0.0 - 30.0),   // lower arm, 0 to 30 degrees
            180.0 + fractions[4] * (-180.0 - 180.0), // spin whole arm, -180 to +180 degrees
            180.0 + fractions[5] * (-180.0 - 180.0), // spin hand, -180 to +180 degrees
        ];

        let pose = LINKAGE0.final_pose(&params0);
        let expected = Pose {
            orientation: [
                [-0.587785252, -0.809016994, 0.0],
                [0.781450409, -0.567756956, -0.258819045],
                [0.209389006, -0.152130018, 0.965925826],
            ],
            position: [-2.82831039, 7.479633205, 4.504161677],
        };

        assert_pose_approx_eq(pose, expected);

        let canvas = draw_linkage_xy_canvas(&LINKAGE0, &params0);
        assert_png_matches_expected("linkage0_xy_mid_fraction.png", &canvas)
    }

    #[test]
    fn test_linkage0_png_matches_expected() -> Result<(), Box<dyn Error>> {
        // [lower hand degrees, bend elbow degrees, close hand distance,
        //  lower arm degrees, spin whole arm degrees, spin hand degrees]
        let params = [-45.26102633, -0.036069163, 0.5, 0.0, -45.15793644, 180.0];

        let canvas = draw_linkage_xy_canvas(&LINKAGE0, &params);
        assert_png_matches_expected("linkage0_xy.png", &canvas)
    }

    #[test]
    fn test_linkage1_png_matches_expected() -> Result<(), Box<dyn Error>> {
        // [spin whole arm degrees, bend elbow degrees, close hand distance]
        let params = [72.0, 86.4, 0.9];

        let canvas = draw_linkage_xy_canvas(&LINKAGE1, &params);
        assert_png_matches_expected("linkage1_xy.png", &canvas)
    }

    #[test]
    fn test_params0_fraction_math_maps_to_ranges() {
        let fractions = [
            0.0,  // lower hand
            0.5,  // bend elbow
            1.0,  // close hand
            1.0,  // lower arm
            0.25, // spin whole arm
            0.75, // spin hand
        ];
        // Angle fractions match the spreadsheet: 0 maps to max and 1 maps to min.
        // Distance fraction maps normally from 0.0 to 1.0 linkage units.
        let params0 = [
            90.0 + fractions[0] * (-90.0 - 90.0), // lower hand, -90 to +90 degrees
            90.0 + fractions[1] * (-90.0 - 90.0), // bend elbow, -90 to +90 degrees
            0.0 + fractions[2] * (1.0 - 0.0),     // close hand, 0 to 1 linkage units
            30.0 + fractions[3] * (0.0 - 30.0),   // lower arm, 0 to 30 degrees
            180.0 + fractions[4] * (-180.0 - 180.0), // spin whole arm, -180 to +180 degrees
            180.0 + fractions[5] * (-180.0 - 180.0), // spin hand, -180 to +180 degrees
        ];

        let expected = [90.0, 0.0, 1.0, 0.0, 90.0, -90.0];
        assert_params_approx_eq(params0, expected);
    }

    #[test]
    fn test_params1_fraction_math_maps_to_ranges() {
        let fractions = [
            0.30, // spin whole arm
            0.02, // bend elbow
            0.10, // close hand
        ];
        // Angle fractions match the spreadsheet: 0 maps to max and 1 maps to min.
        // Close-hand fraction is inverted: 0 is fully closed and 1 is fully open.
        let params1 = [
            180.0 + fractions[0] * (-180.0 - 180.0), // spin whole arm, -180 to +180 degrees
            90.0 + fractions[1] * (-90.0 - 90.0),    // bend elbow, -90 to +90 degrees
            1.0 + fractions[2] * (0.0 - 1.0),        // close hand, 0 to 1 linkage units
        ];

        let expected = [72.0, 86.4, 0.9];
        assert_params_approx_eq(params1, expected);
    }
}
