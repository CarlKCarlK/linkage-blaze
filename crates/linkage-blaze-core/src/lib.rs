#![no_std]
#![forbid(unsafe_code)]
#![doc = "No-allocation robot arm simulation and math primitives."]
//!
//! Model-space axes:
//!
//! - +X = forward / along the link
//! - +Y = left
//! - +Z = up
//!
//! Rotations:
//!
//! - yaw = rotate about local +Z
//! - pitch = rotate about local +Y
//! - roll = rotate about local +X

//todo000 move some of that global static stuff to be const local.
//todo000 revisit the name Param and Args
//todo00 allow splits/DAGs in the models.
//todo00 could have (compile-time?) optimizations that collapse adjacent steps of the same type into one step with a combined angle/distance. Would that be worth it? or even multiple moves if one doesn't have parameters.
//todo00 might be nice to have invisible or colored links, but that would be more turtle than linkage.
//todo00 if we did have colored links RGBA, could use a fluent command.
//todo "DOF" == params = parameters. do these names make sense? are they explained well?

#[cfg(test)]
extern crate std;

mod math;

pub use math::{Mat3, Vec3};

pub use embedded_graphics::pixelcolor::Rgb888;
use math::degrees_to_radians;

/// A step in the robot arm linkage description.
///
/// Model-space axes:
///
/// - +X = forward / along the link
/// - +Y = left
/// - +Z = up
///
/// Rotations are local-frame rotations: yaw about +Z, pitch about +Y,
/// and roll about +X.
#[derive(Clone, Copy, Debug)]
pub enum Step {
    /// Reset to the origin with the identity orientation.
    Start,
    /// Rotate around local +Z.
    Yaw(Arg),
    /// Rotate around local +Y.
    Pitch(Arg),
    /// Rotate around local +X.
    Roll(Arg),
    /// Advance along local +X by the given distance.
    Move(Arg),
    /// Advance along local +Y by the given distance.
    Left(Arg),
    /// Advance along local +Z by the given distance.
    Up(Arg),
    /// Lift the pen so later moves do not draw.
    PenUp,
    /// Lower the pen so later moves draw.
    PenDown,
    /// Set the pen color.
    PenColor(Rgb888),
    /// Set the pen stroke width in linkage units.
    PenWidth(f32),
    /// Add a filled disk at the current pose, in the local +X/+Y plane.
    Disk(f32),
    /// Add a filled disk at the current pose; radius is driven by a degree-of-freedom parameter.
    DiskParam(VariableArg),
    /// Add a ring at the current pose, in the local +X/+Y plane. Stroke width is current pen width.
    Ring(f32),
    /// Add a ring at the current pose; radius is driven by a degree-of-freedom parameter.
    RingParam(VariableArg),
    /// Add a sphere centered at the current pose.
    Sphere(f32),
    /// Add a sphere centered at the current pose; radius is driven by a degree-of-freedom parameter.
    SphereParam(VariableArg),
    /// Save the current pose and pen state under a name for later recall.
    Mark { name: &'static str },
    /// Restore a previously marked pose and pen state (index resolved at build time).
    Restore { index: usize },
}

/// A fixed argument or a variable argument driven by a degree-of-freedom parameter.
///
/// Rotation arguments are stored as radians. Translation arguments are stored as linkage distances.
#[derive(Clone, Copy, Debug)]
pub enum Arg {
    Fixed(f32),
    Variable(VariableArg),
}

/// A variable argument with its degree-of-freedom index and legal range.
#[derive(Clone, Copy, Debug)]
pub struct VariableArg {
    index: usize,
    low: f32,
    span: f32,
}

/// A named runtime linkage parameter.
#[derive(Clone, Copy, Debug)]
pub struct Param {
    name: &'static str,
    default: f32,
}

impl Param {
    const EMPTY: Self = Self {
        name: "",
        default: 0.0,
    };

    /// Return the parameter's display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Return the parameter's normalized default value.
    #[must_use]
    pub const fn default(self) -> f32 {
        self.default
    }
}

impl Arg {
    fn resolve<const DOF: usize>(&self, params: &[f32; DOF]) -> f32 {
        match self {
            Self::Fixed(value) => *value,
            Self::Variable(variable_arg) => variable_arg.resolve(params),
        }
    }

