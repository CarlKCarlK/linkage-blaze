#![no_std]
#![forbid(unsafe_code)]
#![doc = "No-allocation robot arm simulation and math primitives."]

//todo000 move some of that global static stuff to be const local.
//todo000 revisit the name Param and Args
//todo00 allow splits/DAGs in the models.
//todo00 could have (compile-time?) optimizations that collapse adjacent steps of the same type into one step with a combined angle/distance. Would that be worth it? or even multiple moves if one doesn't have parameters.
//todo00 might be nice to have invisible or colored links, but that would be more turtle than linkage.
//todo00 if we did have colored links RGBA, could use a fluent command.

#[cfg(test)]
extern crate std;

pub mod cyd;
mod math;

pub use math::{Mat3, Vec3};

use math::degrees_to_radians;

/// A step in the robot arm linkage description.
///
/// - v0 = tail → nose (forward), v1 = right → left, v2 = belly → back
#[derive(Debug)]
pub enum Step {
    /// Reset to the origin with the identity orientation.
    Start,
    /// Rotate around v2 (belly → back): turn left/right.
    Yaw(Arg),
    /// Rotate around v1 (right → left): nose up/down.
    Pitch(Arg),
    /// Rotate around v0 (tail → nose): right side down.
    Roll(Arg),
    /// Advance along v0 by the given distance.
    Move(Arg),
    /// Lift the pen so later moves do not draw.
    PenUp,
    /// Lower the pen so later moves draw.
    PenDown,
    /// Set the pen color.
    PenColor(u32),
    /// Set the pen stroke width.
    PenWidth(u16),
    /// Add a filled disk at the current pose, in the local v0-v1 plane.
    Disk(f32),
    /// Add a filled disk at the current pose; radius is driven by a degree-of-freedom parameter.
    DiskParam(VariableArg),
    /// Add a ring at the current pose, in the local v0-v1 plane. Stroke width is current pen width.
    Ring(f32),
}

/// A fixed argument or a variable argument driven by a degree-of-freedom parameter.
///
/// Rotation arguments are stored as radians. Move arguments are stored as linkage distances.
#[derive(Debug)]
pub enum Arg {
    Fixed(f32),
    Variable(VariableArg),
}

/// A variable argument with its degree-of-freedom index and legal range.
#[derive(Debug)]
pub struct VariableArg {
    index: usize,
    low: f32,
    span: f32,
}

impl Arg {
    fn resolve<const DOF: usize>(&self, params: &[f32; DOF]) -> f32 {
        match self {
            Self::Fixed(value) => *value,
            Self::Variable(variable_arg) => variable_arg.resolve(params),
        }
    }
}

impl VariableArg {
    const fn new(index: usize, low: f32, high: f32) -> Self {
        Self {
            index,
            low,
            span: high - low,
        }
    }

    const fn from_degrees(index: usize, low: f32, high: f32) -> Self {
        Self::new(index, degrees_to_radians(low), degrees_to_radians(high))
    }

    fn resolve<const DOF: usize>(&self, params: &[f32; DOF]) -> f32 {
        let param = params[self.index];
        self.low + param * self.span
    }
}

/// A fixed-size linkage description.
pub struct Linkage<const DOF: usize, const N: usize> {
    steps: [Step; N],
    len: usize,
}

impl<const DOF: usize, const N: usize> Linkage<DOF, N> {
    /// Start a fixed-size linkage with an implicit origin row.
    pub const fn start() -> Self {
        assert!(N > 0, "linkage must have room for the implicit start step");
        Self {
            steps: [const { Step::Start }; N],
            len: 1,
        }
    }

    /// Return the number of linkage steps, including the implicit start step.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Return the number of runtime parameters this linkage expects.
    #[must_use]
    pub const fn dof(&self) -> usize {
        DOF
    }

