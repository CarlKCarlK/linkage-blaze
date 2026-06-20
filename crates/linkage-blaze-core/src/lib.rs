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

#[cfg(feature = "alloc")]
extern crate alloc;

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

    // todo000 maybe later not static.
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

/// A linkage expression/storage type that can expose a borrowed view for evaluation and rendering.
///
/// This trait provides a minimal, uniform interface for linkage types. The only required method is
/// [`view()`](Linkage::view), which returns a [`LinkageView`] where all evaluation and rendering
/// happens. This keeps the API clean by separating expression/storage from evaluation.
///
/// # Examples
///
/// ```rust
/// # use linkage_blaze_core::{Linkage, LinkageFixed};
/// const LINKAGE: LinkageFixed<1, 8> = LinkageFixed::start()
///     .define_param("distance", 0.5)
///     .forward_param("distance", 1.0, 5.0);
///
/// // Get a view and evaluate
/// let pose = LINKAGE.view().final_pose(&[0.5]);
/// ```
pub trait Linkage<const DOF: usize> {
    /// Create a borrowed view for evaluation and rendering.
    ///
    /// All evaluation methods (final_pose, poses, draw_items, etc.) are available on the returned
    /// [`LinkageView`]. This keeps the conceptual model clean: storage types (LinkageFixed, LinkageBuf)
    /// define expressions; views evaluate them.
    fn view(&self) -> LinkageView<'_, DOF>;

    /// Return the number of runtime parameters (degrees of freedom).
    ///
    /// Default implementation: returns `DOF`.
    fn dof(&self) -> usize {
        DOF
    }

    /// Return the number of linkage steps, including the implicit start step.
    ///
    /// Default implementation: delegates to the view.
    fn len(&self) -> usize {
        self.view().len()
    }

    /// Return a reference to the parameter array.
    ///
    /// Default implementation: delegates to the view.
    fn params(&self) -> &[Param; DOF] {
        self.view().params()
    }

    /// Return a reference to the step slice.
    ///
    /// Default implementation: delegates to the view.
    fn steps(&self) -> &[Step] {
        self.view().steps()
    }
}

/// A borrowed view of a linkage for evaluation and rendering.
///
/// `LinkageView` erases the step capacity `N` while preserving the degree-of-freedom `DOF`.
/// It borrows both the parameter array and the active step slice from a `LinkageFixed`.
///
/// # Examples
///
/// ```rust
/// # use linkage_blaze_core::{LinkageFixed, Vec3};
/// const LINKAGE: LinkageFixed<1, 8> = LinkageFixed::start()
///     .define_param("distance", 0.5)
///     .forward_param("distance", 1.0, 5.0);
///
/// let view = LINKAGE.view();
/// let pose = view.final_pose(&[0.5]);
/// assert!(pose.position().is_close_to(&Vec3::from([3.0, 0.0, 0.0]), 1e-5));
/// ```
/// A borrowed view of a linkage for evaluation and rendering.
///
/// `LinkageView` erases the step capacity `N` while preserving the degree-of-freedom `DOF`.
/// It borrows both the parameter array and the active step slice from a `LinkageFixed`.
///
/// Create a view via [`LinkageFixed::view()`] or convert from `&LinkageFixed` with `From`.
#[derive(Clone, Copy)]
pub struct LinkageView<'a, const DOF: usize> {
    params: &'a [Param; DOF],
    steps: &'a [Step],
}

impl<'a, const DOF: usize> LinkageView<'a, DOF> {
    /// The number of runtime parameters (degrees of freedom).
    pub const DOF: usize = DOF;

    /// Create a new linkage view from parameter and step arrays.
    #[must_use]
    pub(crate) const fn new(params: &'a [Param; DOF], steps: &'a [Step]) -> Self {
        Self { params, steps }
    }

    /// Return the number of runtime parameters (degrees of freedom).
    #[must_use]
    pub const fn dof(&self) -> usize {
        DOF
    }