    const fn offset_param(self, offset: usize) -> Self {
        match self {
            Self::Fixed(_) => self,
            Self::Variable(v) => Self::Variable(v.offset(offset)),
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

    const fn offset(self, offset: usize) -> Self {
        Self {
            index: self.index + offset,
            ..self
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

/// A linkage that can be queried for runtime parameters and evaluated to produce poses and draw items.
///
/// `Linkage` defines the read-only interface for querying parameter metadata. Concrete
/// implementations like [`LinkageFixed`] add builder methods and evaluation capabilities.
///
/// # Parameters
///
/// A linkage has degrees-of-freedom (DOF) which correspond to named runtime parameters
/// that control its behavior. Each parameter has a name, default value, and index.
///
/// # Examples
///
/// Query a linkage for its parameter count and metadata:
///
/// ```rust
/// # use linkage_blaze_core::{Linkage, LinkageFixed};
/// const LINKAGE: LinkageFixed<2, 8> = LinkageFixed::start()
///     .define_param("x", 0.5)
///     .define_param("y", 0.5);
///
/// assert_eq!(LINKAGE.dof(), 2);
/// assert_eq!(LINKAGE.param_name(0), "x");
/// ```
pub trait Linkage {
    /// Return the number of runtime parameters (degrees of freedom) this linkage expects.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<3, 8> = LinkageFixed::start()
    ///     .define_param("rotation", 0.5)
    ///     .define_param("height", 0.5)
    ///     .define_param("width", 0.5);
    ///
    /// assert_eq!(LINKAGE.dof(), 3);
    /// ```
    fn dof(&self) -> usize;

    /// Return the number of linkage steps, including the implicit start step.
    ///
    /// Every linkage begins with an implicit step at the origin, so `len() >= 1`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<0, 5> = LinkageFixed::start()
    ///     .forward(1.0)
    ///     .forward(2.0);
    ///
    /// assert_eq!(LINKAGE.len(), 3);  // start + 2 forward steps
    /// ```
    fn len(&self) -> usize;

    /// Return the number of named parameters defined in this linkage.
    ///
    /// This is the count of parameters that have been explicitly defined via `define_param`.
    /// It is always ≤ [`dof`](Self::dof).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<3, 8> = LinkageFixed::start()
    ///     .define_param("x", 0.0)
    ///     .define_param("y", 0.5);
    ///
    /// assert_eq!(LINKAGE.param_len(), 2);
    /// ```
    fn param_len(&self) -> usize;

    /// Return a parameter definition by index.
    ///
    /// # Panics
    ///
    /// Panics if `index >= param_len()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<2, 8> = LinkageFixed::start()
    ///     .define_param("angle", 0.25)
    ///     .define_param("distance", 0.75);
    ///
    /// let param = LINKAGE.param(0);
    /// assert_eq!(param.name(), "angle");
    /// assert_eq!(param.default(), 0.25);
    /// ```
    fn param(&self, index: usize) -> Param;

    /// Return a parameter's name by index.
    ///
    /// # Panics
    ///
    /// Panics if `index >= param_len()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<2, 8> = LinkageFixed::start()
    ///     .define_param("rotation", 0.0)
    ///     .define_param("scale", 1.0);
    ///
    /// assert_eq!(LINKAGE.param_name(0), "rotation");
    /// assert_eq!(LINKAGE.param_name(1), "scale");
    /// ```
    fn param_name(&self, index: usize) -> &'static str;

    /// Return a parameter's default value by index (normalized to [0.0, 1.0]).
    ///
    /// # Panics
    ///
    /// Panics if `index >= param_len()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<2, 8> = LinkageFixed::start()
    ///     .define_param("a", 0.3)
    ///     .define_param("b", 0.7);
    ///
    /// assert_eq!(LINKAGE.param_default(0), 0.3);
    /// assert_eq!(LINKAGE.param_default(1), 0.7);
    /// ```
    fn param_default(&self, index: usize) -> f32;

    /// Return the number of parameters defined with the given name.
    ///
    /// Supports shadowing: if the same parameter name is defined multiple times,
    /// this counts all occurrences.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<3, 8> = LinkageFixed::start()
    ///     .define_param("angle", 0.0)
    ///     .define_param("angle", 0.5)  // shadow with new default
    ///     .define_param("distance", 1.0);
    ///
    /// assert_eq!(LINKAGE.param_count_named("angle"), 2);
    /// assert_eq!(LINKAGE.param_count_named("distance"), 1);
    /// assert_eq!(LINKAGE.param_count_named("unknown"), 0);
    /// ```
    fn param_count_named(&self, name: &str) -> usize;

    /// Return the index of the `n`th parameter (0-based) with the given name.
    ///
    /// With shadowing, multiple parameters may share the same name. This allows
    /// accessing any of them by occurrence number.
    ///
    /// # Panics
    ///
    /// Panics if the name is not found or if `n` exceeds the occurrence count.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<3, 8> = LinkageFixed::start()
    ///     .define_param("x", 0.0)
    ///     .define_param("y", 0.5)
    ///     .define_param("x", 0.8);  // shadow: second "x"
    ///
    /// assert_eq!(LINKAGE.param_index("x", 0), 0);  // first "x"
    /// assert_eq!(LINKAGE.param_index("x", 1), 2);  // second "x"
    /// assert_eq!(LINKAGE.param_index("y", 0), 1);  // only "y"
    /// ```
    fn param_index(&self, name: &str, n: usize) -> usize;
}

/// A fixed-size linkage description.
pub struct LinkageFixed<const DOF: usize, const N: usize> {
    steps: [Step; N],
    len: usize,
    params: [Param; DOF],
    param_len: usize,
    mark_names: [&'static str; N],
    mark_len: usize,
}

impl<const DOF: usize, const N: usize> LinkageFixed<DOF, N> {
    /// Start a fixed-size linkage with an implicit origin row.
    pub const fn start() -> Self {
        assert!(N > 0, "linkage must have room for the implicit start step");
        Self {
            steps: [const { Step::Start }; N],
            len: 1,
            params: [Param::EMPTY; DOF],
            param_len: 0,
            mark_names: [""; N],
            mark_len: 0,
        }
    }

    /// Return the number of linkage steps, including the implicit start step.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Number of runtime parameters this linkage expects.
    pub const DOF: usize = DOF;

    /// Step-slot capacity of this linkage.
    pub const N: usize = N;

    /// Return the number of runtime parameters this linkage expects.
    #[must_use]
    pub const fn dof(&self) -> usize {
        DOF
    }

    /// Return the number of named parameters defined in this linkage.
    #[must_use]
    pub const fn param_len(&self) -> usize {
        self.param_len
    }

    /// Return a parameter definition by index.
    #[must_use]
    pub const fn param(&self, index: usize) -> Param {
        assert!(index < self.param_len, "parameter index must be defined");
        self.params[index]
    }

    /// Return a parameter's name by index.
    #[must_use]
    pub const fn param_name(&self, index: usize) -> &'static str {
        self.param(index).name()
    }

    /// Return a parameter's default value by index.
    #[must_use]
    pub const fn param_default(&self, index: usize) -> f32 {
        self.param(index).default()
    }

    /// Iterate over all indices of parameters with a given name.
    ///
    /// Use `.last()` for shadowing semantics (most recently defined wins),
    /// `.next()` for the first definition, or collect/iterate for all of them.
    pub fn param_indices<'a>(&'a self, name: &'a str) -> ParamIndices<'a, DOF, N> {
        ParamIndices {
            linkage: self,
            name,
            pos: 0,
        }
    }

    /// Return this linkage's normalized parameter defaults.
    #[must_use]
    pub const fn param_defaults(&self) -> [f32; DOF] {
        let mut params = [0.0; DOF];
        let mut param_index = 0;
        while param_index < self.param_len {
            params[param_index] = self.params[param_index].default;
            param_index += 1;
        }
        params
    }

    /// Define a named runtime parameter.
    ///
    /// Duplicate names are allowed; later definitions shadow earlier ones when
    /// a builder method like `yaw_param` looks up the name.
    pub const fn define_param(mut self, name: &'static str, default: f32) -> Self {
        assert!(self.param_len < DOF, "linkage has more params than DOF");
        assert!(default >= 0.0, "parameter default must be at least 0.0");
        assert!(default <= 1.0, "parameter default must be at most 1.0");
        self.params[self.param_len] = Param { name, default };
        self.param_len += 1;
        self
    }

    /// Add a yaw step from a user-facing angle in degrees.
    pub const fn yaw(self, degrees: f32) -> Self {
        self.push(Step::Yaw(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a yaw step from a runtime parameter in degrees.
    pub const fn yaw_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::Yaw(Arg::Variable(VariableArg::from_degrees(
            index, low, high,
        ))))
    }

    /// Add a pitch step from a user-facing angle in degrees.
    pub const fn pitch(self, degrees: f32) -> Self {
        self.push(Step::Pitch(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a pitch step from a runtime parameter in degrees.
    pub const fn pitch_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::Pitch(Arg::Variable(VariableArg::from_degrees(
            index, low, high,
        ))))
    }

    /// Add a roll step from a user-facing angle in degrees.
    pub const fn roll(self, degrees: f32) -> Self {
        self.push(Step::Roll(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a roll step from a runtime parameter in degrees.
    pub const fn roll_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::Roll(Arg::Variable(VariableArg::from_degrees(
            index, low, high,
        ))))
    }

    /// Add a fixed forward move step.
    pub const fn forward(self, distance: f32) -> Self {
        self.push(Step::Move(Arg::Fixed(distance)))
    }

    /// Add a move step from a runtime parameter.
    pub const fn forward_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::Move(Arg::Variable(VariableArg::new(
            index, low, high,
        ))))
    }

    /// Add a fixed left move step.
    pub const fn left(self, distance: f32) -> Self {
        self.push(Step::Left(Arg::Fixed(distance)))
    }

    /// Add a left move step from a runtime parameter.
    pub const fn left_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::Left(Arg::Variable(VariableArg::new(
            index, low, high,
        ))))
    }

    /// Add a fixed up move step.
    pub const fn up(self, distance: f32) -> Self {
        self.push(Step::Up(Arg::Fixed(distance)))
    }

    /// Add an up move step from a runtime parameter.
    pub const fn up_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::Up(Arg::Variable(VariableArg::new(index, low, high))))
    }

    /// Restart the linkage path from the origin pose.
    /// Save the current pose and pen state under a name for later recall.
    pub const fn mark(mut self, name: &'static str) -> Self {
        assert!(self.mark_len < N, "linkage has more marks than N");
        self.mark_names[self.mark_len] = name;
        self.mark_len += 1;
        self.push(Step::Mark { name })
    }

    /// Restore a previously marked pose and pen state.
    /// Resolves `name` at build time using last-definition-wins (shadowing) semantics.
    pub const fn restore(self, name: &'static str) -> Self {
        let index = match self.last_mark_index(name) {
            Some(i) => i,
            None => {
                panic!("restore: no mark found with name (mark must be defined before restore)")
            }
        };
        self.push(Step::Restore { index })
    }

    /// Restore the `n`th marked pose with the given name (0 = first definition).
    /// Resolves at build time.
    pub const fn restore_nth(self, name: &'static str, n: usize) -> Self {
        let index = match self.mark_index_nth(name, n) {
            Some(i) => i,
            None => panic!(
                "restore_nth: no matching mark found (must define mark before restoring nth)"
            ),
        };
        self.push(Step::Restore { index })
    }

    const fn last_mark_index(&self, name: &str) -> Option<usize> {
        let mut i = self.mark_len;
        while i > 0 {
            i -= 1;
            if str_eq(self.mark_names[i], name) {
                return Some(i);
            }
        }
        None
    }

    const fn mark_index_nth(&self, name: &str, n: usize) -> Option<usize> {
        let mut count = 0;
        let mut i = 0;
        while i < self.mark_len {
            if str_eq(self.mark_names[i], name) {
                if count == n {
                    return Some(i);
                }
                count += 1;
            }
            i += 1;
        }
        None
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
    pub const fn pen_color(self, color: Rgb888) -> Self {
        self.push(Step::PenColor(color))
    }

    /// Set the pen width in linkage units for later move steps.
    pub const fn pen_width(self, width: f32) -> Self {
        assert!(width >= 0.0, "pen width must be non-negative");
        self.push(Step::PenWidth(width))
    }

    /// Add a filled disk at the current pose, in the local v0-v1 plane.
    pub const fn disk(self, radius: f32) -> Self {
        self.push(Step::Disk(radius))
    }

    /// Add a filled disk at the current pose; radius is driven by a degree-of-freedom parameter.
    pub const fn disk_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::DiskParam(VariableArg::new(index, low, high)))
    }

    /// Add a ring at the current pose, in the local v0-v1 plane. Stroke width is the current pen width.
    pub const fn ring(self, radius: f32) -> Self {
        self.push(Step::Ring(radius))
    }

    /// Add a ring at the current pose; radius is driven by a degree-of-freedom parameter.
    pub const fn ring_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::RingParam(VariableArg::new(index, low, high)))
    }