    /// Add a yaw step from a user-facing angle in degrees.
    pub const fn yaw(self, degrees: f32) -> Self {
        self.push(Step::Yaw(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a yaw step from a runtime parameter in degrees.
    pub const fn yaw_param(self, index: usize, low: f32, high: f32) -> Self {
        assert!(index < DOF, "parameter index must be within DOF");
        self.push(Step::Yaw(Arg::Variable(VariableArg::from_degrees(
            index, low, high,
        ))))
    }

    /// Add a pitch step from a user-facing angle in degrees.
    pub const fn pitch(self, degrees: f32) -> Self {
        self.push(Step::Pitch(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a pitch step from a runtime parameter in degrees.
    pub const fn pitch_param(self, index: usize, low: f32, high: f32) -> Self {
        assert!(index < DOF, "parameter index must be within DOF");
        self.push(Step::Pitch(Arg::Variable(VariableArg::from_degrees(
            index, low, high,
        ))))
    }

    /// Add a roll step from a user-facing angle in degrees.
    pub const fn roll(self, degrees: f32) -> Self {
        self.push(Step::Roll(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a roll step from a runtime parameter in degrees.
    pub const fn roll_param(self, index: usize, low: f32, high: f32) -> Self {
        assert!(index < DOF, "parameter index must be within DOF");
        self.push(Step::Roll(Arg::Variable(VariableArg::from_degrees(
            index, low, high,
        ))))
    }

    /// Add a fixed forward move step.
    pub const fn forward(self, distance: f32) -> Self {
        self.push(Step::Move(Arg::Fixed(distance)))
    }

    /// Add a move step from a runtime parameter.
    pub const fn move_param(self, index: usize, low: f32, high: f32) -> Self {
        assert!(index < DOF, "parameter index must be within DOF");
        self.push(Step::Move(Arg::Variable(VariableArg::new(
            index, low, high,
        ))))
    }

    /// Restart the linkage path from the origin pose.
    pub const fn restart(self) -> Self {
        self.push(Step::Start)
    }

    /// Lift the pen so later move steps don't draw.
    pub const fn pen_up(self) -> Self {
        self.push(Step::PenUp)
    }

    /// Lower the pen so later move steps draw.
    pub const fn pen_down(self) -> Self {
        self.push(Step::PenDown)
    }

    /// Set the pen color for later move steps.
    pub const fn pen_color(self, color: u32) -> Self {
        self.push(Step::PenColor(color))
    }

    /// Set the pen width for later move steps.
    pub const fn pen_width(self, width: u16) -> Self {
        self.push(Step::PenWidth(width))
    }

    /// Add a filled disk at the current pose, in the local v0-v1 plane.
    pub const fn disk(self, radius: f32) -> Self {
        self.push(Step::Disk(radius))
    }

    /// Add a filled disk at the current pose; radius is driven by a degree-of-freedom parameter.
    pub const fn disk_param(self, index: usize, low: f32, high: f32) -> Self {
        self.push(Step::DiskParam(VariableArg::new(index, low, high)))
    }

    /// Add a ring at the current pose, in the local v0-v1 plane. Stroke width is the current pen width.
    pub const fn ring(self, radius: f32) -> Self {
        self.push(Step::Ring(radius))
    }

    const fn push(mut self, step: Step) -> Self {
        assert!(self.len < N, "linkage has more steps than N");
        self.steps[self.len] = step;
        self.len += 1;
        self
    }

    /// Iterate over poses produced by evaluating this linkage from 0.0 to 1.0 params.
    pub fn poses<'a>(&'a self, params: &'a [f32; DOF]) -> Poses<'a, DOF, N> {
        Poses::new(self, params)
    }

    /// Iterate over styled poses produced by evaluating this linkage.
    pub fn styled_poses<'a>(&'a self, params: &'a [f32; DOF]) -> StyledPoses<'a, DOF, N> {
        StyledPoses::new(self, params)
    }

    /// Iterate over draw items (line strokes, disks, rings) produced by this linkage.
    pub fn draw_items<'a>(&'a self, params: &'a [f32; DOF]) -> DrawItems<'a, DOF, N> {
        DrawItems::new(self, params)
    }

    /// Return the pose produced after evaluating all steps from 0.0 to 1.0 params.
    ///
    /// This always returns a [`Pose`]. A [`Linkage`] contains an implicit start
    /// step, so the pose sequence is never empty.
    #[must_use]
    pub fn final_pose(&self, params: &[f32; DOF]) -> Pose {
        self.poses(params)
            .last()
            .expect("linkage must yield at least the implicit start pose")
    }
}

fn validate_params<const DOF: usize>(params: &[f32; DOF]) {
    for param_index in 0..DOF {
        //todo0 review whether panicking is the right long-term out-of-range behavior.
        assert!(
            (0.0..=1.0).contains(&params[param_index]),
            "parameter is out of range"
        );
    }
}

fn rotation_matrix<const DOF: usize>(step: &Step, params: &[f32; DOF]) -> Mat3 {
    let radians = match step {
        Step::Yaw(arg) | Step::Pitch(arg) | Step::Roll(arg) => arg.resolve(params),
        Step::Start
        | Step::Move(_)
        | Step::PenUp
        | Step::PenDown
        | Step::PenColor(_)
        | Step::PenWidth(_)
        | Step::Disk(_)
        | Step::DiskParam(_)
        | Step::Ring(_) => return Mat3::IDENTITY,
    };
    match step {
        Step::Yaw(_) => Mat3::yaw(radians),
        Step::Pitch(_) => Mat3::pitch(radians),
        Step::Roll(_) => Mat3::roll(radians),
        Step::Start
        | Step::Move(_)
        | Step::PenUp
        | Step::PenDown
        | Step::PenColor(_)
        | Step::PenWidth(_)
        | Step::Disk(_)
        | Step::DiskParam(_)
        | Step::Ring(_) => Mat3::IDENTITY,
    }
}

/// Logo-style pen state for linkage drawing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pen {
    Up,
    Down,
}

/// Drawing state carried while evaluating a linkage.
#[derive(Clone, Copy, Debug)]
pub struct PenStyle {
    pen: Pen,
    color: u32,
    width: u16,
}

impl PenStyle {
    /// Return the default down pen with color 0 and width 1.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pen: Pen::Down,
            color: 0,
            width: 1,
        }
    }

    /// Return the current pen state.
    #[must_use]
    pub const fn pen(self) -> Pen {
        self.pen
    }

    /// Return the current pen color.
    #[must_use]
    pub const fn color(self) -> u32 {
        self.color
    }

    /// Return the current pen width.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    fn apply(&mut self, step: &Step) {
        match step {
            Step::PenUp => self.pen = Pen::Up,
            Step::PenDown => self.pen = Pen::Down,
            Step::PenColor(color) => self.color = *color,
            Step::PenWidth(width) => self.width = *width,
            Step::Start | Step::Yaw(_) | Step::Pitch(_) | Step::Roll(_) | Step::Move(_) | Step::Disk(_) | Step::DiskParam(_) | Step::Ring(_) => {}
        }
    }
}

/// Full pose after evaluating a linkage step.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    orientation: Mat3,
    position: Vec3,
}

impl Pose {
    /// Create a pose from an orientation and position.
    #[must_use]
    pub const fn new(orientation: Mat3, position: Vec3) -> Self {
        Self {
            orientation,
            position,
        }
    }

    /// Return the origin pose with identity orientation.
    #[must_use]
    pub const fn start() -> Self {
        Self {
            orientation: Mat3::IDENTITY,
            position: Vec3::ZERO,
        }
    }

    /// Return this pose's orientation matrix.
    #[must_use]
    pub const fn orientation(self) -> Mat3 {
        self.orientation
    }

    /// Return this pose's position.
    #[must_use]
    pub const fn position(self) -> Vec3 {
        self.position
    }

    /// Return true when all orientation and position components are within `tolerance`.
    #[must_use]
    pub fn is_close_to(&self, other: &Self, tolerance: f32) -> bool {
        self.orientation.is_close_to(&other.orientation, tolerance)
            && self.position.is_close_to(&other.position, tolerance)
    }

    fn apply<const DOF: usize>(&mut self, step: &Step, params: &[f32; DOF]) {
        match step {
            Step::Start => {
                *self = Self::start();
            }
            Step::Move(arg) => {
                self.position += self.orientation.forward() * arg.resolve(params);
            }
            Step::Yaw(_) | Step::Pitch(_) | Step::Roll(_) => {
                self.orientation = self.orientation * rotation_matrix(step, params);
            }
            Step::PenUp | Step::PenDown | Step::PenColor(_) | Step::PenWidth(_) | Step::Disk(_) | Step::DiskParam(_) | Step::Ring(_) => {}
        }
    }
}

/// Full pose plus Logo-style pen state after evaluating a linkage step.
#[derive(Clone, Copy, Debug)]
pub struct StyledPose {
    pose: Pose,
    pen_style: PenStyle,
}

impl StyledPose {
    /// Return this styled pose's geometry.
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }

    /// Return this styled pose's orientation matrix.
    #[must_use]
    pub const fn orientation(self) -> Mat3 {
        self.pose.orientation()
    }

    /// Return this styled pose's position.
    #[must_use]
    pub const fn position(self) -> Vec3 {
        self.pose.position()
    }

    /// Return this styled pose's pen state.
    #[must_use]
    pub const fn pen(self) -> Pen {
        self.pen_style.pen()
    }

    /// Return this styled pose's pen color.
    #[must_use]
    pub const fn color(self) -> u32 {
        self.pen_style.color()
    }

    /// Return this styled pose's pen width.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.pen_style.width()
    }
}

/// A drawable pen-down move segment produced by a linkage.
#[derive(Clone, Copy, Debug)]
pub struct StrokeSegment {
    start: Pose,
    end: Pose,
    color: u32,
    width: u16,
}

impl StrokeSegment {
    /// Return the segment start pose.
    #[must_use]
    pub const fn start(self) -> Pose {
        self.start
    }

    /// Return the segment end pose.
    #[must_use]
    pub const fn end(self) -> Pose {
        self.end
    }

    /// Return the segment pen color.
    #[must_use]
    pub const fn color(self) -> u32 {
        self.color
    }

    /// Return the segment pen width.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }
}

/// Iterator over poses produced by evaluating a linkage.
///
/// Yields one [`Pose`] after every linkage step, including the implicit [`Step::Start`].
pub struct Poses<'a, const DOF: usize, const N: usize> {
    linkage: &'a Linkage<DOF, N>,
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
}

impl<'a, const DOF: usize, const N: usize> Poses<'a, DOF, N> {
    /// Create a new pose iterator for the given linkage.
    fn new(linkage: &'a Linkage<DOF, N>, params: &'a [f32; DOF]) -> Self {
        validate_params(params);
        Self {
            linkage,
            params,
            index: 0,
            pose: Pose::start(),
        }
    }
}

impl<const DOF: usize, const N: usize> Iterator for Poses<'_, DOF, N> {
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

/// Iterator over styled poses produced by evaluating a linkage.
///
/// Yields after every linkage step, including non-move steps and the implicit
/// [`Step::Start`].
pub struct StyledPoses<'a, const DOF: usize, const N: usize> {
    linkage: &'a Linkage<DOF, N>,
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
}