    /// Return the number of linkage steps, including the implicit start step.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.steps.len()
    }

    /// Return a parameter definition by index.
    ///
    /// # Panics
    ///
    /// Panics if `index >= dof()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::{LinkageFixed, Vec3};
    /// const LINKAGE: LinkageFixed<2, 8> = LinkageFixed::start()
    ///     .define_param("yaw", 0.5)
    ///     .define_param("distance", 0.75);
    ///
    /// let view = LINKAGE.view();
    /// let param = view.param(0);
    /// assert_eq!(param.name(), "yaw");
    /// assert_eq!(param.default(), 0.5);
    /// ```
    #[must_use]
    pub const fn param(&self, index: usize) -> Param {
        self.params[index]
    }

    /// Return a reference to the parameter array.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<2, 8> = LinkageFixed::start()
    ///     .define_param("x", 0.0)
    ///     .define_param("y", 0.5);
    ///
    /// let view = LINKAGE.view();
    /// let params = view.params();
    /// assert_eq!(params.len(), 2);
    /// ```
    #[must_use]
    pub const fn params(&self) -> &'a [Param; DOF] {
        self.params
    }

    /// Return a reference to the step slice.
    #[must_use]
    pub const fn steps(&self) -> &'a [Step] {
        self.steps
    }

    /// Return the index of the `n`th parameter (0-based) with the given name.
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
    ///     .define_param("x", 0.8);
    ///
    /// let view = LINKAGE.view();
    /// assert_eq!(view.param_index("x", 0), 0);  // first "x"
    /// assert_eq!(view.param_index("x", 1), 2);  // second "x"
    /// ```
    #[must_use]
    pub fn param_index(&self, name: &str, n: usize) -> usize {
        let mut found = 0;
        for i in 0..DOF {
            if str_eq(self.params[i].name, name) {
                if found == n {
                    return i;
                }
                found += 1;
            }
        }
        panic!("parameter name not found or occurrence index out of range")
    }

    /// Return the final pose after evaluating all steps.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::{LinkageFixed, Vec3};
    /// const LINKAGE: LinkageFixed<2, 8> = LinkageFixed::start()
    ///     .define_param("yaw", 0.5)
    ///     .define_param("distance", 0.5)
    ///     .yaw_param("yaw", -90.0, 90.0)
    ///     .forward_param("distance", 1.0, 5.0);
    ///
    /// let view = LINKAGE.view();
    /// let pose = view.final_pose(&[0.5, 0.6]);
    /// assert!(pose.position().is_close_to(&Vec3::from([3.4, 0.0, 0.0]), 1e-5));
    /// ```
    pub fn final_pose(&self, params: &[f32; DOF]) -> Pose {
        self.poses(params)
            .last()
            .expect("linkage must yield at least the implicit start pose")
    }

    /// Iterate over all intermediate poses produced by evaluating this linkage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::{LinkageFixed, Vec3};
    /// const LINKAGE: LinkageFixed<1, 8> = LinkageFixed::start()
    ///     .define_param("distance", 0.5)
    ///     .forward_param("distance", 1.0, 5.0);
    ///
    /// let view = LINKAGE.view();
    /// let mut poses = view.poses(&[0.5]);
    /// let start = poses.next().expect("linkage always has start pose");
    /// assert!(start.position().is_close_to(&Vec3::from([0.0, 0.0, 0.0]), 1e-5));
    /// let end = poses.next().expect("forward step exists");
    /// assert!(end.position().is_close_to(&Vec3::from([3.0, 0.0, 0.0]), 1e-5));
    /// ```
    pub fn poses<'b>(&'b self, params: &'b [f32; DOF]) -> impl Iterator<Item = Pose> + 'b {
        self.styled_poses(params).map(|sp| sp.pose())
    }

    /// Iterate over all styled poses with their pen state.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::{LinkageFixed, Vec3, PenState};
    /// const LINKAGE: LinkageFixed<0, 8> = LinkageFixed::start()
    ///     .forward(1.0)
    ///     .forward(2.0);
    ///
    /// let view = LINKAGE.view();
    /// let mut styled = view.styled_poses(&[]);
    /// let start = styled.next().expect("has start");
    /// assert!(start.pose().position().is_close_to(&Vec3::from([0.0, 0.0, 0.0]), 1e-5));
    /// assert_eq!(start.pen(), PenState::Down);
    /// let _ = styled.next();
    /// let end = styled.next().expect("has second forward");
    /// assert!(end.pose().position()[0] > 2.9);
    /// ```
    pub fn styled_poses<'b>(
        &'b self,
        params: &'b [f32; DOF],
    ) -> impl Iterator<Item = StyledPose> + 'b {
        StyledPosesView::new(self.steps, params)
    }

    /// Iterate over all draw items produced by this linkage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::{LinkageFixed, DrawItem};
    /// const LINKAGE: LinkageFixed<0, 8> = LinkageFixed::start()
    ///     .forward(1.0)
    ///     .forward(2.0);
    ///
    /// let view = LINKAGE.view();
    /// let has_stroke = view.draw_items(&[])
    ///     .any(|item| matches!(item, DrawItem::Stroke(_)));
    /// assert!(has_stroke);
    /// ```
    pub fn draw_items<'b>(&'b self, params: &'b [f32; DOF]) -> impl Iterator<Item = DrawItem> + 'b {
        DrawItemsView::new(self.steps, params)
    }
}

impl<'a, const DOF: usize, const N: usize> From<&'a LinkageFixed<DOF, N>> for LinkageView<'a, DOF> {
    fn from(linkage: &'a LinkageFixed<DOF, N>) -> Self {
        linkage.view()
    }
}