    /// Add a sphere centered at the current pose.
    pub const fn sphere(self, radius: f32) -> Self {
        self.push(Step::Sphere(radius))
    }

    /// Add a sphere centered at the current pose; radius is driven by a degree-of-freedom parameter.
    pub const fn sphere_param(self, name: &str, low: f32, high: f32) -> Self {
        let index = self.expect_param_index(name);
        self.push(Step::SphereParam(VariableArg::new(index, low, high)))
    }

    /// Return a new linkage with a sphere at the start and end of every move step
    /// (Move, Left, Up — both fixed and parametric).
    ///
    /// Spheres at adjacent move endpoints overlap and render twice, which is fine.
    /// `NOUT` must be ≥ `self.len` + (number of move steps × 2).
    pub const fn with_joint_spheres<const NOUT: usize>(
        self,
        joint_radius: f32,
    ) -> LinkageFixed<DOF, NOUT> {
        let mut out = LinkageFixed {
            steps: [const { Step::Start }; NOUT],
            len: 0,
            params: [Param::EMPTY; DOF],
            param_len: self.param_len,
            mark_names: [""; NOUT],
            mark_len: self.mark_len,
        };
        let mut i = 0;
        while i < self.param_len {
            out.params[i] = self.params[i];
            i += 1;
        }
        let mut i = 0;
        while i < self.mark_len {
            out.mark_names[i] = self.mark_names[i];
            i += 1;
        }
        let mut i = 0;
        while i < self.len {
            let step = self.steps[i];
            let is_move = match step {
                Step::Move(_) | Step::Left(_) | Step::Up(_) => true,
                _ => false,
            };
            if is_move {
                assert!(out.len < NOUT, "NOUT too small for with_joint_spheres");
                out.steps[out.len] = Step::Sphere(joint_radius);
                out.len += 1;
            }
            assert!(out.len < NOUT, "NOUT too small for with_joint_spheres");
            out.steps[out.len] = step;
            out.len += 1;
            if is_move {
                assert!(out.len < NOUT, "NOUT too small for with_joint_spheres");
                out.steps[out.len] = Step::Sphere(joint_radius);
                out.len += 1;
            }
            i += 1;
        }
        out
    }