impl<'a, const DOF: usize, const N: usize> StyledPoses<'a, DOF, N> {
    fn new(linkage: &'a Linkage<DOF, N>, params: &'a [f32; DOF]) -> Self {
        validate_params(params);
        Self {
            linkage,
            params,
            index: 0,
            pose: Pose::start(),
            pen_style: PenStyle::new(),
        }
    }
}

impl<const DOF: usize, const N: usize> Iterator for StyledPoses<'_, DOF, N> {
    type Item = StyledPose;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.linkage.len {
            return None;
        }
        let step = &self.linkage.steps[self.index];
        self.index += 1;
        self.pose.apply(step, self.params);
        self.pen_style.apply(step);
        Some(StyledPose {
            pose: self.pose,
            pen_style: self.pen_style,
        })
    }
}

/// A disk shape yielded by a linkage at the current pose.
#[derive(Clone, Copy, Debug)]
pub struct DiskItem {
    pose: Pose,
    radius: f32,
    color: u32,
}

impl DiskItem {
    #[must_use]
    pub const fn pose(self) -> Pose { self.pose }
    #[must_use]
    pub const fn radius(self) -> f32 { self.radius }
    #[must_use]
    pub const fn color(self) -> u32 { self.color }
}

/// A ring shape yielded by a linkage at the current pose. Stroke width is the pen width at that step.
#[derive(Clone, Copy, Debug)]
pub struct RingItem {
    pose: Pose,
    radius: f32,
    color: u32,
    width: u16,
}