/// Emit const fn fluent DSL methods for LinkageFixed.
/// These are the simple one-step methods that work the same way for both storage types.
macro_rules! emit_fixed_step_methods {
    () => {
        // Fixed-argument methods (yaw, pitch, roll, forward, etc.)
        pub const fn yaw(self, degrees: f32) -> Self {
            self.push(Step::Yaw(Arg::Fixed(degrees_to_radians(degrees))))
        }
        pub const fn pitch(self, degrees: f32) -> Self {
            self.push(Step::Pitch(Arg::Fixed(degrees_to_radians(degrees))))
        }
        pub const fn roll(self, degrees: f32) -> Self {
            self.push(Step::Roll(Arg::Fixed(degrees_to_radians(degrees))))
        }
        pub const fn forward(self, distance: f32) -> Self {
            self.push(Step::Move(Arg::Fixed(distance)))
        }
        pub const fn left(self, distance: f32) -> Self {
            self.push(Step::Left(Arg::Fixed(distance)))
        }
        pub const fn up(self, distance: f32) -> Self {
            self.push(Step::Up(Arg::Fixed(distance)))
        }
        pub const fn pen_up(self) -> Self {
            self.push(Step::PenUp)
        }
        pub const fn pen_down(self) -> Self {
            self.push(Step::PenDown)
        }
        pub const fn pen_color(self, color: Rgb888) -> Self {
            self.push(Step::PenColor(color))
        }
        pub const fn pen_width(self, width: f32) -> Self {
            assert!(width >= 0.0, "pen width must be non-negative");
            self.push(Step::PenWidth(width))
        }
        pub const fn disk(self, radius: f32) -> Self {
            self.push(Step::Disk(radius))
        }
        pub const fn ring(self, radius: f32) -> Self {
            self.push(Step::Ring(radius))
        }
        pub const fn sphere(self, radius: f32) -> Self {
            self.push(Step::Sphere(radius))
        }

        // Parameterized methods (yaw_param, pitch_param, etc.)
        pub const fn yaw_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Yaw(Arg::Variable(VariableArg::from_degrees(index, low, high))))
        }
        pub const fn pitch_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Pitch(Arg::Variable(VariableArg::from_degrees(index, low, high))))
        }
        pub const fn roll_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Roll(Arg::Variable(VariableArg::from_degrees(index, low, high))))
        }
        pub const fn forward_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Move(Arg::Variable(VariableArg::new(index, low, high))))
        }
        pub const fn left_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Left(Arg::Variable(VariableArg::new(index, low, high))))
        }
        pub const fn up_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Up(Arg::Variable(VariableArg::new(index, low, high))))
        }
        pub const fn disk_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::DiskParam(VariableArg::new(index, low, high)))
        }
        pub const fn ring_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::RingParam(VariableArg::new(index, low, high)))
        }
        pub const fn sphere_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::SphereParam(VariableArg::new(index, low, high)))
        }

        // Restore methods
        pub const fn restore(self, name: &'static str) -> Self {
            let index = match self.last_mark_index(name) {
                Some(i) => i,
                None => panic!("restore: no mark found with name (mark must be defined before restore)"),
            };
            self.push(Step::Restore { index })
        }
        pub const fn restore_nth(self, name: &'static str, n: usize) -> Self {
            let index = match self.mark_index_nth(name, n) {
                Some(i) => i,
                None => panic!("restore_nth: no matching mark found (must define mark before restoring nth)"),
            };
            self.push(Step::Restore { index })
        }
    };
}

/// Emit ordinary fn fluent DSL methods for LinkageBuf.
/// These are the simple one-step methods that work the same way for both storage types.
#[cfg(feature = "alloc")]
macro_rules! emit_buf_step_methods {
    () => {
        // Fixed-argument methods (yaw, pitch, roll, forward, etc.)
        pub fn yaw(self, degrees: f32) -> Self {
            self.push_step(Step::Yaw(Arg::Fixed(degrees_to_radians(degrees))))
        }
        pub fn pitch(self, degrees: f32) -> Self {
            self.push_step(Step::Pitch(Arg::Fixed(degrees_to_radians(degrees))))
        }
        pub fn roll(self, degrees: f32) -> Self {
            self.push_step(Step::Roll(Arg::Fixed(degrees_to_radians(degrees))))
        }
        pub fn forward(self, distance: f32) -> Self {
            self.push_step(Step::Move(Arg::Fixed(distance)))
        }
        pub fn left(self, distance: f32) -> Self {
            self.push_step(Step::Left(Arg::Fixed(distance)))
        }
        pub fn up(self, distance: f32) -> Self {
            self.push_step(Step::Up(Arg::Fixed(distance)))
        }
        pub fn pen_up(self) -> Self {
            self.push_step(Step::PenUp)
        }
        pub fn pen_down(self) -> Self {
            self.push_step(Step::PenDown)
        }
        pub fn pen_color(self, color: Rgb888) -> Self {
            self.push_step(Step::PenColor(color))
        }
        pub fn pen_width(self, width: f32) -> Self {
            assert!(width >= 0.0, "pen width must be non-negative");
            self.push_step(Step::PenWidth(width))
        }
        pub fn disk(self, radius: f32) -> Self {
            self.push_step(Step::Disk(radius))
        }
        pub fn ring(self, radius: f32) -> Self {
            self.push_step(Step::Ring(radius))
        }
        pub fn sphere(self, radius: f32) -> Self {
            self.push_step(Step::Sphere(radius))
        }

        // Parameterized methods (yaw_param, pitch_param, etc.)
        pub fn yaw_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Yaw(Arg::Variable(VariableArg::from_degrees(index, low, high))))
        }
        pub fn pitch_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Pitch(Arg::Variable(VariableArg::from_degrees(index, low, high))))
        }
        pub fn roll_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Roll(Arg::Variable(VariableArg::from_degrees(index, low, high))))
        }
        pub fn forward_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Move(Arg::Variable(VariableArg::new(index, low, high))))
        }
        pub fn left_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Left(Arg::Variable(VariableArg::new(index, low, high))))
        }
        pub fn up_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Up(Arg::Variable(VariableArg::new(index, low, high))))
        }
        pub fn disk_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::DiskParam(VariableArg::new(index, low, high)))
        }
        pub fn ring_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::RingParam(VariableArg::new(index, low, high)))
        }
        pub fn sphere_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::SphereParam(VariableArg::new(index, low, high)))
        }

        // Restore methods - handled specially for LinkageBuf
    };
}