    const fn push(mut self, step: Step) -> Self {
        assert!(self.len < N, "linkage has more steps than N");
        self.steps[self.len] = step;
        self.len += 1;
        self
    }

    /// Return the index of the most recently defined parameter with the given name.
    ///
    /// Scans backwards so the most recently defined definition wins (shadowing).
    const fn last_param_index(&self, name: &str) -> Option<usize> {
        let mut i = self.param_len;
        while i > 0 {
            i -= 1;
            if str_eq(self.params[i].name, name) {
                return Some(i);
            }
        }
        None
    }

    const fn expect_param_index(&self, name: &str) -> usize {
        match self.last_param_index(name) {
            Some(index) => index,
            None => panic!("unknown parameter name"),
        }
    }

    /// Return the number of parameters defined with the given name.
    #[must_use]
    pub const fn param_count_named(&self, name: &str) -> usize {
        let mut count = 0;
        let mut param_index = 0;
        while param_index < self.param_len {
            if str_eq(self.params[param_index].name, name) {
                count += 1;
            }
            param_index += 1;
        }
        count
    }

    /// Return the index of the `n`th parameter (0-based) with the given name.
    ///
    /// Panics at compile time if fewer than `n + 1` parameters have that name.
    #[must_use]
    pub const fn param_index(&self, name: &str, n: usize) -> usize {
        let mut found = 0;
        let mut slot = 0;
        while slot < self.param_len {
            if str_eq(self.params[slot].name, name) {
                if found == n {
                    return slot;
                }
                found += 1;
            }
            slot += 1;
        }
        panic!("parameter name not found or occurrence index out of range")
    }

    /// Iterate over poses produced by evaluating this linkage from 0.0 to 1.0 params.
    pub fn poses<'a>(&'a self, params: &'a [f32; DOF]) -> impl Iterator<Item = Pose> + 'a {
        self.styled_poses(params).map(|sp| sp.pose)
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

    /// Append another linkage's steps after this one's, merging their parameters.
    ///
    /// The caller must supply the output sizes as const generics since Rust cannot
    /// compute `DOF + DOF2` as a const expression yet — follow the same pattern as
    /// `LedLayout::combine_h`. A compile-time assertion verifies the sizes are correct.
    ///
    /// The `other` linkage's implicit `Start` step is skipped so evaluation continues
    /// from wherever `self` ends rather than resetting to the origin.
    pub const fn combine<
        const DOF2: usize,
        const N2: usize,
        const DOF_OUT: usize,
        const N_OUT: usize,
    >(
        self,
        other: LinkageFixed<DOF2, N2>,
    ) -> LinkageFixed<DOF_OUT, N_OUT> {
        assert!(DOF_OUT == DOF + DOF2, "DOF_OUT must equal DOF1 + DOF2");
        let other_steps = other.len - 1; // skip the implicit Start step
        assert!(
            N_OUT >= self.len + other_steps,
            "N_OUT must fit all steps from both linkages"
        );

        let mut out = LinkageFixed {
            steps: [const { Step::Start }; N_OUT],
            len: 0,
            params: [Param::EMPTY; DOF_OUT],
            param_len: 0,
            mark_names: [""; N_OUT],
            mark_len: 0,
        };

        // Copy self's steps as-is
        let mut i = 0;
        while i < self.len {
            out.steps[i] = self.steps[i];
            i += 1;
        }
        out.len = self.len;

        // Copy other's steps (skip index 0 = Start), offsetting param and remember indices
        let mut i = 1;
        while i < other.len {
            out.steps[out.len] = other.steps[i].offset_params(DOF, self.mark_len);
            out.len += 1;
            i += 1;
        }

        // Copy self's params
        let mut i = 0;
        while i < self.param_len {
            out.params[i] = self.params[i];
            i += 1;
        }

        // Copy other's params
        let mut i = 0;
        while i < other.param_len {
            out.params[DOF + i] = other.params[i];
            i += 1;
        }
        out.param_len = self.param_len + other.param_len;

        // Copy remember names from both
        let mut i = 0;
        while i < self.mark_len {
            out.mark_names[i] = self.mark_names[i];
            i += 1;
        }
        let mut i = 0;
        while i < other.mark_len {
            out.mark_names[self.mark_len + i] = other.mark_names[i];
            i += 1;
        }
        out.mark_len = self.mark_len + other.mark_len;

        out
    }
}

impl<const DOF: usize, const N: usize> Linkage for LinkageFixed<DOF, N> {
    fn dof(&self) -> usize {
        DOF
    }