impl RingItem {
    #[must_use]
    pub const fn pose(self) -> Pose { self.pose }
    #[must_use]
    pub const fn radius(self) -> f32 { self.radius }
    #[must_use]
    pub const fn color(self) -> u32 { self.color }
    #[must_use]
    pub const fn width(self) -> u16 { self.width }
}

/// A draw item produced by a linkage: a line stroke, a filled disk, or a ring.
#[derive(Clone, Copy, Debug)]
pub enum DrawItem {
    Stroke(StrokeSegment),
    Disk(DiskItem),
    Ring(RingItem),
}

/// Iterator over draw items (line strokes, disks, rings) produced by a linkage.
///
/// Move steps with the pen down yield [`DrawItem::Stroke`]. Disk and Ring steps
/// always yield their respective variants. All other steps only update state.
pub struct DrawItems<'a, const DOF: usize, const N: usize> {
    linkage: &'a Linkage<DOF, N>,
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
}

impl<'a, const DOF: usize, const N: usize> DrawItems<'a, DOF, N> {
    fn new(linkage: &'a Linkage<DOF, N>, params: &'a [f32; DOF]) -> Self {
        validate_params(params);
        Self {
            linkage,
            params,
            index: 0,
            pose: Pose::start(),
            pen_style: PenStyle::new(),
        }
    }
}