/// A fixed-capacity const linkage expression/storage type.
///
/// `LinkageFixed` stores linkage steps and parameters in fixed-size arrays, enabling
/// `const` construction and evaluation without allocation. Use the fluent DSL methods
/// to extend the linkage expression and define complex arm kinematics at compile time.
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

    /// Number of runtime parameters this linkage expects.
    pub const DOF: usize = DOF;

    /// Step-slot capacity of this linkage.
    pub const N: usize = N;

    /// Create a borrowed view for evaluation and rendering.
    ///
    /// The view erases the step capacity `N` while preserving the degree-of-freedom `DOF`.
    /// All evaluation methods (poses, draw_items, etc.) operate on the view.
    #[must_use]
    #[inline]
    pub fn view(&self) -> LinkageView<'_, DOF> {
        LinkageView::new(&self.params, &self.steps[..self.len])
    }

    /// Return a parameter definition by index (const accessor for internal use).
    #[must_use]
    pub const fn param(&self, index: usize) -> Param {
        assert!(index < self.param_len, "parameter index must be defined");
        self.params[index]
    }

    /// Return the number of runtime parameters this linkage expects.
    #[must_use]
    pub const fn dof(&self) -> usize {
        DOF
    }

    /// Return the index of the `n`th parameter (0-based) with the given name (const accessor for internal use).
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
    /// a DSL method like `yaw_param` looks up the name.
    pub const fn define_param(mut self, name: &'static str, default: f32) -> Self {
        assert!(self.param_len < DOF, "linkage has more params than DOF");
        assert!(default >= 0.0, "parameter default must be at least 0.0");
        assert!(default <= 1.0, "parameter default must be at most 1.0");
        self.params[self.param_len] = Param { name, default };
        self.param_len += 1;
        self
    }

    // ── Fluent DSL methods (generated from emit_fixed_step_methods macro) ──
    // To add a new simple step method, edit the macro, not this impl block.
    emit_fixed_step_methods!();

    /// Save the current pose and pen state under a name for later recall.
    pub const fn mark(mut self, name: &'static str) -> Self {
        assert!(self.mark_len < N, "linkage has more marks than N");
        self.mark_names[self.mark_len] = name;
        self.mark_len += 1;
        self.push(Step::Mark { name })
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

impl<const DOF: usize, const N: usize> Linkage<DOF> for LinkageFixed<DOF, N> {
    fn view(&self) -> LinkageView<'_, DOF> {
        LinkageView::new(&self.params, &self.steps[..self.len])
    }

    fn len(&self) -> usize {
        self.len
    }

    fn params(&self) -> &[Param; DOF] {
        &self.params
    }

    fn steps(&self) -> &[Step] {
        &self.steps[..self.len]
    }
}

impl<'a, const DOF: usize> Linkage<DOF> for LinkageView<'a, DOF> {
    fn view(&self) -> LinkageView<'_, DOF> {
        *self
    }
}

#[cfg(feature = "alloc")]
/// A growable linkage expression/storage type.
///
/// `LinkageBuf` stores linkage steps in a [`Vec`](alloc::vec::Vec) and parameters in an array,
/// allowing dynamic growth at runtime. Unlike [`LinkageFixed`], construction is not `const`,
/// but the fluent DSL methods and evaluation interface are identical.
///
/// **Note:** `LinkageBuf` requires the `alloc` feature. Enable it in `Cargo.toml`:
/// ```toml
/// linkage-blaze-core = { features = ["alloc"] }
/// ```
///
/// Use [`LinkageBuf::start()`] to begin a linkage expression, then chain fluent DSL methods to extend
/// it. Call [`view()`](LinkageBuf::view) to create a borrowed view for evaluation and rendering.
///
/// # Building linkage expressions
///
/// ```rust
/// # #[cfg(feature = "alloc")]
/// # {
/// # use linkage_blaze_core::{LinkageBuf, Vec3};
/// let linkage = LinkageBuf::start()
///     .define_param("distance", 0.5)
///     .forward_param("distance", 1.0, 5.0);
///
/// let pose = linkage.view().final_pose(&[0.5]);
/// assert!(pose.position().is_close_to(&Vec3::from([3.0, 0.0, 0.0]), 1e-5));
/// # }
/// ```
///
/// # Converting from fixed storage
///
/// ```rust
/// # #[cfg(feature = "alloc")]
/// # {
/// # use linkage_blaze_core::{LinkageFixed, LinkageBuf, Vec3};
/// const FIXED: LinkageFixed<1, 8> = LinkageFixed::start()
///     .define_param("distance", 0.5)
///     .forward_param("distance", 1.0, 5.0);
///
/// let buf = LinkageBuf::from(&FIXED);
/// let pose = buf.view().final_pose(&[0.5]);
/// assert!(pose.position().is_close_to(&Vec3::from([3.0, 0.0, 0.0]), 1e-5));
/// # }
/// ```
pub struct LinkageBuf<const DOF: usize> {
    params: [Param; DOF],
    param_len: usize,
    steps: alloc::vec::Vec<Step>,
    mark_names: alloc::vec::Vec<&'static str>,
}