    fn len(&self) -> usize {
        self.len
    }

    fn param_len(&self) -> usize {
        self.param_len
    }

    fn param(&self, index: usize) -> Param {
        assert!(index < self.param_len, "parameter index must be defined");
        self.params[index]
    }

    fn param_name(&self, index: usize) -> &'static str {
        self.param(index).name()
    }

    fn param_default(&self, index: usize) -> f32 {
        self.param(index).default()
    }

    fn param_count_named(&self, name: &str) -> usize {
        let mut count = 0;
        let mut param_index = 0;
        while param_index < self.param_len {
            if str_eq(self.params[param_index].name, name) {
                count += 1;
            }
            param_index += 1;
        }
        count
    }

    fn param_index(&self, name: &str, n: usize) -> usize {
        let mut found = 0;
        let mut slot = 0;
        while slot < self.param_len {
            if str_eq(self.params[slot].name, name) {
                if found == n {
                    return slot;
                }
                found += 1;
            }
            slot += 1;
        }
        panic!("parameter name not found or occurrence index out of range")
    }
}

impl Step {
    const fn offset_params(self, param_offset: usize, remember_offset: usize) -> Self {
        match self {
            Self::Yaw(arg) => Self::Yaw(arg.offset_param(param_offset)),
            Self::Pitch(arg) => Self::Pitch(arg.offset_param(param_offset)),
            Self::Roll(arg) => Self::Roll(arg.offset_param(param_offset)),
            Self::Move(arg) => Self::Move(arg.offset_param(param_offset)),
            Self::Left(arg) => Self::Left(arg.offset_param(param_offset)),
            Self::Up(arg) => Self::Up(arg.offset_param(param_offset)),
            Self::DiskParam(v) => Self::DiskParam(v.offset(param_offset)),
            Self::RingParam(v) => Self::RingParam(v.offset(param_offset)),
            Self::SphereParam(v) => Self::SphereParam(v.offset(param_offset)),
            Self::Restore { index } => Self::Restore {
                index: index + remember_offset,
            },
            other => other,
        }
    }
}

/// Iterator over all parameter indices with a given name.
///
/// Returned by [`LinkageFixed::param_indices`]. Scans forward through the param list,
/// so `.next()` gives the first definition and `.last()` gives the most recent one
/// (shadowing semantics — the one builder methods like `yaw_param` bind to).
pub struct ParamIndices<'a, const DOF: usize, const N: usize> {
    linkage: &'a LinkageFixed<DOF, N>,
    name: &'a str,
    pos: usize,
}

impl<'a, const DOF: usize, const N: usize> Iterator for ParamIndices<'a, DOF, N> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.linkage.param_len() {
            let i = self.pos;
            self.pos += 1;
            if str_eq(self.linkage.params[i].name, self.name) {
                return Some(i);
            }
        }
        None
    }
}

const fn str_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();

    if left.len() != right.len() {
        return false;
    }

    let mut byte_index = 0;
    while byte_index < left.len() {
        if left[byte_index] != right[byte_index] {
            return false;
        }
        byte_index += 1;
    }

    true
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
        | Step::Left(_)
        | Step::Up(_)
        | Step::PenUp
        | Step::PenDown
        | Step::PenColor(_)
        | Step::PenWidth(_)
        | Step::Disk(_)
        | Step::DiskParam(_)
        | Step::Ring(_)
        | Step::RingParam(_)
        | Step::Sphere(_)
        | Step::SphereParam(_)
        | Step::Mark { .. }
        | Step::Restore { .. } => return Mat3::IDENTITY,
    };
    match step {
        Step::Yaw(_) => Mat3::yaw(radians),
        Step::Pitch(_) => Mat3::pitch(radians),
        Step::Roll(_) => Mat3::roll(radians),
        Step::Start
        | Step::Move(_)
        | Step::Left(_)
        | Step::Up(_)
        | Step::PenUp
        | Step::PenDown
        | Step::PenColor(_)
        | Step::PenWidth(_)
        | Step::Disk(_)
        | Step::DiskParam(_)
        | Step::Ring(_)
        | Step::RingParam(_)
        | Step::Sphere(_)
        | Step::SphereParam(_)
        | Step::Mark { .. }
        | Step::Restore { .. } => Mat3::IDENTITY,
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
    color: Rgb888,
    width: f32,
}

impl PenStyle {
    /// Return the default down pen with white color and width 0.1.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pen: Pen::Down,
            color: Rgb888::new(255, 255, 255),
            width: 0.1,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    /// Return the current pen state.
    #[must_use]
    pub const fn pen(self) -> Pen {
        self.pen
    }