impl<const DOF: usize, const N: usize> Iterator for DrawItems<'_, DOF, N> {
    type Item = DrawItem;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.linkage.len {
            let step = &self.linkage.steps[self.index];
            self.index += 1;
            let start_pose = self.pose;
            let pen_style = self.pen_style;
            self.pose.apply(step, self.params);
            self.pen_style.apply(step);

            match step {
                Step::Move(_) if matches!(pen_style.pen(), Pen::Down) => {
                    return Some(DrawItem::Stroke(StrokeSegment {
                        start: start_pose,
                        end: self.pose,
                        color: pen_style.color(),
                        width: pen_style.width(),
                    }));
                }
                Step::Disk(radius) => {
                    return Some(DrawItem::Disk(DiskItem {
                        pose: start_pose,
                        radius: *radius,
                        color: pen_style.color(),
                    }));
                }
                Step::DiskParam(var_arg) => {
                    return Some(DrawItem::Disk(DiskItem {
                        pose: start_pose,
                        radius: var_arg.resolve(self.params),
                        color: pen_style.color(),
                    }));
                }
                Step::Ring(radius) => {
                    return Some(DrawItem::Ring(RingItem {
                        pose: start_pose,
                        radius: *radius,
                        color: pen_style.color(),
                        width: pen_style.width(),
                    }));
                }
                _ => {}
            }
        }

        None
    }
}

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests {
    use super::{Linkage, Pose};
    use crate::test_helpers::{
        assert_png_matches_expected, assert_pose_approx_eq, assert_pose_trace_matches_expected,
        draw_linkage_xy_canvas,
    };
    use std::{boxed::Box, error::Error};

    //todo000 *_param might not be a good suffix.
    const LINKAGE0: Linkage<6, 24> = Linkage::start()
        .yaw(90.0)
        .yaw_param(4, 180.0, -180.0) // spin whole arm
        .pitch(90.0)
        .forward(2.5)
        .pitch(-90.0)
        .pitch_param(3, 30.0, 0.0) // lower arm
        .forward(3.0)
        .yaw_param(1, 90.0, -90.0) // bend elbow
        .forward(3.0)
        .pitch_param(0, 90.0, -90.0) // lower hand
        .forward(1.0)
        .roll_param(5, 180.0, -180.0) // spin hand
        .forward(0.5)
        .yaw(90.0)
        .move_param(2, 0.0, 0.5) // close hand
        .yaw(-90.0)
        .forward(1.0)
        .yaw(180.0)
        .forward(1.0)
        .yaw(90.0)
        .move_param(2, 0.0, 1.0) // close hand
        .yaw(90.0)
        .forward(1.0);

    const LINKAGE1: Linkage<3, 16> = Linkage::start()
        .yaw(90.0)
        .yaw_param(0, 180.0, -180.0) // spin whole arm
        .forward(3.0)
        .yaw_param(1, 90.0, -90.0) // bend elbow
        .forward(3.0)
        .yaw(90.0)
        .move_param(2, 0.5, 0.0) // close hand
        .yaw(-90.0)
        .forward(1.0)
        .yaw(-180.0)
        .forward(1.0)
        .yaw(90.0)
        .move_param(2, 1.0, 0.0) // close hand
        .yaw(90.0)
        .forward(1.0);

    #[test]
    fn test_excel_pose_trace0_matches_expected() -> Result<(), Box<dyn Error>> {
        // Fractions for [lower hand, bend elbow, close hand,
        //  lower arm, spin whole arm, spin hand].
        let params = [0.7514501463, 0.5002003842, 0.5, 1.0, 0.6254387123, 0.0];
        assert_pose_trace_matches_expected("excel_pose_trace0.csv", LINKAGE0.poses(&params))
    }

    #[test]
    fn test_excel_pose_trace1_matches_expected() -> Result<(), Box<dyn Error>> {
        // [spin whole arm, bend elbow, close hand]
        let params = [0.30, 0.02, 0.10];
        assert_pose_trace_matches_expected("excel_pose_trace1.csv", LINKAGE1.poses(&params))
    }

    #[test]
    fn test_setting0_matches_excel_final_pose() -> Result<(), Box<dyn Error>> {
        //todo00 might be nice to have the names available somehow.
        let params = [
            0.7514501463, // lower hand
            0.49,         // bend elbow
            0.50011957,   // close hand
            1.0,          // lower arm
            0.6254387123, // spin whole arm
            1.0,          // spin hand
        ];
        let pose = LINKAGE0.final_pose(&params);
        let expected = Pose::new(
            [
                [0.48325038, 0.7270788, 0.48767346],
                [0.5117748, -0.68655396, 0.51645917],
                [0.7103207, 0.0, -0.70387816],
            ]
            .into(),
            [5.213134, 5.747819, -0.7241982].into(),
        );

        assert_pose_approx_eq(pose, expected);
        Ok(())
    }

    #[test]
    fn test_setting1_matches_excel_final_pose() -> Result<(), Box<dyn Error>> {
        let params = [
            0.30, // spin whole arm
            0.02, // bend elbow
            0.10, // close hand
        ];
        let pose = LINKAGE1.final_pose(&params);
        let expected = Pose::new(
            [
                [-0.368124515, 0.929776430, 0.0],
                [-0.929776430, -0.368124515, 0.0],
                [0.0, 0.0, 1.0],
            ]
            .into(),
            [-4.744067192, -2.626399040, 0.0].into(),
        );

        assert_pose_approx_eq(pose, expected);
        Ok(())
    }

    #[test]
    fn test_mid_setting0_matches_excel_final_pose_and_png() -> Result<(), Box<dyn Error>> {
        let params = [
            0.5, // lower hand
            0.3, // bend elbow
            1.0, // close hand
            0.5, // lower arm
            0.5, // spin whole arm
            0.5, // spin hand
        ];
        let pose = LINKAGE0.final_pose(&params);
        let expected = Pose::new(
            [
                [-0.5877855, -0.80901694, 0.0],
                [0.78145033, -0.5677572, 0.25881904],
                [-0.20938899, 0.15213005, 0.9659258],
            ]
            .into(),
            [-2.828311, 7.4796333, -4.504162].into(),
        );

        assert_pose_approx_eq(pose, expected);

        let canvas = draw_linkage_xy_canvas(&LINKAGE0, &params);
        assert_png_matches_expected("linkage0_xy_mid_fraction.png", &canvas)
    }

    #[test]
    fn test_linkage0_png_matches_expected() -> Result<(), Box<dyn Error>> {
        // Fractions for [lower hand, bend elbow, close hand,
        //  lower arm, spin whole arm, spin hand].
        let params = [0.7514501463, 0.5002003842, 0.5, 1.0, 0.6254387123, 0.0];

        let canvas = draw_linkage_xy_canvas(&LINKAGE0, &params);
        assert_png_matches_expected("linkage0_xy.png", &canvas)
    }

    #[test]
    fn test_linkage1_png_matches_expected() -> Result<(), Box<dyn Error>> {
        // [spin whole arm, bend elbow, close hand]
        let params = [0.30, 0.02, 0.10];

        let canvas = draw_linkage_xy_canvas(&LINKAGE1, &params);
        assert_png_matches_expected("linkage1_xy.png", &canvas)
    }

    #[test]
    #[should_panic(expected = "parameter is out of range")]
    fn test_params_are_range_checked() {
        let params = [
            0.0, // lower hand
            0.5, // bend elbow
            1.1, // close hand, invalid param
            1.0, // lower arm
            0.0, // spin whole arm
            0.5, // spin hand
        ];

        let _ = LINKAGE0.final_pose(&params);
    }
}