#[cfg(feature = "alloc")]
impl<const DOF: usize> LinkageBuf<DOF> {
    /// Start a growable linkage with an implicit origin.
    pub fn start() -> Self {
        Self {
            params: [Param::EMPTY; DOF],
            param_len: 0,
            steps: {
                let mut v = alloc::vec::Vec::new();
                v.push(Step::Start);
                v
            },
            mark_names: alloc::vec::Vec::new(),
        }
    }

    /// Number of runtime parameters this linkage expects.
    pub const DOF: usize = DOF;

    /// Create a borrowed view for evaluation and rendering.
    ///
    /// The view erases the step capacity while preserving the degree-of-freedom `DOF`.
    /// All evaluation methods (poses, draw_items, etc.) operate on the view.
    #[must_use]
    #[inline]
    pub fn view(&self) -> LinkageView<'_, DOF> {
        LinkageView::new(&self.params, &self.steps)
    }

    /// Define a named runtime parameter, extending the linkage expression.
    ///
    /// Duplicate names are allowed; later definitions shadow earlier ones when
    /// a DSL method like `yaw_param` looks up the name.
    pub fn define_param(mut self, name: &'static str, default: f32) -> Self {
        assert!(self.param_len < DOF, "linkage has more params than DOF");
        assert!(default >= 0.0, "parameter default must be at least 0.0");
        assert!(default <= 1.0, "parameter default must be at most 1.0");
        self.params[self.param_len] = Param { name, default };
        self.param_len += 1;
        self
    }

    // ── Fluent DSL methods (generated from emit_buf_step_methods macro) ──
    // To add a new simple step method, edit the macro, not this impl block.
    emit_buf_step_methods!();

    /// Save the current pose and pen state under a name for later recall.
    pub fn mark(mut self, name: &'static str) -> Self {
        self.mark_names.push(name);
        self.push_step_internal(Step::Mark { name });
        self
    }

    /// Restore a previously marked pose and pen state.
    /// Resolves `name` at build time using last-definition-wins (shadowing) semantics.
    pub fn restore(self, name: &'static str) -> Self {
        let index = match self.last_mark_index(name) {
            Some(i) => i,
            None => {
                panic!("restore: no mark found with name (mark must be defined before restore)")
            }
        };
        self.push_step(Step::Restore { index })
    }

    /// Restore the `n`th marked pose with the given name (0 = first definition).
    /// Resolves at build time.
    pub fn restore_nth(self, name: &'static str, n: usize) -> Self {
        let index = match self.mark_index_nth(name, n) {
            Some(i) => i,
            None => panic!(
                "restore_nth: no matching mark found (must define mark before restoring nth)"
            ),
        };
        self.push_step(Step::Restore { index })
    }


    fn push_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    fn push_step_internal(&mut self, step: Step) {
        self.steps.push(step);
    }

    fn last_mark_index(&self, name: &str) -> Option<usize> {
        let mut i = self.mark_names.len();
        while i > 0 {
            i -= 1;
            if str_eq(self.mark_names[i], name) {
                return Some(i);
            }
        }
        None
    }

    fn mark_index_nth(&self, name: &str, n: usize) -> Option<usize> {
        let mut count = 0;
        for (i, &mark_name) in self.mark_names.iter().enumerate() {
            if str_eq(mark_name, name) {
                if count == n {
                    return Some(i);
                }
                count += 1;
            }
        }
        None
    }

    fn last_param_index(&self, name: &str) -> Option<usize> {
        let mut i = self.param_len;
        while i > 0 {
            i -= 1;
            if str_eq(self.params[i].name, name) {
                return Some(i);
            }
        }
        None
    }

    fn expect_param_index(&self, name: &str) -> usize {
        match self.last_param_index(name) {
            Some(index) => index,
            None => panic!("unknown parameter name"),
        }
    }

    /// Append another linkage buffer's steps and parameters, creating a new buffer with combined DOF.
    ///
    /// This method consumes both buffers and produces a new one with DOF_OUT = DOF + DOF2.
    /// Parameters from `other` are concatenated after parameters from `self`.
    /// Steps from `other` (excluding its implicit Start step) are appended after this linkage's steps,
    /// with parameter and mark indices offset appropriately.
    ///
    /// The caller must provide the expected output DOF as an explicit type parameter,
    /// matching `DOF + DOF2`. This mirrors the pattern used in `LinkageFixed::combine`.
    ///
    /// # Panics
    ///
    /// Panics if `DOF_OUT != DOF + DOF2`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "alloc")]
    /// # {
    /// # use linkage_blaze_core::{LinkageBuf, Vec3};
    /// let a = LinkageBuf::<1>::start()
    ///     .define_param("x", 0.5)
    ///     .forward_param("x", 0.0, 10.0);
    ///
    /// let b = LinkageBuf::<1>::start()
    ///     .define_param("y", 0.5)
    ///     .left_param("y", 0.0, 5.0);
    ///
    /// let c: LinkageBuf<2> = a.append::<1, 2>(b);
    /// let params = [0.5, 0.5];
    /// let pose = c.view().final_pose(&params);
    /// # }
    /// ```
    pub fn append<const DOF2: usize, const DOF_OUT: usize>(
        self,
        other: LinkageBuf<DOF2>,
    ) -> LinkageBuf<DOF_OUT> {
        assert!(DOF_OUT == DOF + DOF2, "DOF_OUT must equal DOF + DOF2");

        let mut out = LinkageBuf {
            params: [Param::EMPTY; DOF_OUT],
            param_len: 0,
            steps: alloc::vec::Vec::new(),
            mark_names: alloc::vec::Vec::new(),
        };

        // Copy self's steps (including Start)
        out.steps.extend_from_slice(&self.steps);

        // Append other's steps (skip Start), offsetting param and mark indices
        let mark_offset = self.mark_names.len();
        for i in 1..other.steps.len() {
            let step = other.steps[i].offset_params(DOF, mark_offset);
            out.steps.push(step);
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

        // Copy mark names from both
        out.mark_names.extend_from_slice(&self.mark_names);
        out.mark_names.extend_from_slice(&other.mark_names);

        out
    }

    /// Extend this linkage with steps and parameters from a linkage view.
    ///
    /// Similar to `append`, but copies steps and parameters from a `LinkageView` instead.
    /// This is useful when you have a view of an existing linkage.
    ///
    /// # Panics
    ///
    /// Panics if `DOF_OUT != DOF + DOF2`.
    pub fn extend_view<const DOF2: usize, const DOF_OUT: usize>(
        self,
        other: LinkageView<'_, DOF2>,
    ) -> LinkageBuf<DOF_OUT> {
        assert!(DOF_OUT == DOF + DOF2, "DOF_OUT must equal DOF + DOF2");

        let mut out = LinkageBuf {
            params: [Param::EMPTY; DOF_OUT],
            param_len: 0,
            steps: alloc::vec::Vec::new(),
            mark_names: alloc::vec::Vec::new(),
        };

        // Copy self's steps (including Start)
        out.steps.extend_from_slice(&self.steps);

        // Append steps from the view (skip Start)
        let mark_offset = self.mark_names.len();
        let view_steps = other.steps();
        for i in 1..view_steps.len() {
            let step = view_steps[i].offset_params(DOF, mark_offset);
            out.steps.push(step);
        }

        // Copy self's params
        let mut i = 0;
        while i < self.param_len {
            out.params[i] = self.params[i];
            i += 1;
        }

        // Copy params from the view
        let view_params = other.params();
        let mut i = 0;
        while i < DOF2 {
            out.params[DOF + i] = view_params[i];
            i += 1;
        }
        out.param_len = self.param_len + DOF2;

        // Copy mark names from self
        out.mark_names.extend_from_slice(&self.mark_names);

        out
    }
}