    /// Return the current pen color.
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }

    /// Return the current pen width.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }

    fn apply(&mut self, step: &Step) {
        match step {
            Step::Start => self.reset(),
            Step::PenUp => self.pen = Pen::Up,
            Step::PenDown => self.pen = Pen::Down,
            Step::PenColor(color) => self.color = *color,
            Step::PenWidth(width) => self.width = *width,
            Step::Yaw(_)
            | Step::Pitch(_)
            | Step::Roll(_)
            | Step::Move(_)
            | Step::Left(_)
            | Step::Up(_)
            | Step::Disk(_)
            | Step::DiskParam(_)
            | Step::Ring(_)
            | Step::RingParam(_)
            | Step::Sphere(_)
            | Step::SphereParam(_)
            | Step::Mark { .. }
            | Step::Restore { .. } => {}
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
            Step::Left(arg) => {
                self.position += self.orientation.left() * arg.resolve(params);
            }
            Step::Up(arg) => {
                self.position += self.orientation.up() * arg.resolve(params);
            }
            Step::Yaw(_) | Step::Pitch(_) | Step::Roll(_) => {
                self.orientation = self.orientation * rotation_matrix(step, params);
            }
            Step::PenUp
            | Step::PenDown
            | Step::PenColor(_)
            | Step::PenWidth(_)
            | Step::Disk(_)
            | Step::DiskParam(_)
            | Step::Ring(_)
            | Step::RingParam(_)
            | Step::Sphere(_)
            | Step::SphereParam(_)
            | Step::Mark { .. }
            | Step::Restore { .. } => {}
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
    pub const fn color(self) -> Rgb888 {
        self.pen_style.color()
    }

    /// Return this styled pose's pen width.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.pen_style.width()
    }
}

/// A drawable pen-down move segment produced by a linkage.
#[derive(Clone, Copy, Debug)]
pub struct StrokeSegment {
    start: Pose,
    end: Pose,
    color: Rgb888,
    width: f32,
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
    pub const fn color(self) -> Rgb888 {
        self.color
    }

    /// Return the segment pen width.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }
}

/// Iterator over poses produced by evaluating a linkage.
///
/// Yields one [`Pose`] after every linkage step, including the implicit [`Step::Start`].

/// Iterator over styled poses produced by evaluating a linkage.
///
/// Yields after every linkage step, including non-move steps and the implicit
/// [`Step::Start`].
#[derive(Clone, Copy)]
struct MarkedState {
    pose: Pose,
    pen_style: PenStyle,
}

pub struct StyledPoses<'a, const DOF: usize, const N: usize> {
    linkage: &'a LinkageFixed<DOF, N>,
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
    marked: [MarkedState; N],
    marked_len: usize,
}

impl<'a, const DOF: usize, const N: usize> StyledPoses<'a, DOF, N> {
    fn new(linkage: &'a LinkageFixed<DOF, N>, params: &'a [f32; DOF]) -> Self {
        validate_params(params);
        Self {
            linkage,
            params,
            index: 0,
            pose: Pose::start(),
            pen_style: PenStyle::new(),
            marked: [MarkedState {
                pose: Pose::start(),
                pen_style: PenStyle::new(),
            }; N],
            marked_len: 0,
        }
    }
}

impl<const DOF: usize, const N: usize> Iterator for StyledPoses<'_, DOF, N> {
    type Item = StyledPose;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index >= self.linkage.len {
                return None;
            }
            let step = &self.linkage.steps[self.index];
            self.index += 1;

            match step {
                Step::Mark { name: _ } => {
                    assert!(self.marked_len < N, "too many marked states");
                    self.marked[self.marked_len] = MarkedState {
                        pose: self.pose,
                        pen_style: self.pen_style,
                    };
                    self.marked_len += 1;
                    continue;
                }
                Step::Restore { index } => {
                    self.pose = self.marked[*index].pose;
                    self.pen_style = self.marked[*index].pen_style;
                    continue;
                }
                _ => {}
            }

            self.pose.apply(step, self.params);
            self.pen_style.apply(step);
            return Some(StyledPose {
                pose: self.pose,
                pen_style: self.pen_style,
            });
        }
    }
}

/// A disk shape yielded by a linkage at the current pose.
#[derive(Clone, Copy, Debug)]
pub struct DiskItem {
    pose: Pose,
    radius: f32,
    color: Rgb888,
}

impl DiskItem {
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }
}

/// A ring shape yielded by a linkage at the current pose. Stroke width is the pen width at that step.
#[derive(Clone, Copy, Debug)]
pub struct RingItem {
    pose: Pose,
    radius: f32,
    color: Rgb888,
    width: f32,
}

impl RingItem {
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }
}

/// A sphere shape yielded by a linkage at the current pose.
#[derive(Clone, Copy, Debug)]
pub struct SphereItem {
    pose: Pose,
    radius: f32,
    color: Rgb888,
}

impl SphereItem {
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }
}

/// A draw item produced by a linkage: a line stroke, a filled disk, a ring, or a sphere.
#[derive(Clone, Copy, Debug)]
pub enum DrawItem {
    Stroke(StrokeSegment),
    Disk(DiskItem),
    Ring(RingItem),
    Sphere(SphereItem),
}

/// Iterator over draw items (line strokes, disks, rings, spheres) produced by a linkage.
///
/// Translation steps with the pen down yield [`DrawItem::Stroke`]. Shape steps
/// always yield their respective variants. All other steps only update state.
pub struct DrawItems<'a, const DOF: usize, const N: usize> {
    linkage: &'a LinkageFixed<DOF, N>,
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
    marked: [MarkedState; N],
    marked_len: usize,
}

impl<'a, const DOF: usize, const N: usize> DrawItems<'a, DOF, N> {
    fn new(linkage: &'a LinkageFixed<DOF, N>, params: &'a [f32; DOF]) -> Self {
        validate_params(params);
        Self {
            linkage,
            params,
            index: 0,
            pose: Pose::start(),
            pen_style: PenStyle::new(),
            marked: [MarkedState {
                pose: Pose::start(),
                pen_style: PenStyle::new(),
            }; N],
            marked_len: 0,
        }
    }
}