#[cfg(feature = "alloc")]
impl<const DOF: usize> Linkage<DOF> for LinkageBuf<DOF> {
    fn view(&self) -> LinkageView<'_, DOF> {
        self.view()
    }

    fn len(&self) -> usize {
        self.steps.len()
    }

    fn params(&self) -> &[Param; DOF] {
        &self.params
    }

    fn steps(&self) -> &[Step] {
        &self.steps
    }
}

#[cfg(feature = "alloc")]
impl<const DOF: usize, const N: usize> From<&LinkageFixed<DOF, N>> for LinkageBuf<DOF> {
    fn from(linkage: &LinkageFixed<DOF, N>) -> Self {
        Self {
            params: linkage.params,
            param_len: linkage.param_len,
            steps: linkage.steps[..linkage.len].to_vec(),
            mark_names: linkage.mark_names[..linkage.mark_len].to_vec(),
        }
    }
}

#[cfg(feature = "alloc")]
impl<'a, const DOF: usize> From<&'a LinkageBuf<DOF>> for LinkageView<'a, DOF> {
    fn from(linkage: &'a LinkageBuf<DOF>) -> Self {
        linkage.view()
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
pub enum PenState {
    Up,
    Down,
}

/// Drawing state carried while evaluating a linkage.
#[derive(Clone, Copy, Debug)]
pub struct PenStyle {
    pen: PenState,
    color: Rgb888,
    width: f32,
}

impl PenStyle {
    /// Return the default down pen with white color and width 0.1.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pen: PenState::Down,
            color: Rgb888::new(255, 255, 255),
            width: 0.1,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    /// Return the current pen state.
    #[must_use]
    pub const fn pen(self) -> PenState {
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
            Step::PenUp => self.pen = PenState::Up,
            Step::PenDown => self.pen = PenState::Down,
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

    /// Return this styled pose's pen state.
    #[must_use]
    pub const fn pen(self) -> PenState {
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

/// Iterator over styled poses from a LinkageView (does not require const N).
struct StyledPosesView<'a, const DOF: usize> {
    steps: &'a [Step],
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
    marked: [MarkedState; 32],
    marked_len: usize,
}

impl<'a, const DOF: usize> StyledPosesView<'a, DOF> {
    fn new(steps: &'a [Step], params_values: &'a [f32; DOF]) -> Self {
        validate_params(params_values);
        Self {
            steps,
            params: params_values,
            index: 0,
            pose: Pose::start(),
            pen_style: PenStyle::new(),
            marked: [MarkedState {
                pose: Pose::start(),
                pen_style: PenStyle::new(),
            }; 32],
            marked_len: 0,
        }
    }
}

impl<const DOF: usize> Iterator for StyledPosesView<'_, DOF> {
    type Item = StyledPose;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index >= self.steps.len() {
                return None;
            }
            let step = &self.steps[self.index];
            self.index += 1;

            match step {
                Step::Mark { name: _ } => {
                    assert!(self.marked_len < 32, "too many marked states");
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

/// Iterator over draw items from a LinkageView (does not require const N).
struct DrawItemsView<'a, const DOF: usize> {
    steps: &'a [Step],
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
    marked: [MarkedState; 32],
    marked_len: usize,
}

impl<'a, const DOF: usize> DrawItemsView<'a, DOF> {
    fn new(steps: &'a [Step], params_values: &'a [f32; DOF]) -> Self {
        validate_params(params_values);
        Self {
            steps,
            params: params_values,
            index: 0,
            pose: Pose::start(),
            pen_style: PenStyle::new(),
            marked: [MarkedState {
                pose: Pose::start(),
                pen_style: PenStyle::new(),
            }; 32],
            marked_len: 0,
        }
    }
}

impl<const DOF: usize> Iterator for DrawItemsView<'_, DOF> {
    type Item = DrawItem;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.steps.len() {
            let step = &self.steps[self.index];
            self.index += 1;

            match step {
                Step::Mark { name: _ } => {
                    assert!(self.marked_len < 32, "too many marked states");
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
                    if matches!(pen_style.pen(), PenState::Down) =>
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
                _ => continue,
            }
        }

        None
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

// ── .lb.rs include macros ────────────────────────────────────────────────────
//
// A `.lb.rs` file is a complete Rust expression.
// It contains one `linkage![ ... ]` invocation.
// The body is a fluent DSL chain of leading-dot method calls.
// The including macro (`linkage_fixed!` or `linkage_buf!`) defines the local
// `__linkage_blaze_start!` macro that selects the storage type.
// The file must not call `start!()` and must not define `macro_rules! linkage`.

/// Callback macro used inside `.lb.rs` linkage files.
///
/// A `.lb.rs` file is a complete Rust expression that contains exactly one
/// `linkage![ ... ]` invocation.  The body is a fluent DSL chain of
/// leading-dot method calls (e.g. `.define_param(...)`, `.forward(...)`,
/// `.mark(...)`).
///
/// `linkage!` is a *callback* macro: it does not know the storage type on its
/// own.  The including macro ([`linkage_fixed!`] or [`linkage_buf!`]) defines
/// the local helper `__linkage_blaze_start!` immediately before the
/// `include!`, which `linkage!` calls to obtain the correctly-typed builder.
///
/// ## `.lb.rs` convention
///
/// - The file contains one `linkage![ ... ]` invocation and nothing else.
/// - The body begins with leading-dot methods (no explicit `start()` call).
/// - The file must **not** call `start!()`.
/// - The file must **not** define `macro_rules! linkage`.
/// - Use [`linkage_fixed!`] or [`linkage_buf!`] to include the file.
///
/// ## Example `.lb.rs` file
///
/// ```rust,ignore
/// linkage![
///     .define_param("hour", 0.0)
///     .define_param("face spin", 0.5)
///     .roll_param("face spin", -90.0, 90.0)
///     .mark("face")
///     .pen_color(Rgb888::new(33, 79, 155))
///     .disk(66.0)
/// ]
/// ```
#[macro_export]
macro_rules! linkage {
    ($($chain:tt)*) => {
        (__linkage_blaze_start!()) $($chain)*
    };
}

/// Include a `.lb.rs` linkage file as a [`LinkageFixed`] expression.
///
/// The path is relative to the source file containing the macro invocation
/// (same rules as `include!`).  The file must contain exactly one
/// `linkage![ ... ]` invocation using the fluent DSL — see [`linkage!`] for
/// the full `.lb.rs` convention.
///
/// ## Forms
///
/// Prefer the one-argument form with an explicit type annotation:
///
/// ```rust,ignore
/// const CLOCK: LinkageFixed<2, 48> =
///     linkage_fixed!("clock.lb.rs");
///
/// // Inside a function body:
/// let clock: LinkageFixed<2, 48> =
///     linkage_fixed!("clock.lb.rs");
/// ```
///
/// Use the explicit-number form only when the surrounding type cannot be
/// inferred:
///
/// ```rust,ignore
/// const CLOCK: LinkageFixed<2, 48> =
///     linkage_fixed!("clock.lb.rs", 2, 48);
/// ```
#[macro_export]
macro_rules! linkage_fixed {
    ($path:literal) => {{
        macro_rules! __linkage_blaze_start {
            () => { $crate::LinkageFixed::start() }
        }
        include!($path)
    }};
    ($path:literal, $dof:expr, $n:expr) => {{
        let linkage: $crate::LinkageFixed<$dof, $n> = $crate::linkage_fixed!($path);
        linkage
    }};
}

/// Include a `.lb.rs` linkage file as a [`LinkageBuf`] expression.
///
/// Requires the `alloc` feature.  The path follows the same rules as
/// `include!`.  The file must contain exactly one `linkage![ ... ]`
/// invocation — see [`linkage!`] for the full `.lb.rs` convention.
///
/// ## Forms
///
/// Prefer the one-argument form with an explicit type annotation:
///
/// ```rust,ignore
/// let clock: LinkageBuf<2> =
///     linkage_buf!("clock.lb.rs");
/// ```
///
/// Use the explicit-number form only when the surrounding type cannot be
/// inferred:
///
/// ```rust,ignore
/// let clock = linkage_buf!("clock.lb.rs", 2);
/// ```
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! linkage_buf {
    ($path:literal) => {{
        macro_rules! __linkage_blaze_start {
            () => { $crate::LinkageBuf::start() }
        }
        include!($path)
    }};
    ($path:literal, $dof:expr) => {{
        let linkage: $crate::LinkageBuf<$dof> = $crate::linkage_buf!($path);
        linkage
    }};
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
    use std::{boxed::Box, error::Error};

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
            .view()
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
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([10.0, 0.0, 0.0]), 1e-6));
    }

    #[test]
    fn yaw_then_forward_moves_along_positive_y() {
        const LINKAGE: LinkageFixed<0, 3> = LinkageFixed::start().yaw(90.0).forward(10.0);

        let params = [];
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 10.0, 0.0]), 1e-5));
    }

    #[test]
    fn left_moves_along_positive_y() {
        const LINKAGE: LinkageFixed<0, 2> = LinkageFixed::start().left(10.0);

        let params = [];
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 10.0, 0.0]), 1e-6));
    }

    #[test]
    fn up_moves_along_positive_z() {
        const LINKAGE: LinkageFixed<0, 2> = LinkageFixed::start().up(10.0);

        let params = [];
        let actual = LINKAGE.view().final_pose(&params).position();

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
        let actual = LINKAGE.view().final_pose(&params).position();

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
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([10.0, 5.0, 0.0]), 1e-5));
    }

    #[test]
    fn test_excel_pose_trace0_matches_expected() -> Result<(), Box<dyn Error>> {
        // Fractions for [raise hand, bend elbow, close hand,
        //  lower arm, spin whole arm, spin hand].
        let params = [0.7514501463, 0.5002003842, 0.5, 1.0, 0.6254387123, 0.0];
        assert_pose_trace_matches_expected("excel_pose_trace0.csv", LINKAGE0.view().poses(&params))
    }

    #[test]
    fn test_excel_pose_trace1_matches_expected() -> Result<(), Box<dyn Error>> {
        // [spin whole arm, bend elbow, close hand]
        let params = [0.30, 0.02, 0.10];
        assert_pose_trace_matches_expected("excel_pose_trace1.csv", LINKAGE1.view().poses(&params))
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
        let pose = LINKAGE0.view().final_pose(&params);
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
        let pose = LINKAGE1.view().final_pose(&params);
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
        let pose = LINKAGE0.view().final_pose(&params);
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

        let _ = LINKAGE0.view().final_pose(&params);
    }

    // ── Shadowing semantics ───────────────────────────────────────────────────
    //
    // A param name may appear more than once in a linkage.  DSL methods like
    // `yaw_param` bind to the *most recently defined* param with that name —
    // this is "shadowing".  The earlier definition is not removed; it still
    // occupies its slot in the param array.

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
        let pos = LINKAGE.view().final_pose(&params).position();
        // yaw driven by index 1 = 0.0 → 0° → moves along +X
        assert!(pos.is_close_to(&Vec3::from([10.0, 0.0, 0.0]), 1e-5));
    }

}