impl<const DOF: usize, const N: usize> Iterator for DrawItems<'_, DOF, N> {
    type Item = DrawItem;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.linkage.len {
            let step = &self.linkage.steps[self.index];
            self.index += 1;

            match step {
                Step::Mark { name: _ } => {
                    assert!(self.marked_len < N, "too many marked states");
                    self.marked[self.marked_len] = MarkedState {
                        pose: self.pose,
                        pen_style: self.pen_style,
                    };
                    self.marked_len += 1;
                    continue;
                }
                Step::Restore { index } => {
                    self.pose = self.marked[*index].pose;
                    self.pen_style = self.marked[*index].pen_style;
                    continue;
                }
                _ => {}
            }

            let start_pose = self.pose;
            let pen_style = self.pen_style;
            self.pose.apply(step, self.params);
            self.pen_style.apply(step);

            match step {
                Step::Move(_) | Step::Left(_) | Step::Up(_)
                    if matches!(pen_style.pen(), Pen::Down) =>
                {
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
                Step::RingParam(var_arg) => {
                    return Some(DrawItem::Ring(RingItem {
                        pose: start_pose,
                        radius: var_arg.resolve(self.params),
                        color: pen_style.color(),
                        width: pen_style.width(),
                    }));
                }
                Step::Sphere(radius) => {
                    return Some(DrawItem::Sphere(SphereItem {
                        pose: start_pose,
                        radius: *radius,
                        color: pen_style.color(),
                    }));
                }
                Step::SphereParam(var_arg) => {
                    return Some(DrawItem::Sphere(SphereItem {
                        pose: start_pose,
                        radius: var_arg.resolve(self.params),
                        color: pen_style.color(),
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
    use super::{DrawItem, LinkageFixed, Pose, Vec3};
    use crate::test_helpers::{
        assert_png_matches_expected, assert_pose_approx_eq, assert_pose_trace_matches_expected,
        draw_linkage_xy_canvas,
    };
    use std::{boxed::Box, error::Error, vec::Vec};

    //todo000 *_param might not be a good suffix.
    const LINKAGE0: LinkageFixed<6, 24> = LinkageFixed::start()
        .define_param("raise hand", 0.5)
        .define_param("bend elbow", 0.5)
        .define_param("close hand", 0.5)
        .define_param("lower arm", 0.5)
        .define_param("spin whole arm", 0.5)
        .define_param("spin hand", 0.5)
        .yaw(90.0)
        .yaw_param("spin whole arm", 180.0, -180.0)
        .pitch(90.0)
        .forward(2.5)
        .pitch(-90.0)
        .pitch_param("lower arm", 30.0, 0.0)
        .forward(3.0)
        .yaw_param("bend elbow", 90.0, -90.0)
        .forward(3.0)
        .pitch_param("raise hand", 90.0, -90.0)
        .forward(1.0)
        .roll_param("spin hand", -180.0, 180.0)
        .forward(0.5)
        .yaw(90.0)
        .forward_param("close hand", 0.0, 0.5)
        .yaw(-90.0)
        .forward(1.0)
        .yaw(180.0)
        .forward(1.0)
        .yaw(90.0)
        .forward_param("close hand", 0.0, 1.0)
        .yaw(90.0)
        .forward(1.0);

    // todo0000 kill 2nd one
    const LINKAGE1: LinkageFixed<3, 16> = LinkageFixed::start()
        .define_param("spin whole arm", 0.5)
        .define_param("bend elbow", 0.5)
        .define_param("close hand", 0.5)
        .yaw(90.0)
        .yaw_param("spin whole arm", 180.0, -180.0)
        .forward(3.0)
        .yaw_param("bend elbow", 90.0, -90.0)
        .forward(3.0)
        .yaw(90.0)
        .forward_param("close hand", 0.5, 0.0)
        .yaw(-90.0)
        .forward(1.0)
        .yaw(-180.0)
        .forward(1.0)
        .yaw(90.0)
        .forward_param("close hand", 1.0, 0.0)
        .yaw(90.0)
        .forward(1.0);

    #[test]
    fn zero_pen_width_still_draws() {
        const LINKAGE: LinkageFixed<0, 4> = LinkageFixed::start().pen_width(0.0).forward(1.0);

        let params = [];
        let draw_item = LINKAGE
            .draw_items(&params)
            .next()
            .expect("zero-width pen should still produce a stroke");

        match draw_item {
            DrawItem::Stroke(stroke_segment) => {
                assert_eq!(stroke_segment.width(), 0.0);
            }
            _ => panic!("expected stroke from zero-width pen"),
        }
    }

    #[test]
    fn forward_moves_along_positive_x() {
        const LINKAGE: LinkageFixed<0, 2> = LinkageFixed::start().forward(10.0);

        let params = [];
        let actual = LINKAGE.final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([10.0, 0.0, 0.0]), 1e-6));
    }

    #[test]
    fn yaw_then_forward_moves_along_positive_y() {
        const LINKAGE: LinkageFixed<0, 3> = LinkageFixed::start().yaw(90.0).forward(10.0);

        let params = [];
        let actual = LINKAGE.final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 10.0, 0.0]), 1e-5));
    }

    #[test]
    fn left_moves_along_positive_y() {
        const LINKAGE: LinkageFixed<0, 2> = LinkageFixed::start().left(10.0);

        let params = [];
        let actual = LINKAGE.final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 10.0, 0.0]), 1e-6));
    }

    #[test]
    fn up_moves_along_positive_z() {
        const LINKAGE: LinkageFixed<0, 2> = LinkageFixed::start().up(10.0);

        let params = [];
        let actual = LINKAGE.final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 0.0, 10.0]), 1e-6));
    }

    #[test]
    fn translation_params_move_along_named_axes() {
        const LINKAGE: LinkageFixed<3, 7> = LinkageFixed::start()
            .define_param("forward", 0.5)
            .define_param("left", 0.5)
            .define_param("up", 0.5)
            .forward_param("forward", 0.0, 10.0)
            .left_param("left", 0.0, 20.0)
            .up_param("up", 0.0, 30.0);

        let params = [0.2, 0.3, 0.4];
        let actual = LINKAGE.final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([2.0, 6.0, 12.0]), 1e-6));
    }

    #[test]
    fn planar_two_link_arm_uses_yaw_then_forward() {
        const LINKAGE: LinkageFixed<0, 5> = LinkageFixed::start()
            .yaw(0.0)
            .forward(10.0)
            .yaw(90.0)
            .forward(5.0);

        let params = [];
        let actual = LINKAGE.final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([10.0, 5.0, 0.0]), 1e-5));
    }

    #[test]
    fn test_excel_pose_trace0_matches_expected() -> Result<(), Box<dyn Error>> {
        // Fractions for [raise hand, bend elbow, close hand,
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
            0.7514501463, // raise hand
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
            0.5, // raise hand
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
        // Fractions for [raise hand, bend elbow, close hand,
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
            0.0, // raise hand
            0.5, // bend elbow
            1.1, // close hand, invalid param
            1.0, // lower arm
            0.0, // spin whole arm
            0.5, // spin hand
        ];

        let _ = LINKAGE0.final_pose(&params);
    }

    // ── Shadowing semantics ───────────────────────────────────────────────────
    //
    // A param name may appear more than once in a linkage.  Builder methods like
    // `yaw_param` bind to the *most recently defined* param with that name —
    // this is "shadowing".  The earlier definition is not removed; it still
    // occupies its slot in the param array and can be reached via
    // `param_indices(...).next()`.

    #[test]
    fn duplicate_define_param_does_not_panic() {
        // Simply building a linkage with a duplicate name must succeed.
        // (Previously this would panic with "duplicate parameter name".)
        const _: LinkageFixed<2, 2> = LinkageFixed::start()
            .define_param("angle", 0.25) // index 0
            .define_param("angle", 0.75); // index 1 — shadows index 0
    }

    #[test]
    fn shadowing_builder_binds_to_most_recent_definition() {
        // "angle" is defined twice.  yaw_param("angle") bakes in the index of
        // the second definition (index 1), not the first (index 0).
        //
        // We verify this by setting params = [1.0, 0.0]:
        //   - if bound to index 0 → yaw 90° → forward lands at ~(0, 10, 0)
        //   - if bound to index 1 → yaw  0° → forward lands at (10, 0, 0)  ✓
        const LINKAGE: LinkageFixed<2, 5> = LinkageFixed::start()
            .define_param("angle", 0.0) // index 0, default 0.0
            .define_param("angle", 1.0) // index 1, default 1.0 — shadows index 0
            .yaw_param("angle", 0.0, 90.0) // binds to index 1 (most recent)
            .forward(10.0);

        let params = [1.0, 0.0]; // index 0 = full, index 1 = zero
        let pos = LINKAGE.final_pose(&params).position();
        // yaw driven by index 1 = 0.0 → 0° → moves along +X
        assert!(pos.is_close_to(&Vec3::from([10.0, 0.0, 0.0]), 1e-5));
    }

    #[test]
    fn param_indices_returns_all_matches_in_forward_order() {
        // Three params: "spin" at 0, "angle" at 1, "spin" again at 2.
        // param_indices("spin") should yield indices 0 and 2 in that order.
        const LINKAGE: LinkageFixed<3, 2> = LinkageFixed::start()
            .define_param("spin", 0.2) // index 0
            .define_param("angle", 0.5) // index 1
            .define_param("spin", 0.8); // index 2

        let indices: Vec<usize> = LINKAGE.param_indices("spin").collect();
        assert_eq!(indices, [0, 2]);
    }

    #[test]
    fn param_indices_next_gives_first_definition() {
        const LINKAGE: LinkageFixed<3, 2> = LinkageFixed::start()
            .define_param("spin", 0.2)
            .define_param("angle", 0.5)
            .define_param("spin", 0.8);

        // .next() iterates forward — first hit is the earliest definition
        assert_eq!(LINKAGE.param_indices("spin").next(), Some(0));
    }

    #[test]
    fn param_indices_last_gives_most_recent_definition() {
        const LINKAGE: LinkageFixed<3, 2> = LinkageFixed::start()
            .define_param("spin", 0.2)
            .define_param("angle", 0.5)
            .define_param("spin", 0.8);

        // .last() exhausts the iterator forward and returns the final element,
        // which is the most recently defined — the shadowing definition.
        assert_eq!(LINKAGE.param_indices("spin").last(), Some(2));
    }

    #[test]
    fn param_indices_count_shows_how_many_times_a_name_appears() {
        const LINKAGE: LinkageFixed<3, 2> = LinkageFixed::start()
            .define_param("spin", 0.2)
            .define_param("angle", 0.5)
            .define_param("spin", 0.8);

        assert_eq!(LINKAGE.param_indices("spin").count(), 2);
        assert_eq!(LINKAGE.param_indices("angle").count(), 1);
    }

    #[test]
    fn param_indices_returns_empty_for_unknown_name() {
        const LINKAGE: LinkageFixed<1, 2> = LinkageFixed::start().define_param("spin", 0.5);

        assert_eq!(LINKAGE.param_indices("unknown").next(), None);
        assert_eq!(LINKAGE.param_indices("unknown").count(), 0);
    }

    #[test]
    fn combine_each_piece_can_have_its_own_names_or_shared_names() {
        // After combine, each piece's params appear in order: A's first, then B's.
        // If both define a name, param_indices returns both indices.
        const A: LinkageFixed<1, 2> = LinkageFixed::start().define_param("angle", 0.25); // index 0
        const B: LinkageFixed<1, 2> = LinkageFixed::start().define_param("angle", 0.75); // becomes index 1
        const COMBINED: LinkageFixed<2, 3> = A.combine::<1, 2, 2, 3>(B);

        // Both definitions are visible
        let indices: Vec<usize> = COMBINED.param_indices("angle").collect();
        assert_eq!(indices, [0, 1]);

        // The shadowing definition (most recent, from B) is at index 1
        assert_eq!(COMBINED.param_indices("angle").last(), Some(1));

        // The first definition (from A) is at index 0
        assert_eq!(COMBINED.param_indices("angle").next(), Some(0));
    }
}
