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

#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::format;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub use math::{Mat3, Vec3};

pub use embedded_graphics::pixelcolor::{Rgb888, WebColors};
pub use embedded_graphics::prelude::RgbColor;
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
    /// Save the current pose and pen state into a resolved mark slot.
    Mark { index: usize },
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

    /// Return the degree-of-freedom parameter index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// Return the low end of the parameter range.
    #[must_use]
    pub const fn low(self) -> f32 {
        self.low
    }

    /// Return the high end of the parameter range.
    #[must_use]
    pub const fn high(self) -> f32 {
        self.low + self.span
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
/// const LINKAGE: LinkageFixed<1, 0, 8> = LinkageFixed::start()
///     .define_param("distance", 0.5)
///     .forward_param("distance", 1.0, 5.0);
///
/// // Get a view and evaluate
/// let pose = LINKAGE.view().final_pose(&[0.5]);
/// ```
pub trait Linkage<const DOF: usize, const MARKS: usize> {
    /// Create a borrowed view for evaluation and rendering.
    ///
    /// All evaluation methods (final_pose, poses, draw_items, etc.) are available on the returned
    /// [`LinkageView`]. This keeps the conceptual model clean: storage types (LinkageFixed, LinkageBuf)
    /// define expressions; views evaluate them.
    fn view(&self) -> LinkageView<'_, DOF, MARKS>;

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
/// const LINKAGE: LinkageFixed<1, 0, 8> = LinkageFixed::start()
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
pub struct LinkageView<'a, const DOF: usize, const MARKS: usize> {
    params: &'a [Param; DOF],
    steps: &'a [Step],
    mark_names: &'a [&'static str; MARKS],
    mark_len: usize,
}

impl<'a, const DOF: usize, const MARKS: usize> LinkageView<'a, DOF, MARKS> {
    /// The number of runtime parameters (degrees of freedom).
    pub const DOF: usize = DOF;

    /// Create a new linkage view from parameter and step arrays.
    #[must_use]
    pub(crate) const fn new(
        params: &'a [Param; DOF],
        steps: &'a [Step],
        mark_names: &'a [&'static str; MARKS],
        mark_len: usize,
    ) -> Self {
        Self {
            params,
            steps,
            mark_names,
            mark_len,
        }
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
    /// const LINKAGE: LinkageFixed<2, 0, 8> = LinkageFixed::start()
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
    /// const LINKAGE: LinkageFixed<2, 0, 8> = LinkageFixed::start()
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

    /// Return the active mark-name slots.
    #[must_use]
    pub const fn mark_names(&self) -> &'a [&'static str; MARKS] {
        self.mark_names
    }

    /// Return the number of distinct mark slots used by this linkage.
    #[must_use]
    pub const fn mark_len(&self) -> usize {
        self.mark_len
    }

    /// Serialize this linkage view as `.lb.rs` source using the `linkage![ ... ]` format.
    ///
    /// The output is intended for editor interchange and generated linkage files.
    /// Color values are emitted as `Rgb888::new(r, g, b)` calls.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn to_lb_rs(&self) -> String {
        let named_params = self.params.iter().filter(|p| !p.name().is_empty()).count();
        let mut source = format!(
            "// DOF={} MARKS={} STEPS={}\n",
            named_params,
            self.mark_len,
            self.len()
        );
        source.push_str("linkage![\n");
        for param in self.params {
            if !param.name().is_empty() {
                source.push_str("    .define_param(\"");
                source.push_str(param.name());
                source.push_str("\", ");
                push_f32(&mut source, param.default());
                source.push_str(")\n");
            }
        }

        for &step in self.steps {
            match step {
                Step::Start => {}
                Step::Yaw(arg) => push_arg_step(self, &mut source, "yaw", "yaw_param", arg, true),
                Step::Pitch(arg) => {
                    push_arg_step(self, &mut source, "pitch", "pitch_param", arg, true);
                }
                Step::Roll(arg) => {
                    push_arg_step(self, &mut source, "roll", "roll_param", arg, true);
                }
                Step::Move(arg) => {
                    push_arg_step(self, &mut source, "forward", "forward_param", arg, false);
                }
                Step::Left(arg) => {
                    push_arg_step(self, &mut source, "left", "left_param", arg, false);
                }
                Step::Up(arg) => push_arg_step(self, &mut source, "up", "up_param", arg, false),
                Step::PenUp => source.push_str("    .pen_up()\n"),
                Step::PenDown => source.push_str("    .pen_down()\n"),
                Step::PenColor(color) => {
                    source.push_str("    .pen_color(Rgb888::new(");
                    source.push_str(&format!("{}, {}, {}", color.r(), color.g(), color.b()));
                    source.push_str("))\n");
                }
                Step::PenWidth(width) => {
                    source.push_str("    .pen_width(");
                    push_f32(&mut source, width);
                    source.push_str(")\n");
                }
                Step::Disk(radius) => push_fixed_step(&mut source, "disk", radius),
                Step::DiskParam(arg) => push_variable_step(self, &mut source, "disk_param", arg),
                Step::Ring(radius) => push_fixed_step(&mut source, "ring", radius),
                Step::RingParam(arg) => push_variable_step(self, &mut source, "ring_param", arg),
                Step::Sphere(radius) => push_fixed_step(&mut source, "sphere", radius),
                Step::SphereParam(arg) => {
                    push_variable_step(self, &mut source, "sphere_param", arg);
                }
                Step::Mark { index } => {
                    let name = self.mark_names[index];
                    source.push_str("    .mark(\"");
                    source.push_str(name);
                    source.push_str("\")\n");
                }
                Step::Restore { index } => {
                    let name = self.mark_names[index];
                    source.push_str("    .restore(\"");
                    source.push_str(name);
                    source.push_str("\")\n");
                }
            }
        }
        source.push_str("]\n");
        source
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
    /// const LINKAGE: LinkageFixed<3, 0, 8> = LinkageFixed::start()
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
    /// const LINKAGE: LinkageFixed<2, 0, 8> = LinkageFixed::start()
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
    /// const LINKAGE: LinkageFixed<1, 0, 8> = LinkageFixed::start()
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
    /// const LINKAGE: LinkageFixed<0, 0, 8> = LinkageFixed::start()
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
        StyledPosesView::<DOF, MARKS>::new(self.steps, params)
    }

    /// Iterate over all draw items produced by this linkage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use linkage_blaze_core::{LinkageFixed, DrawItem};
    /// const LINKAGE: LinkageFixed<0, 0, 8> = LinkageFixed::start()
    ///     .forward(1.0)
    ///     .forward(2.0);
    ///
    /// let view = LINKAGE.view();
    /// let has_stroke = view.draw_items(&[])
    ///     .any(|item| matches!(item, DrawItem::Stroke(_)));
    /// assert!(has_stroke);
    /// ```
    pub fn draw_items<'b>(&'b self, params: &'b [f32; DOF]) -> DrawItemIter<'b, DOF, MARKS> {
        DrawItemIter::<DOF, MARKS>::new(self.steps, self.mark_names, params)
    }
}

impl<'a, const DOF: usize, const MARKS: usize, const N: usize> From<&'a LinkageFixed<DOF, MARKS, N>>
    for LinkageView<'a, DOF, MARKS>
{
    fn from(linkage: &'a LinkageFixed<DOF, MARKS, N>) -> Self {
        linkage.view()
    }
}

#[cfg(feature = "alloc")]
fn push_arg_step<const DOF: usize, const MARKS: usize>(
    linkage_view: &LinkageView<'_, DOF, MARKS>,
    source: &mut String,
    fixed_method: &str,
    variable_method: &str,
    arg: Arg,
    degrees: bool,
) {
    match arg {
        Arg::Fixed(value) => {
            let value = if degrees { value.to_degrees() } else { value };
            push_fixed_step(source, fixed_method, value);
        }
        Arg::Variable(variable_arg) => {
            let low = if degrees {
                variable_arg.low().to_degrees()
            } else {
                variable_arg.low()
            };
            let high = if degrees {
                variable_arg.high().to_degrees()
            } else {
                variable_arg.high()
            };
            source.push_str("    .");
            source.push_str(variable_method);
            source.push_str("(\"");
            source.push_str(linkage_view.param(variable_arg.index()).name());
            source.push_str("\", ");
            push_f32(source, low);
            source.push_str(", ");
            push_f32(source, high);
            source.push_str(")\n");
        }
    }
}

#[cfg(feature = "alloc")]
fn push_fixed_step(source: &mut String, method: &str, value: f32) {
    source.push_str("    .");
    source.push_str(method);
    source.push('(');
    push_f32(source, value);
    source.push_str(")\n");
}

#[cfg(feature = "alloc")]
fn push_variable_step<const DOF: usize, const MARKS: usize>(
    linkage_view: &LinkageView<'_, DOF, MARKS>,
    source: &mut String,
    method: &str,
    variable_arg: VariableArg,
) {
    source.push_str("    .");
    source.push_str(method);
    source.push_str("(\"");
    source.push_str(linkage_view.param(variable_arg.index()).name());
    source.push_str("\", ");
    push_f32(source, variable_arg.low());
    source.push_str(", ");
    push_f32(source, variable_arg.high());
    source.push_str(")\n");
}

#[cfg(feature = "alloc")]
fn push_f32(source: &mut String, value: f32) {
    source.push_str(&format!("{value:?}"));
}

#[cfg(feature = "alloc")]
fn parse_lb_rs<const DOF: usize, const MARKS: usize>(
    source: &str,
) -> Result<LinkageBuf<DOF, MARKS>, String> {
    let mut linkage = LinkageBuf::start();

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let Some(method_call) = parse_method_call(line_number, line)? else {
            continue;
        };
        linkage = apply_parsed_method(line_number, linkage, &method_call)?;
    }

    Ok(linkage)
}

#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
struct ParsedMethodCall {
    name: String,
    args: Vec<String>,
}

#[cfg(feature = "alloc")]
fn parse_method_call(line_number: usize, line: &str) -> Result<Option<ParsedMethodCall>, String> {
    let line = strip_rust_comment(line).trim();
    if line.is_empty() || line == "LinkageFixed::start()" || is_linkage_macro_wrapper(line) {
        return Ok(None);
    }

    if line.contains("LinkageFixed::start()") {
        return Ok(None);
    }

    let line = line.trim_end_matches(';').trim_end_matches(',').trim();
    let line = line.strip_prefix('.').unwrap_or(line);

    let open = line
        .find('(')
        .ok_or_else(|| format!("line {line_number}: expected `(`"))?;
    let close = line
        .rfind(')')
        .ok_or_else(|| format!("line {line_number}: expected `)`"))?;
    if close < open {
        return Err(format!("line {line_number}: `)` appears before `(`"));
    }
    let trailing = line[close + 1..].trim();
    if !trailing.is_empty() {
        return Err(format!(
            "line {line_number}: unexpected text after method call `{trailing}`"
        ));
    }

    let name = line[..open].trim();
    if name.is_empty() {
        return Err(format!("line {line_number}: method name is empty"));
    }

    Ok(Some(ParsedMethodCall {
        name: name.to_owned(),
        args: split_args(line_number, &line[open + 1..close])?,
    }))
}

#[cfg(feature = "alloc")]
fn is_linkage_macro_wrapper(line: &str) -> bool {
    let line = line.trim_end_matches(';').trim();
    matches!(line, "linkage![" | "linkage! [" | "]")
}

#[cfg(feature = "alloc")]
fn split_args(line_number: usize, args: &str) -> Result<Vec<String>, String> {
    let mut split_args = Vec::new();
    let mut current_arg = String::new();
    let mut parenthesis_depth = 0;
    let mut in_string = false;

    for character in args.chars() {
        match character {
            '"' => {
                in_string = !in_string;
                current_arg.push(character);
            }
            '(' if !in_string => {
                parenthesis_depth += 1;
                current_arg.push(character);
            }
            ')' if !in_string => {
                if parenthesis_depth == 0 {
                    return Err(format!(
                        "line {line_number}: unexpected `)` in argument list"
                    ));
                }
                parenthesis_depth -= 1;
                current_arg.push(character);
            }
            ',' if !in_string && parenthesis_depth == 0 => {
                let trimmed_arg = current_arg.trim();
                if !trimmed_arg.is_empty() {
                    split_args.push(trimmed_arg.to_owned());
                }
                current_arg = String::new();
            }
            _ => current_arg.push(character),
        }
    }

    if in_string {
        return Err(format!("line {line_number}: unterminated string literal"));
    }
    if parenthesis_depth != 0 {
        return Err(format!("line {line_number}: unterminated nested argument"));
    }

    let current_arg = current_arg.trim();
    if !current_arg.is_empty() {
        split_args.push(current_arg.to_owned());
    }

    Ok(split_args)
}

#[cfg(feature = "alloc")]
fn apply_parsed_method<const DOF: usize, const MARKS: usize>(
    line_number: usize,
    linkage: LinkageBuf<DOF, MARKS>,
    method_call: &ParsedMethodCall,
) -> Result<LinkageBuf<DOF, MARKS>, String> {
    match method_call.name.as_str() {
        "define_param" => {
            expect_arg_count(line_number, method_call, 2)?;
            let name = parse_static_string_arg(line_number, method_call, 0)?;
            let default = parse_number_arg(line_number, method_call, 1)?;
            if !(0.0..=1.0).contains(&default) {
                return Err(format!(
                    "line {line_number}: define_param default must be between 0.0 and 1.0"
                ));
            }
            Ok(linkage.define_param(name, default))
        }
        "forward" | "move" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.forward(parse_number_arg(line_number, method_call, 0)?))
        }
        "forward_param" => apply_translation_param(line_number, linkage, method_call, "forward"),
        "left" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.left(parse_number_arg(line_number, method_call, 0)?))
        }
        "left_param" => apply_translation_param(line_number, linkage, method_call, "left"),
        "up" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.up(parse_number_arg(line_number, method_call, 0)?))
        }
        "up_param" => apply_translation_param(line_number, linkage, method_call, "up"),
        "yaw" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.yaw(parse_number_arg(line_number, method_call, 0)?))
        }
        "yaw_param" => apply_rotation_param(line_number, linkage, method_call, "yaw"),
        "pitch" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.pitch(parse_number_arg(line_number, method_call, 0)?))
        }
        "pitch_param" => apply_rotation_param(line_number, linkage, method_call, "pitch"),
        "roll" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.roll(parse_number_arg(line_number, method_call, 0)?))
        }
        "roll_param" => apply_rotation_param(line_number, linkage, method_call, "roll"),
        "pen_up" => {
            expect_arg_count(line_number, method_call, 0)?;
            Ok(linkage.pen_up())
        }
        "pen_down" => {
            expect_arg_count(line_number, method_call, 0)?;
            Ok(linkage.pen_down())
        }
        "pen_color" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.pen_color(parse_color_arg(line_number, method_call)?))
        }
        "pen_width" => {
            expect_arg_count(line_number, method_call, 1)?;
            let width = parse_number_arg(line_number, method_call, 0)?;
            if width < 0.0 {
                return Err(format!(
                    "line {line_number}: pen_width must be non-negative"
                ));
            }
            Ok(linkage.pen_width(width))
        }
        "mark" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.mark(parse_static_string_arg(line_number, method_call, 0)?))
        }
        "restore" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.restore(parse_static_string_arg(line_number, method_call, 0)?))
        }
        "disk" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.disk(parse_radius(line_number, method_call, 0)?))
        }
        "disk_param" => apply_radius_param(line_number, linkage, method_call, "disk"),
        "ring" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.ring(parse_radius(line_number, method_call, 0)?))
        }
        "ring_param" => apply_radius_param(line_number, linkage, method_call, "ring"),
        "sphere" => {
            expect_arg_count(line_number, method_call, 1)?;
            Ok(linkage.sphere(parse_radius(line_number, method_call, 0)?))
        }
        "sphere_param" => apply_radius_param(line_number, linkage, method_call, "sphere"),
        _ => Err(format!(
            "line {line_number}: unknown method `{}`",
            method_call.name
        )),
    }
}

#[cfg(feature = "alloc")]
fn apply_translation_param<const DOF: usize, const MARKS: usize>(
    line_number: usize,
    linkage: LinkageBuf<DOF, MARKS>,
    method_call: &ParsedMethodCall,
    axis: &str,
) -> Result<LinkageBuf<DOF, MARKS>, String> {
    expect_arg_count(line_number, method_call, 3)?;
    let name = parse_string_arg(line_number, method_call, 0)?;
    let low = parse_number_arg(line_number, method_call, 1)?;
    let high = parse_number_arg(line_number, method_call, 2)?;
    match axis {
        "forward" => Ok(linkage.forward_param(name, low, high)),
        "left" => Ok(linkage.left_param(name, low, high)),
        "up" => Ok(linkage.up_param(name, low, high)),
        _ => unreachable!(),
    }
}

#[cfg(feature = "alloc")]
fn apply_rotation_param<const DOF: usize, const MARKS: usize>(
    line_number: usize,
    linkage: LinkageBuf<DOF, MARKS>,
    method_call: &ParsedMethodCall,
    axis: &str,
) -> Result<LinkageBuf<DOF, MARKS>, String> {
    expect_arg_count(line_number, method_call, 3)?;
    let name = parse_string_arg(line_number, method_call, 0)?;
    let low = parse_number_arg(line_number, method_call, 1)?;
    let high = parse_number_arg(line_number, method_call, 2)?;
    match axis {
        "yaw" => Ok(linkage.yaw_param(name, low, high)),
        "pitch" => Ok(linkage.pitch_param(name, low, high)),
        "roll" => Ok(linkage.roll_param(name, low, high)),
        _ => unreachable!(),
    }
}

#[cfg(feature = "alloc")]
fn apply_radius_param<const DOF: usize, const MARKS: usize>(
    line_number: usize,
    linkage: LinkageBuf<DOF, MARKS>,
    method_call: &ParsedMethodCall,
    shape: &str,
) -> Result<LinkageBuf<DOF, MARKS>, String> {
    expect_arg_count(line_number, method_call, 3)?;
    let name = parse_string_arg(line_number, method_call, 0)?;
    let low = parse_number_arg(line_number, method_call, 1)?;
    let high = parse_number_arg(line_number, method_call, 2)?;
    if low < 0.0 || high < 0.0 {
        return Err(format!(
            "line {line_number}: `{}` radius range must be non-negative",
            method_call.name
        ));
    }
    match shape {
        "disk" => Ok(linkage.disk_param(name, low, high)),
        "ring" => Ok(linkage.ring_param(name, low, high)),
        "sphere" => Ok(linkage.sphere_param(name, low, high)),
        _ => unreachable!(),
    }
}

#[cfg(feature = "alloc")]
fn strip_rust_comment(line: &str) -> &str {
    line.split_once("//")
        .map_or(line, |(before_comment, _)| before_comment)
}

#[cfg(feature = "alloc")]
fn expect_arg_count(
    line_number: usize,
    method_call: &ParsedMethodCall,
    expected: usize,
) -> Result<(), String> {
    if method_call.args.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "line {line_number}: `{}` expects {expected} argument(s), got {}",
            method_call.name,
            method_call.args.len()
        ))
    }
}

#[cfg(feature = "alloc")]
fn parse_number_arg(
    line_number: usize,
    method_call: &ParsedMethodCall,
    arg_index: usize,
) -> Result<f32, String> {
    let value = &method_call.args[arg_index];
    parse_number_or_constant(line_number, method_call.name.as_str(), value)
}

#[cfg(feature = "alloc")]
fn parse_static_string_arg(
    line_number: usize,
    method_call: &ParsedMethodCall,
    arg_index: usize,
) -> Result<&'static str, String> {
    let value = parse_string_arg(line_number, method_call, arg_index)?;
    Ok(Box::leak(value.to_owned().into_boxed_str()))
}

#[cfg(feature = "alloc")]
fn parse_string_arg(
    line_number: usize,
    method_call: &ParsedMethodCall,
    arg_index: usize,
) -> Result<&str, String> {
    let value = method_call.args[arg_index].as_str();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| {
            format!(
                "line {line_number}: `{}` argument `{value}` must be a string literal",
                method_call.name
            )
        })
}

#[cfg(feature = "alloc")]
fn parse_number_or_constant(
    line_number: usize,
    method_name: &str,
    value: &str,
) -> Result<f32, String> {
    let numeric_value: String = value
        .chars()
        .filter(|character| *character != '_')
        .collect();

    if let Ok(parsed) = numeric_value.parse::<f32>() {
        if looks_like_integer(&numeric_value) {
            return Err(format!(
                "line {line_number}: `{method_name}` argument `{value}` is an integer; use `{value}.0`"
            ));
        }
        return Ok(parsed);
    }

    number_constant(value).ok_or_else(|| {
        format!(
            "line {line_number}: `{method_name}` argument `{value}` is not a number or known constant"
        )
    })
}

#[cfg(feature = "alloc")]
fn looks_like_integer(s: &str) -> bool {
    let digits = s
        .strip_prefix('-')
        .or_else(|| s.strip_prefix('+'))
        .unwrap_or(s);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

#[cfg(feature = "alloc")]
fn parse_radius(
    line_number: usize,
    method_call: &ParsedMethodCall,
    arg_index: usize,
) -> Result<f32, String> {
    let radius = parse_number_arg(line_number, method_call, arg_index)?;
    if radius < 0.0 {
        Err(format!(
            "line {line_number}: `{}` radius must be non-negative",
            method_call.name
        ))
    } else {
        Ok(radius)
    }
}

#[cfg(feature = "alloc")]
fn number_constant(name: &str) -> Option<f32> {
    match name {
        "ARM_WIDTH" => Some(3.0),
        "AXIS_WIDTH" => Some(1.0),
        _ => None,
    }
}

#[cfg(feature = "alloc")]
fn parse_color_arg(line_number: usize, method_call: &ParsedMethodCall) -> Result<Rgb888, String> {
    let value = method_call.args[0].as_str();

    if let Some(color_args) = value
        .strip_prefix("Rgb888::new(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_rgb888_new_color(line_number, color_args);
    }

    match value {
        "Rgb888::CSS_BLACK" => Ok(Rgb888::CSS_BLACK),
        "Rgb888::CSS_BLUE" => Ok(Rgb888::CSS_BLUE),
        "Rgb888::CSS_CRIMSON" => Ok(Rgb888::CSS_CRIMSON),
        "Rgb888::CSS_DARK_SLATE_GRAY" => Ok(Rgb888::CSS_DARK_SLATE_GRAY),
        "Rgb888::CSS_DIM_GRAY" => Ok(Rgb888::CSS_DIM_GRAY),
        "Rgb888::CSS_GRAY" => Ok(Rgb888::CSS_GRAY),
        "Rgb888::CSS_LIGHT_GRAY" => Ok(Rgb888::CSS_LIGHT_GRAY),
        "Rgb888::CSS_LIGHT_SLATE_GRAY" => Ok(Rgb888::CSS_LIGHT_SLATE_GRAY),
        "Rgb888::CSS_RED" => Ok(Rgb888::CSS_RED),
        "Rgb888::CSS_SLATE_GRAY" => Ok(Rgb888::CSS_SLATE_GRAY),
        "Rgb888::CSS_STEEL_BLUE" => Ok(Rgb888::CSS_STEEL_BLUE),
        "Rgb888::CSS_WHITE" => Ok(Rgb888::CSS_WHITE),
        _ => Err(format!(
            "line {line_number}: unknown color `{value}`; use `Rgb888::CSS_*` or `Rgb888::new(r, g, b)`"
        )),
    }
}

#[cfg(feature = "alloc")]
fn parse_rgb888_new_color(line_number: usize, args: &str) -> Result<Rgb888, String> {
    let args = split_args(line_number, args)?;
    if args.len() != 3 {
        return Err(format!(
            "line {line_number}: `Rgb888::new` expects 3 argument(s), got {}",
            args.len()
        ));
    }

    Ok(Rgb888::new(
        parse_u8_arg(line_number, "Rgb888::new", &args[0])?,
        parse_u8_arg(line_number, "Rgb888::new", &args[1])?,
        parse_u8_arg(line_number, "Rgb888::new", &args[2])?,
    ))
}

#[cfg(feature = "alloc")]
fn parse_u8_arg(line_number: usize, method_name: &str, value: &str) -> Result<u8, String> {
    let numeric_value: String = value
        .chars()
        .filter(|character| *character != '_')
        .collect();
    numeric_value
        .parse::<u8>()
        .map_err(|_| format!("line {line_number}: `{method_name}` argument `{value}` is not a u8"))
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
            self.push(Step::Yaw(Arg::Variable(VariableArg::from_degrees(
                index, low, high,
            ))))
        }
        pub const fn pitch_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Pitch(Arg::Variable(VariableArg::from_degrees(
                index, low, high,
            ))))
        }
        pub const fn roll_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Roll(Arg::Variable(VariableArg::from_degrees(
                index, low, high,
            ))))
        }
        pub const fn forward_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Move(Arg::Variable(VariableArg::new(
                index, low, high,
            ))))
        }
        pub const fn left_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push(Step::Left(Arg::Variable(VariableArg::new(
                index, low, high,
            ))))
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
            let index = match self.mark_index(name) {
                Some(i) => i,
                None => {
                    panic!("restore: no mark found with name (mark must be defined before restore)")
                }
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
            self.push_step(Step::Yaw(Arg::Variable(VariableArg::from_degrees(
                index, low, high,
            ))))
        }
        pub fn pitch_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Pitch(Arg::Variable(VariableArg::from_degrees(
                index, low, high,
            ))))
        }
        pub fn roll_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Roll(Arg::Variable(VariableArg::from_degrees(
                index, low, high,
            ))))
        }
        pub fn forward_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Move(Arg::Variable(VariableArg::new(
                index, low, high,
            ))))
        }
        pub fn left_param(self, name: &str, low: f32, high: f32) -> Self {
            let index = self.expect_param_index(name);
            self.push_step(Step::Left(Arg::Variable(VariableArg::new(
                index, low, high,
            ))))
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
///
/// Parameter indexes are identities. Parameter names are labels/selectors and may
/// be duplicated. Use `freeze_param_index` and `retain_param_indexes` for precise
/// specialization. Name-based freezing requires exactly one matching parameter,
/// while name-based retaining keeps all slots matching each requested name. Freeze
/// raw values are operation values, not normalized slider values: rotations use
/// degrees, and distances/radii use linkage units.
pub struct LinkageFixed<const DOF: usize, const MARKS: usize, const N: usize> {
    steps: [Step; N],
    len: usize,
    params: [Param; DOF],
    param_len: usize,
    mark_names: [&'static str; MARKS],
    mark_len: usize,
}

impl<const DOF: usize, const MARKS: usize, const N: usize> LinkageFixed<DOF, MARKS, N> {
    /// Start a fixed-size linkage with an implicit origin row.
    pub const fn start() -> Self {
        assert!(N > 0, "linkage must have room for the implicit start step");
        Self {
            steps: [const { Step::Start }; N],
            len: 1,
            params: [Param::EMPTY; DOF],
            param_len: 0,
            mark_names: [""; MARKS],
            mark_len: 0,
        }
    }

    /// Number of runtime parameters this linkage expects.
    pub const DOF: usize = DOF;

    /// Step-slot capacity of this linkage.
    pub const N: usize = N;

    /// Mark-slot capacity of this linkage.
    pub const MARKS: usize = MARKS;

    /// Create a borrowed view for evaluation and rendering.
    ///
    /// The view erases the step capacity `N` while preserving the degree-of-freedom `DOF`.
    /// All evaluation methods (poses, draw_items, etc.) operate on the view.
    #[must_use]
    #[inline]
    pub fn view(&self) -> LinkageView<'_, DOF, MARKS> {
        LinkageView::new(
            &self.params,
            &self.steps[..self.len],
            &self.mark_names,
            self.mark_len,
        )
    }

    /// Return the number of runtime parameters this linkage expects.
    #[must_use]
    pub const fn dof(&self) -> usize {
        DOF
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
        let index = match self.mark_index(name) {
            Some(index) => index,
            None => {
                assert!(self.mark_len < MARKS, "linkage has more marks than MARKS");
                let index = self.mark_len;
                self.mark_names[index] = name;
                self.mark_len += 1;
                index
            }
        };
        self.push(Step::Mark { index })
    }

    const fn mark_index(&self, name: &str) -> Option<usize> {
        let mut mark_index = 0;
        while mark_index < self.mark_len {
            if str_eq(self.mark_names[mark_index], name) {
                return Some(mark_index);
            }
            mark_index += 1;
        }
        None
    }

    /// Return a new linkage with a sphere at the start and end of every move step
    /// (Move, Left, Up — both fixed and parametric).
    ///
    /// Spheres at adjacent move endpoints overlap and render twice, which is fine.
    /// `N_OUT` must be ≥ `self.len` + (number of move steps × 2).
    pub const fn with_joint_spheres<const N_OUT: usize>(
        self,
        joint_radius: f32,
    ) -> LinkageFixed<DOF, MARKS, N_OUT> {
        let mut out = LinkageFixed {
            steps: [const { Step::Start }; N_OUT],
            len: 0,
            params: [Param::EMPTY; DOF],
            param_len: self.param_len,
            mark_names: [""; MARKS],
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
                assert!(out.len < N_OUT, "N_OUT too small for with_joint_spheres");
                out.steps[out.len] = Step::Sphere(joint_radius);
                out.len += 1;
            }
            assert!(out.len < N_OUT, "N_OUT too small for with_joint_spheres");
            out.steps[out.len] = step;
            out.len += 1;
            if is_move {
                assert!(out.len < N_OUT, "N_OUT too small for with_joint_spheres");
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

    /// Freeze exactly one parameter slot by index at a raw operation value.
    ///
    /// Parameter indexes are identities. Parameter names are labels and may be
    /// duplicated. `raw_value` is the fixed operation value, not a normalized
    /// slider value: rotations use degrees, while translations, radii, and widths
    /// use linkage units. The raw value must be inside every referenced step range
    /// for this slot; out-of-range values panic, including during const evaluation.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<2, 0, 6> = LinkageFixed::start()
    ///     .define_param("angle", 0.5)
    ///     .define_param("distance", 0.5)
    ///     .yaw_param("angle", -180.0, 180.0)
    ///     .forward_param("distance", 0.0, 10.0);
    ///
    /// const FROZEN: LinkageFixed<1, 0, 6> = LINKAGE.freeze_param_index(0, 90.0);
    /// ```
    pub const fn freeze_param_index<const OUT_DOF: usize>(
        self,
        param_index: usize,
        raw_value: f32,
    ) -> LinkageFixed<OUT_DOF, MARKS, N> {
        let mut is_frozen = [false; DOF];
        let frozen_at_default = [false; DOF];
        let mut frozen_raw = [0.0f32; DOF];

        assert!(
            param_index < self.param_len,
            "freeze param index out of bounds"
        );
        is_frozen[param_index] = true;
        frozen_raw[param_index] = raw_value;

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    /// Freeze the uniquely named parameter slot at a raw operation value.
    ///
    /// This is a convenience selector over [`freeze_param_index`](Self::freeze_param_index).
    /// It panics if no parameter has `name`, or if more than one parameter has
    /// that name. Use index-based freezing when names are duplicated.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<2, 0, 6> = LinkageFixed::start()
    ///     .define_param("angle", 0.5)
    ///     .define_param("distance", 0.5)
    ///     .yaw_param("angle", -180.0, 180.0)
    ///     .forward_param("distance", 0.0, 10.0);
    ///
    /// const FROZEN: LinkageFixed<1, 0, 6> = LINKAGE.freeze_param_name("angle", 90.0);
    /// ```
    pub const fn freeze_param_name<const OUT_DOF: usize>(
        self,
        name: &'static str,
        raw_value: f32,
    ) -> LinkageFixed<OUT_DOF, MARKS, N> {
        let mut found_index = 0usize;
        let mut found_count = 0usize;
        let mut param_index = 0;
        while param_index < self.param_len {
            if str_eq(self.params[param_index].name, name) {
                found_index = param_index;
                found_count += 1;
            }
            param_index += 1;
        }
        assert!(found_count > 0, "freeze name not found in params");
        assert!(found_count == 1, "freeze name is ambiguous");

        self.freeze_param_index(found_index, raw_value)
    }

    /// Freeze exactly one parameter slot by index at its normalized default.
    ///
    /// This is useful for specialization from declared defaults. Unlike
    /// [`freeze_param_index`](Self::freeze_param_index), one default-normalized
    /// parameter can legally feed steps with different raw ranges.
    pub const fn freeze_param_index_at_default<const OUT_DOF: usize>(
        self,
        param_index: usize,
    ) -> LinkageFixed<OUT_DOF, MARKS, N> {
        let mut is_frozen = [false; DOF];
        let mut frozen_at_default = [false; DOF];
        let frozen_raw = [0.0f32; DOF];

        assert!(
            param_index < self.param_len,
            "freeze param index out of bounds"
        );
        is_frozen[param_index] = true;
        frozen_at_default[param_index] = true;

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    /// Freeze the uniquely named parameter slot at its normalized default.
    ///
    /// Panics if no parameter has `name`, or if more than one parameter has that
    /// name.
    pub const fn freeze_param_name_at_default<const OUT_DOF: usize>(
        self,
        name: &'static str,
    ) -> LinkageFixed<OUT_DOF, MARKS, N> {
        let mut found_index = 0usize;
        let mut found_count = 0usize;
        let mut param_index = 0;
        while param_index < self.param_len {
            if str_eq(self.params[param_index].name, name) {
                found_index = param_index;
                found_count += 1;
            }
            param_index += 1;
        }
        assert!(found_count > 0, "freeze name not found in params");
        assert!(found_count == 1, "freeze name is ambiguous");

        self.freeze_param_index_at_default(found_index)
    }

    /// Retain exactly the listed parameter slots and freeze all others at their defaults.
    ///
    /// Retained slots are reindexed densely in original parameter-slot order, not
    /// caller-list order. Duplicate indexes and out-of-bounds indexes panic.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use linkage_blaze_core::LinkageFixed;
    /// const LINKAGE: LinkageFixed<3, 0, 8> = LinkageFixed::start()
    ///     .define_param("angle", 0.5)
    ///     .define_param("pitch", 0.5)
    ///     .define_param("distance", 0.5)
    ///     .yaw_param("angle", -180.0, 180.0)
    ///     .pitch_param("pitch", -90.0, 90.0)
    ///     .forward_param("distance", 0.0, 10.0);
    ///
    /// const RETAINED: LinkageFixed<1, 0, 8> = LINKAGE.retain_param_indexes(&[2]);
    /// ```
    pub const fn retain_param_indexes<const OUT_DOF: usize>(
        self,
        indexes: &[usize],
    ) -> LinkageFixed<OUT_DOF, MARKS, N> {
        let mut index_index = 0;
        while index_index < indexes.len() {
            let retain_index = indexes[index_index];
            assert!(
                retain_index < self.param_len,
                "retain param index out of bounds"
            );
            let mut previous_index = 0;
            while previous_index < index_index {
                assert!(
                    indexes[previous_index] != retain_index,
                    "duplicate index in retain list"
                );
                previous_index += 1;
            }
            index_index += 1;
        }

        let mut is_frozen = [false; DOF];
        let mut frozen_at_default = [false; DOF];
        let frozen_raw = [0.0f32; DOF];

        let mut param_index = 0;
        while param_index < self.param_len {
            let mut found = false;
            let mut index_index = 0;
            while index_index < indexes.len() {
                if param_index == indexes[index_index] {
                    found = true;
                    break;
                }
                index_index += 1;
            }
            if !found {
                is_frozen[param_index] = true;
                frozen_at_default[param_index] = true;
            }
            param_index += 1;
        }

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    /// Retain parameters selected by name and freeze all others at their defaults.
    ///
    /// Each requested name must exist. If a requested name matches multiple
    /// parameter slots, all matching slots are retained. Duplicate names in the
    /// requested name list panic.
    pub const fn retain_param_names<const OUT_DOF: usize>(
        self,
        names: &[&'static str],
    ) -> LinkageFixed<OUT_DOF, MARKS, N> {
        let mut name_index = 0;
        while name_index < names.len() {
            let retain_name = names[name_index];
            let mut previous_name_index = 0;
            while previous_name_index < name_index {
                assert!(
                    !str_eq(names[previous_name_index], retain_name),
                    "duplicate name in retain list"
                );
                previous_name_index += 1;
            }

            let mut found = false;
            let mut param_index = 0;
            while param_index < self.param_len {
                if str_eq(self.params[param_index].name, retain_name) {
                    found = true;
                    break;
                }
                param_index += 1;
            }
            assert!(found, "retain name not found in params");
            name_index += 1;
        }

        let mut is_frozen = [false; DOF];
        let mut frozen_at_default = [false; DOF];
        let frozen_raw = [0.0f32; DOF];

        let mut param_index = 0;
        while param_index < self.param_len {
            let mut found = false;
            let mut name_index = 0;
            while name_index < names.len() {
                if str_eq(self.params[param_index].name, names[name_index]) {
                    found = true;
                    break;
                }
                name_index += 1;
            }
            if !found {
                is_frozen[param_index] = true;
                frozen_at_default[param_index] = true;
            }
            param_index += 1;
        }

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    const fn freeze_with_map<const OUT_DOF: usize>(
        self,
        is_frozen: [bool; DOF],
        frozen_at_default: [bool; DOF],
        frozen_raw: [f32; DOF],
    ) -> LinkageFixed<OUT_DOF, MARKS, N> {
        let mut new_param_index = [0usize; DOF];
        let mut new_param_len = 0usize;

        let mut param_index = 0;
        while param_index < self.param_len {
            if !is_frozen[param_index] {
                new_param_index[param_index] = new_param_len;
                new_param_len += 1;
            }
            param_index += 1;
        }

        assert!(
            new_param_len == OUT_DOF,
            "OUT_DOF must equal DOF minus the number of frozen parameters"
        );

        let mut out = LinkageFixed {
            steps: [const { Step::Start }; N],
            len: 0,
            params: [Param::EMPTY; OUT_DOF],
            param_len: 0,
            mark_names: [""; MARKS],
            mark_len: self.mark_len,
        };

        let mut mark_index = 0;
        while mark_index < self.mark_len {
            out.mark_names[mark_index] = self.mark_names[mark_index];
            mark_index += 1;
        }

        let mut param_index = 0;
        while param_index < self.param_len {
            if !is_frozen[param_index] {
                out.params[new_param_index[param_index]] = self.params[param_index];
                out.param_len += 1;
            }
            param_index += 1;
        }

        let mut step_index = 0;
        while step_index < self.len {
            out.steps[step_index] = rewrite_step_for_freeze(
                self.steps[step_index],
                &is_frozen,
                &frozen_at_default,
                &frozen_raw,
                &self.params,
                &new_param_index,
            );
            step_index += 1;
        }
        out.len = self.len;

        out.strip_fixed_noops_same_capacity()
            .merge_adjacent_fixed_same_capacity()
            .strip_fixed_noops_same_capacity()
    }

    /// Remove steps that are provably identity operations under any input.
    ///
    /// See [`LinkageBuf::strip_fixed_noops`] for semantics. `OUT_N` must equal
    /// the number of steps remaining after no-op removal; the function asserts
    /// this at const-eval time.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use linkage_blaze_core::LinkageFixed;
    /// const L: LinkageFixed<0, 0, 3> = LinkageFixed::start()
    ///     .yaw(0.0)
    ///     .forward(1.0);
    ///
    /// const STRIPPED: LinkageFixed<0, 0, 2> = L.strip_fixed_noops();
    /// ```
    pub const fn strip_fixed_noops<const OUT_N: usize>(self) -> LinkageFixed<DOF, MARKS, OUT_N> {
        let stripped = self.strip_fixed_noops_same_capacity();
        stripped.resize_steps()
    }

    /// Merge runs of consecutive fixed-value steps of the same motion type.
    ///
    /// See [`LinkageBuf::merge_adjacent_fixed`] for semantics. `OUT_N` must
    /// equal the number of steps remaining after all merges; the function
    /// asserts this at const-eval time.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use linkage_blaze_core::LinkageFixed;
    /// // Start (1) + Yaw (1) + Yaw (1) = 3 steps.
    /// const L: LinkageFixed<0, 0, 3> = LinkageFixed::start()
    ///     .yaw(57.6_f32.to_radians())
    ///     .yaw((-171.87_f32).to_radians());
    ///
    /// // Two consecutive yaws fold into one: Start + Yaw = 2 steps.
    /// const M: LinkageFixed<0, 0, 2> = L.merge_adjacent_fixed();
    /// ```
    pub const fn merge_adjacent_fixed<const OUT_N: usize>(self) -> LinkageFixed<DOF, MARKS, OUT_N> {
        let merged = self.merge_adjacent_fixed_same_capacity();
        merged.resize_steps()
    }

    const fn resize_steps<const OUT_N: usize>(self) -> LinkageFixed<DOF, MARKS, OUT_N> {
        let mut out_steps = [const { Step::Start }; OUT_N];
        let mut step_index = 0;
        while step_index < self.len {
            out_steps[step_index] = self.steps[step_index];
            step_index += 1;
        }
        assert!(self.len == OUT_N, "OUT_N does not match actual step count");
        LinkageFixed {
            steps: out_steps,
            len: self.len,
            params: self.params,
            param_len: self.param_len,
            mark_names: self.mark_names,
            mark_len: self.mark_len,
        }
    }

    const fn strip_fixed_noops_same_capacity(self) -> Self {
        let mut out_steps = [const { Step::Start }; N];
        let mut out_len = 0usize;
        let mut step_index = 0;
        while step_index < self.len {
            let step = self.steps[step_index];
            if !is_fixed_noop(step) {
                out_steps[out_len] = step;
                out_len += 1;
            }
            step_index += 1;
        }
        Self {
            steps: out_steps,
            len: out_len,
            params: self.params,
            param_len: self.param_len,
            mark_names: self.mark_names,
            mark_len: self.mark_len,
        }
    }

    const fn merge_adjacent_fixed_same_capacity(self) -> Self {
        let mut out_steps = [const { Step::Start }; N];
        let mut out_len = 0usize;
        let mut i = 0;
        while i < self.len {
            let step = self.steps[i];
            i += 1;
            let merged = match step {
                Step::Yaw(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.len {
                        if let Step::Yaw(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Yaw(Arg::Fixed(total))
                }
                Step::Pitch(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.len {
                        if let Step::Pitch(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Pitch(Arg::Fixed(total))
                }
                Step::Roll(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.len {
                        if let Step::Roll(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Roll(Arg::Fixed(total))
                }
                Step::Move(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.len {
                        if let Step::Move(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Move(Arg::Fixed(total))
                }
                Step::Left(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.len {
                        if let Step::Left(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Left(Arg::Fixed(total))
                }
                Step::Up(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.len {
                        if let Step::Up(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Up(Arg::Fixed(total))
                }
                other => other,
            };
            out_steps[out_len] = merged;
            out_len += 1;
        }
        Self {
            steps: out_steps,
            len: out_len,
            params: self.params,
            param_len: self.param_len,
            mark_names: self.mark_names,
            mark_len: self.mark_len,
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
        const MARKS2: usize,
        const N2: usize,
        const DOF_OUT: usize,
        const MARKS_OUT: usize,
        const N_OUT: usize,
    >(
        self,
        other: LinkageFixed<DOF2, MARKS2, N2>,
    ) -> LinkageFixed<DOF_OUT, MARKS_OUT, N_OUT> {
        assert!(DOF_OUT == DOF + DOF2, "DOF_OUT must equal DOF1 + DOF2");
        assert!(
            MARKS_OUT >= self.mark_len + other.mark_len,
            "MARKS_OUT must fit all marks from both linkages"
        );
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
            mark_names: [""; MARKS_OUT],
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

impl<const DOF: usize, const MARKS: usize, const N: usize> Linkage<DOF, MARKS>
    for LinkageFixed<DOF, MARKS, N>
{
    fn view(&self) -> LinkageView<'_, DOF, MARKS> {
        LinkageView::new(
            &self.params,
            &self.steps[..self.len],
            &self.mark_names,
            self.mark_len,
        )
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

impl<'a, const DOF: usize, const MARKS: usize> Linkage<DOF, MARKS> for LinkageView<'a, DOF, MARKS> {
    fn view(&self) -> LinkageView<'_, DOF, MARKS> {
        *self
    }
}

#[cfg(feature = "alloc")]
/// A growable linkage expression/storage type.
///
/// `LinkageBuf` stores linkage steps in a [`Vec`] and parameters in an array,
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
/// let linkage: LinkageBuf<1, 0> = LinkageBuf::start()
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
/// const FIXED: LinkageFixed<1, 0, 8> = LinkageFixed::start()
///     .define_param("distance", 0.5)
///     .forward_param("distance", 1.0, 5.0);
///
/// let buf = LinkageBuf::from(&FIXED);
/// let pose = buf.view().final_pose(&[0.5]);
/// assert!(pose.position().is_close_to(&Vec3::from([3.0, 0.0, 0.0]), 1e-5));
/// # }
/// ```
#[derive(Clone)]
/// A growable linkage expression/storage type.
///
/// Parameter specialization follows the same rules as [`LinkageFixed`]:
/// parameter indexes are identities, names are labels/selectors and may be
/// duplicated, name-based freezing must be unambiguous, and raw freeze values are
/// operation values rather than normalized slider values.
pub struct LinkageBuf<const DOF: usize, const MARKS: usize> {
    params: [Param; DOF],
    param_len: usize,
    steps: alloc::vec::Vec<Step>,
    mark_names: [&'static str; MARKS],
    mark_len: usize,
}

#[cfg(feature = "alloc")]
impl<const DOF: usize, const MARKS: usize> LinkageBuf<DOF, MARKS> {
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
            mark_names: [""; MARKS],
            mark_len: 0,
        }
    }

    /// Parse `.lb.rs` source into a growable linkage.
    ///
    /// Accepts the editor format with a leading `linkage![` or `linkage! [` wrapper
    /// and a trailing `]`, plus the fluent leading-dot method calls used by the
    /// linkage DSL.
    pub fn from_lb_rs(source: &str) -> Result<Self, String> {
        parse_lb_rs(source)
    }

    /// Number of runtime parameters this linkage expects.
    pub const DOF: usize = DOF;

    /// Mark-slot capacity of this linkage.
    pub const MARKS: usize = MARKS;

    /// Create a borrowed view for evaluation and rendering.
    ///
    /// The view erases the step capacity while preserving the degree-of-freedom `DOF`.
    /// All evaluation methods (poses, draw_items, etc.) operate on the view.
    #[must_use]
    #[inline]
    pub fn view(&self) -> LinkageView<'_, DOF, MARKS> {
        LinkageView::new(&self.params, &self.steps, &self.mark_names, self.mark_len)
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
        let index = match self.mark_index(name) {
            Some(index) => index,
            None => {
                assert!(self.mark_len < MARKS, "linkage has more marks than MARKS");
                let index = self.mark_len;
                self.mark_names[index] = name;
                self.mark_len += 1;
                index
            }
        };
        self.push_step_internal(Step::Mark { index });
        self
    }

    /// Restore a previously marked pose and pen state.
    /// Resolves `name` at build time using last-definition-wins (shadowing) semantics.
    pub fn restore(self, name: &'static str) -> Self {
        let index = match self.mark_index(name) {
            Some(i) => i,
            None => {
                panic!("restore: no mark found with name (mark must be defined before restore)")
            }
        };
        self.push_step(Step::Restore { index })
    }

    /// Insert a sphere at every joint (before and after each `Move`/`Left`/`Up` step).
    ///
    /// Mirrors `LinkageFixed::with_joint_spheres`, but operates on growable storage.
    pub fn with_joint_spheres(self, joint_radius: f32) -> Self {
        let mut out = Self {
            params: self.params,
            param_len: self.param_len,
            steps: alloc::vec::Vec::with_capacity(self.steps.len() * 3),
            mark_names: self.mark_names,
            mark_len: self.mark_len,
        };
        for step in &self.steps {
            let is_move = matches!(step, Step::Move(_) | Step::Left(_) | Step::Up(_));
            if is_move {
                out.steps.push(Step::Sphere(joint_radius));
            }
            out.steps.push(*step);
            if is_move {
                out.steps.push(Step::Sphere(joint_radius));
            }
        }
        out
    }

    /// Borrowing variant of `with_joint_spheres` — clones `self` then adds joint spheres.
    pub fn with_joint_spheres_ref(&self, joint_radius: f32) -> Self {
        self.clone().with_joint_spheres(joint_radius)
    }

    fn push_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    fn push_step_internal(&mut self, step: Step) {
        self.steps.push(step);
    }

    fn mark_index(&self, name: &str) -> Option<usize> {
        let mut mark_index = 0;
        while mark_index < self.mark_len {
            if str_eq(self.mark_names[mark_index], name) {
                return Some(mark_index);
            }
            mark_index += 1;
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

    /// Combine another owned linkage buffer's steps and parameters into this one.
    ///
    /// Consumes both buffers and produces a new one with DOF_OUT = DOF + DOF2.
    /// Parameters from `other` are concatenated after parameters from `self`.
    /// Steps from `other` (excluding its implicit Start step) are appended after this linkage's steps,
    /// with parameter and mark indices offset appropriately.
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
    /// let a = LinkageBuf::<1, 0>::start()
    ///     .define_param("x", 0.5)
    ///     .forward_param("x", 0.0, 10.0);
    ///
    /// let b = LinkageBuf::<1, 0>::start()
    ///     .define_param("y", 0.5)
    ///     .left_param("y", 0.0, 5.0);
    ///
    /// let c: LinkageBuf<2, 0> = a.combine(b);
    /// let params = [0.5, 0.5];
    /// let pose = c.view().final_pose(&params);
    /// # }
    /// ```
    pub fn combine<
        const DOF2: usize,
        const MARKS2: usize,
        const DOF_OUT: usize,
        const MARKS_OUT: usize,
    >(
        self,
        other: LinkageBuf<DOF2, MARKS2>,
    ) -> LinkageBuf<DOF_OUT, MARKS_OUT> {
        assert!(DOF_OUT == DOF + DOF2, "DOF_OUT must equal DOF + DOF2");
        assert!(
            MARKS_OUT >= self.mark_len + other.mark_len,
            "MARKS_OUT must fit all marks from both linkages"
        );

        let mut out = LinkageBuf {
            params: [Param::EMPTY; DOF_OUT],
            param_len: 0,
            steps: alloc::vec::Vec::new(),
            mark_names: [""; MARKS_OUT],
            mark_len: 0,
        };

        // Copy self's steps (including Start)
        out.steps.extend_from_slice(&self.steps);

        // Append other's steps (skip Start), offsetting param and mark indices
        let mark_offset = self.mark_len;
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

    /// Produce a new linkage by combining `self` (borrowed) with a `LinkageView` (also borrowed).
    ///
    /// Both inputs are preserved — `self` is not consumed, and `other` is accessed via a
    /// shared view. The name `combine_ref` signals that neither side is moved.
    /// Use `combine` when you are done with both inputs and want to avoid the clone.
    ///
    /// # Panics
    ///
    /// Panics if `DOF_OUT != DOF + DOF2`.
    pub fn combine_ref<
        const DOF2: usize,
        const MARKS2: usize,
        const DOF_OUT: usize,
        const MARKS_OUT: usize,
    >(
        &self,
        other: LinkageView<'_, DOF2, MARKS2>,
    ) -> LinkageBuf<DOF_OUT, MARKS_OUT> {
        assert!(DOF_OUT == DOF + DOF2, "DOF_OUT must equal DOF + DOF2");
        assert!(
            MARKS_OUT >= self.mark_len + other.mark_len(),
            "MARKS_OUT must fit all marks from both linkages"
        );

        let mut out = LinkageBuf {
            params: [Param::EMPTY; DOF_OUT],
            param_len: 0,
            steps: alloc::vec::Vec::new(),
            mark_names: [""; MARKS_OUT],
            mark_len: 0,
        };

        // Copy self's steps (including Start)
        out.steps.extend_from_slice(&self.steps);

        // Append steps from the view (skip Start)
        let mark_offset = self.mark_len;
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

        let mut i = 0;
        while i < self.mark_len {
            out.mark_names[i] = self.mark_names[i];
            i += 1;
        }
        let other_mark_names = other.mark_names();
        let mut i = 0;
        while i < other.mark_len() {
            out.mark_names[self.mark_len + i] = other_mark_names[i];
            i += 1;
        }
        out.mark_len = self.mark_len + other.mark_len();

        out
    }
}

#[cfg(feature = "alloc")]
impl<const DOF: usize, const MARKS: usize> LinkageBuf<DOF, MARKS> {
    /// Freeze exactly one parameter slot by index at a raw operation value.
    ///
    /// Parameter indexes are identities. Parameter names are labels and may be
    /// duplicated. `raw_value` is the fixed operation value, not a normalized
    /// slider value: rotations use degrees, while translations, radii, and widths
    /// use linkage units. The raw value must be inside every referenced step range
    /// for this slot.
    pub fn freeze_param_index<const OUT_DOF: usize>(
        self,
        param_index: usize,
        raw_value: f32,
    ) -> LinkageBuf<OUT_DOF, MARKS> {
        let mut is_frozen = [false; DOF];
        let frozen_at_default = [false; DOF];
        let mut frozen_raw = [0.0f32; DOF];

        assert!(
            param_index < self.param_len,
            "freeze param index out of bounds"
        );
        is_frozen[param_index] = true;
        frozen_raw[param_index] = raw_value;

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    /// Freeze the uniquely named parameter slot at a raw operation value.
    ///
    /// Panics if no parameter has `name`, or if more than one parameter has that
    /// name. Use [`freeze_param_index`](Self::freeze_param_index) when names are
    /// duplicated.
    pub fn freeze_param_name<const OUT_DOF: usize>(
        self,
        name: &'static str,
        raw_value: f32,
    ) -> LinkageBuf<OUT_DOF, MARKS> {
        let mut found_index = 0usize;
        let mut found_count = 0usize;
        let mut param_index = 0;
        while param_index < self.param_len {
            if str_eq(self.params[param_index].name, name) {
                found_index = param_index;
                found_count += 1;
            }
            param_index += 1;
        }
        assert!(found_count > 0, "freeze name not found in params");
        assert!(found_count == 1, "freeze name is ambiguous");

        self.freeze_param_index(found_index, raw_value)
    }

    /// Freeze exactly one parameter slot by index at its normalized default.
    pub fn freeze_param_index_at_default<const OUT_DOF: usize>(
        self,
        param_index: usize,
    ) -> LinkageBuf<OUT_DOF, MARKS> {
        let mut is_frozen = [false; DOF];
        let mut frozen_at_default = [false; DOF];
        let frozen_raw = [0.0f32; DOF];

        assert!(
            param_index < self.param_len,
            "freeze param index out of bounds"
        );
        is_frozen[param_index] = true;
        frozen_at_default[param_index] = true;

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    /// Freeze the uniquely named parameter slot at its normalized default.
    pub fn freeze_param_name_at_default<const OUT_DOF: usize>(
        self,
        name: &'static str,
    ) -> LinkageBuf<OUT_DOF, MARKS> {
        let mut found_index = 0usize;
        let mut found_count = 0usize;
        let mut param_index = 0;
        while param_index < self.param_len {
            if str_eq(self.params[param_index].name, name) {
                found_index = param_index;
                found_count += 1;
            }
            param_index += 1;
        }
        assert!(found_count > 0, "freeze name not found in params");
        assert!(found_count == 1, "freeze name is ambiguous");

        self.freeze_param_index_at_default(found_index)
    }

    /// Retain exactly the listed parameter slots and freeze all others at their defaults.
    pub fn retain_param_indexes<const OUT_DOF: usize>(
        self,
        indexes: &[usize],
    ) -> LinkageBuf<OUT_DOF, MARKS> {
        let mut index_index = 0;
        while index_index < indexes.len() {
            let retain_index = indexes[index_index];
            assert!(
                retain_index < self.param_len,
                "retain param index out of bounds"
            );
            let mut previous_index = 0;
            while previous_index < index_index {
                assert!(
                    indexes[previous_index] != retain_index,
                    "duplicate index in retain list"
                );
                previous_index += 1;
            }
            index_index += 1;
        }

        let mut is_frozen = [false; DOF];
        let mut frozen_at_default = [false; DOF];
        let frozen_raw = [0.0f32; DOF];

        let mut param_index = 0;
        while param_index < self.param_len {
            let mut found = false;
            let mut index_index = 0;
            while index_index < indexes.len() {
                if param_index == indexes[index_index] {
                    found = true;
                    break;
                }
                index_index += 1;
            }
            if !found {
                is_frozen[param_index] = true;
                frozen_at_default[param_index] = true;
            }
            param_index += 1;
        }

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    /// Retain parameters selected by name and freeze all others at their defaults.
    ///
    /// Each requested name must exist. If a requested name matches multiple
    /// parameter slots, all matching slots are retained. Duplicate names in the
    /// requested name list panic.
    pub fn retain_param_names<const OUT_DOF: usize>(
        self,
        names: &[&'static str],
    ) -> LinkageBuf<OUT_DOF, MARKS> {
        let mut ni = 0;
        while ni < names.len() {
            let retain_name = names[ni];
            let mut ni2 = 0;
            while ni2 < ni {
                assert!(
                    !str_eq(names[ni2], retain_name),
                    "duplicate name in retain list"
                );
                ni2 += 1;
            }
            let mut found = false;
            let mut pi = 0;
            while pi < self.param_len {
                if str_eq(self.params[pi].name, retain_name) {
                    found = true;
                    break;
                }
                pi += 1;
            }
            assert!(found, "retain name not found in params");
            ni += 1;
        }

        let mut is_frozen = [false; DOF];
        let mut frozen_at_default = [false; DOF];
        let frozen_raw = [0.0f32; DOF];

        let mut param_index = 0;
        while param_index < self.param_len {
            let mut found = false;
            let mut name_index = 0;
            while name_index < names.len() {
                if str_eq(self.params[param_index].name, names[name_index]) {
                    found = true;
                    break;
                }
                name_index += 1;
            }
            if !found {
                is_frozen[param_index] = true;
                frozen_at_default[param_index] = true;
            }
            param_index += 1;
        }

        self.freeze_with_map(is_frozen, frozen_at_default, frozen_raw)
    }

    fn freeze_with_map<const OUT_DOF: usize>(
        self,
        is_frozen: [bool; DOF],
        frozen_at_default: [bool; DOF],
        frozen_raw: [f32; DOF],
    ) -> LinkageBuf<OUT_DOF, MARKS> {
        let mut new_param_index = [0usize; DOF];
        let mut new_param_len = 0usize;
        let mut param_index = 0;
        while param_index < self.param_len {
            if !is_frozen[param_index] {
                new_param_index[param_index] = new_param_len;
                new_param_len += 1;
            }
            param_index += 1;
        }

        assert!(
            new_param_len == OUT_DOF,
            "OUT_DOF must equal DOF minus the number of frozen parameters"
        );

        let mut out = LinkageBuf {
            params: [Param::EMPTY; OUT_DOF],
            param_len: 0,
            steps: Vec::new(),
            mark_names: [""; MARKS],
            mark_len: self.mark_len,
        };

        let mut mark_index = 0;
        while mark_index < self.mark_len {
            out.mark_names[mark_index] = self.mark_names[mark_index];
            mark_index += 1;
        }

        let mut param_index = 0;
        while param_index < self.param_len {
            if !is_frozen[param_index] {
                out.params[new_param_index[param_index]] = self.params[param_index];
                out.param_len += 1;
            }
            param_index += 1;
        }

        out.steps = self
            .steps
            .into_iter()
            .map(|step| {
                rewrite_step_for_freeze(
                    step,
                    &is_frozen,
                    &frozen_at_default,
                    &frozen_raw,
                    &self.params,
                    &new_param_index,
                )
            })
            .collect();

        out.strip_fixed_noops()
            .merge_adjacent_fixed()
            .strip_fixed_noops()
    }

    /// Remove steps that are provably identity operations under any input.
    ///
    /// A fixed-value rotation or translation of exactly `0.0` has no effect on
    /// the pose. These accumulate after parameter specialization freezes
    /// channels whose physical value is zero. Stripping them makes the output of
    /// [`to_lb_rs`](crate::LinkageView::to_lb_rs) more readable.
    ///
    /// Only unconditionally-zero fixed steps are removed; variable-arg steps and
    /// non-motion steps (`Mark`, `Restore`, `PenUp`, `PenDown`, `Disk`, etc.)
    /// are left untouched.
    pub fn strip_fixed_noops(mut self) -> Self {
        self.steps.retain(|&step| !is_fixed_noop(step));
        self
    }

    /// Merge runs of consecutive fixed-value steps of the same motion type.
    ///
    /// For example, `.yaw(57.6).yaw(-171.87)` becomes `.yaw(-114.27)`. Any
    /// number of consecutive same-type fixed steps are folded into one. The
    /// merged value is the arithmetic sum of their arguments.
    ///
    /// Only `Yaw`, `Pitch`, `Roll`, `Move`, `Left`, and `Up` steps with
    /// `Fixed` arguments are merged. Variable-arg steps and non-motion steps
    /// break a run.
    ///
    /// Combine with [`strip_fixed_noops`](Self::strip_fixed_noops) afterward
    /// to also remove any merged steps whose sum is zero.
    pub fn merge_adjacent_fixed(self) -> Self {
        let mut out = Vec::with_capacity(self.steps.len());
        let mut i = 0;
        while i < self.steps.len() {
            let step = self.steps[i];
            i += 1;
            let merged = match step {
                Step::Yaw(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.steps.len() {
                        if let Step::Yaw(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Yaw(Arg::Fixed(total))
                }
                Step::Pitch(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.steps.len() {
                        if let Step::Pitch(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Pitch(Arg::Fixed(total))
                }
                Step::Roll(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.steps.len() {
                        if let Step::Roll(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Roll(Arg::Fixed(total))
                }
                Step::Move(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.steps.len() {
                        if let Step::Move(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Move(Arg::Fixed(total))
                }
                Step::Left(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.steps.len() {
                        if let Step::Left(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Left(Arg::Fixed(total))
                }
                Step::Up(Arg::Fixed(v)) => {
                    let mut total = v;
                    while i < self.steps.len() {
                        if let Step::Up(Arg::Fixed(v2)) = self.steps[i] {
                            total += v2;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    Step::Up(Arg::Fixed(total))
                }
                other => other,
            };
            out.push(merged);
        }
        Self {
            steps: out,
            params: self.params,
            param_len: self.param_len,
            mark_names: self.mark_names,
            mark_len: self.mark_len,
        }
    }
}

#[cfg(feature = "alloc")]
impl<const DOF: usize, const MARKS: usize> Linkage<DOF, MARKS> for LinkageBuf<DOF, MARKS> {
    fn view(&self) -> LinkageView<'_, DOF, MARKS> {
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
impl<const DOF: usize, const MARKS: usize, const N: usize> From<&LinkageFixed<DOF, MARKS, N>>
    for LinkageBuf<DOF, MARKS>
{
    fn from(linkage: &LinkageFixed<DOF, MARKS, N>) -> Self {
        Self {
            params: linkage.params,
            param_len: linkage.param_len,
            steps: linkage.steps[..linkage.len].to_vec(),
            mark_names: linkage.mark_names,
            mark_len: linkage.mark_len,
        }
    }
}

#[cfg(feature = "alloc")]
impl<'a, const DOF: usize, const MARKS: usize> From<&'a LinkageBuf<DOF, MARKS>>
    for LinkageView<'a, DOF, MARKS>
{
    fn from(linkage: &'a LinkageBuf<DOF, MARKS>) -> Self {
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
            Self::Mark { index } => Self::Mark {
                index: index + remember_offset,
            },
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

const fn is_fixed_noop(step: Step) -> bool {
    matches!(
        step,
        Step::Yaw(Arg::Fixed(v))
        | Step::Pitch(Arg::Fixed(v))
        | Step::Roll(Arg::Fixed(v))
        | Step::Move(Arg::Fixed(v))
        | Step::Left(Arg::Fixed(v))
        | Step::Up(Arg::Fixed(v))
        if v == 0.0
    )
}

const fn rewrite_arg_for_freeze(
    arg: Arg,
    is_frozen: &[bool],
    frozen_at_default: &[bool],
    frozen_raw: &[f32],
    params: &[Param],
    new_param_index: &[usize],
    is_rotation: bool,
) -> Arg {
    match arg {
        Arg::Fixed(_) => arg,
        Arg::Variable(variable_arg) => {
            if is_frozen[variable_arg.index] {
                let physical = frozen_physical_value(
                    variable_arg,
                    frozen_at_default,
                    frozen_raw,
                    params,
                    is_rotation,
                );
                Arg::Fixed(physical)
            } else {
                Arg::Variable(VariableArg {
                    index: new_param_index[variable_arg.index],
                    low: variable_arg.low,
                    span: variable_arg.span,
                })
            }
        }
    }
}

const fn rewrite_step_for_freeze(
    step: Step,
    is_frozen: &[bool],
    frozen_at_default: &[bool],
    frozen_raw: &[f32],
    params: &[Param],
    new_param_index: &[usize],
) -> Step {
    match step {
        Step::Yaw(arg) => Step::Yaw(rewrite_arg_for_freeze(
            arg,
            is_frozen,
            frozen_at_default,
            frozen_raw,
            params,
            new_param_index,
            true,
        )),
        Step::Pitch(arg) => Step::Pitch(rewrite_arg_for_freeze(
            arg,
            is_frozen,
            frozen_at_default,
            frozen_raw,
            params,
            new_param_index,
            true,
        )),
        Step::Roll(arg) => Step::Roll(rewrite_arg_for_freeze(
            arg,
            is_frozen,
            frozen_at_default,
            frozen_raw,
            params,
            new_param_index,
            true,
        )),
        Step::Move(arg) => Step::Move(rewrite_arg_for_freeze(
            arg,
            is_frozen,
            frozen_at_default,
            frozen_raw,
            params,
            new_param_index,
            false,
        )),
        Step::Left(arg) => Step::Left(rewrite_arg_for_freeze(
            arg,
            is_frozen,
            frozen_at_default,
            frozen_raw,
            params,
            new_param_index,
            false,
        )),
        Step::Up(arg) => Step::Up(rewrite_arg_for_freeze(
            arg,
            is_frozen,
            frozen_at_default,
            frozen_raw,
            params,
            new_param_index,
            false,
        )),
        Step::DiskParam(variable_arg) => {
            if is_frozen[variable_arg.index] {
                let physical = frozen_physical_value(
                    variable_arg,
                    frozen_at_default,
                    frozen_raw,
                    params,
                    false,
                );
                Step::Disk(physical)
            } else {
                Step::DiskParam(VariableArg {
                    index: new_param_index[variable_arg.index],
                    low: variable_arg.low,
                    span: variable_arg.span,
                })
            }
        }
        Step::RingParam(variable_arg) => {
            if is_frozen[variable_arg.index] {
                let physical = frozen_physical_value(
                    variable_arg,
                    frozen_at_default,
                    frozen_raw,
                    params,
                    false,
                );
                Step::Ring(physical)
            } else {
                Step::RingParam(VariableArg {
                    index: new_param_index[variable_arg.index],
                    low: variable_arg.low,
                    span: variable_arg.span,
                })
            }
        }
        Step::SphereParam(variable_arg) => {
            if is_frozen[variable_arg.index] {
                let physical = frozen_physical_value(
                    variable_arg,
                    frozen_at_default,
                    frozen_raw,
                    params,
                    false,
                );
                Step::Sphere(physical)
            } else {
                Step::SphereParam(VariableArg {
                    index: new_param_index[variable_arg.index],
                    low: variable_arg.low,
                    span: variable_arg.span,
                })
            }
        }
        other => other,
    }
}

const fn frozen_physical_value(
    variable_arg: VariableArg,
    frozen_at_default: &[bool],
    frozen_raw: &[f32],
    params: &[Param],
    is_rotation: bool,
) -> f32 {
    if frozen_at_default[variable_arg.index] {
        variable_arg.low + params[variable_arg.index].default * variable_arg.span
    } else {
        let raw_value = frozen_raw[variable_arg.index];
        let physical = if is_rotation {
            degrees_to_radians(raw_value)
        } else {
            raw_value
        };
        assert_raw_value_in_range(variable_arg, physical);
        physical
    }
}

const fn assert_raw_value_in_range(variable_arg: VariableArg, physical: f32) {
    let high = variable_arg.low + variable_arg.span;
    let (min, max) = if variable_arg.low <= high {
        (variable_arg.low, high)
    } else {
        (high, variable_arg.low)
    };
    assert!(
        physical >= min && physical <= max,
        "raw freeze value out of range"
    );
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
struct StyledPosesView<'a, const DOF: usize, const MARKS: usize> {
    steps: &'a [Step],
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
    marked: [MarkedState; MARKS],
}

impl<'a, const DOF: usize, const MARKS: usize> StyledPosesView<'a, DOF, MARKS> {
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
            }; MARKS],
        }
    }
}

impl<const DOF: usize, const MARKS: usize> Iterator for StyledPosesView<'_, DOF, MARKS> {
    type Item = StyledPose;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.index >= self.steps.len() {
                return None;
            }
            let step = &self.steps[self.index];
            self.index += 1;

            match step {
                Step::Mark { index } => {
                    self.marked[*index] = MarkedState {
                        pose: self.pose,
                        pen_style: self.pen_style,
                    };
                    continue;
                }
                Step::Restore { index } => {
                    let marked_state = self.marked[*index];
                    self.pose = marked_state.pose;
                    self.pen_style = marked_state.pen_style;
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
/// Iterator over [`DrawItem`]s produced by evaluating a linkage.
///
/// Obtain via [`LinkageView::draw_items`]. After exhausting the iterator the
/// [`marked_pose`](DrawItemIter::marked_pose) method lets you query the final
/// pose at any named mark.
pub struct DrawItemIter<'a, const DOF: usize, const MARKS: usize> {
    steps: &'a [Step],
    mark_names: &'a [&'static str; MARKS],
    params: &'a [f32; DOF],
    index: usize,
    pose: Pose,
    pen_style: PenStyle,
    marked: [MarkedState; MARKS],
}

impl<'a, const DOF: usize, const MARKS: usize> DrawItemIter<'a, DOF, MARKS> {
    fn new(
        steps: &'a [Step],
        mark_names: &'a [&'static str; MARKS],
        params_values: &'a [f32; DOF],
    ) -> Self {
        validate_params(params_values);
        Self {
            steps,
            mark_names,
            params: params_values,
            index: 0,
            pose: Pose::start(),
            pen_style: PenStyle::new(),
            marked: [MarkedState {
                pose: Pose::start(),
                pen_style: PenStyle::new(),
            }; MARKS],
        }
    }

    /// Return the pose recorded at the named mark, or `None` if the mark was
    /// never reached during iteration.  Call this after the iterator is
    /// exhausted to inspect final joint positions.
    #[must_use]
    pub fn marked_pose(&self, name: &str) -> Option<Pose> {
        self.mark_names
            .iter()
            .position(|&n| n == name)
            .map(|index| self.marked[index].pose)
    }
}

impl<const DOF: usize, const MARKS: usize> Iterator for DrawItemIter<'_, DOF, MARKS> {
    type Item = DrawItem;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.steps.len() {
            let step = &self.steps[self.index];
            self.index += 1;

            match step {
                Step::Mark { index } => {
                    self.marked[*index] = MarkedState {
                        pose: self.pose,
                        pen_style: self.pen_style,
                    };
                    continue;
                }
                Step::Restore { index } => {
                    let marked_state = self.marked[*index];
                    self.pose = marked_state.pose;
                    self.pen_style = marked_state.pen_style;
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
/// const CLOCK: LinkageFixed<2, 4, 48> =
///     linkage_fixed!("clock.lb.rs");
///
/// // Inside a function body:
/// let clock: LinkageFixed<2, 4, 48> =
///     linkage_fixed!("clock.lb.rs");
/// ```
///
/// Use the explicit-number form only when the surrounding type cannot be
/// inferred:
///
/// ```rust,ignore
/// const CLOCK: LinkageFixed<2, 4, 48> =
///     linkage_fixed!("clock.lb.rs", 2, 4, 48);
/// ```
#[macro_export]
macro_rules! linkage_fixed {
    ($path:literal) => {{
        macro_rules! __linkage_blaze_start {
            () => {
                $crate::LinkageFixed::start()
            };
        }
        include!($path)
    }};
    ($path:literal, $dof:expr, $marks:expr, $n:expr) => {{
        let linkage: $crate::LinkageFixed<$dof, $marks, $n> = $crate::linkage_fixed!($path);
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
/// let clock: LinkageBuf<2, 4> =
///     linkage_buf!("clock.lb.rs");
/// ```
///
/// Use the explicit-number form only when the surrounding type cannot be
/// inferred:
///
/// ```rust,ignore
/// let clock = linkage_buf!("clock.lb.rs", 2, 4);
/// ```
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! linkage_buf {
    ($path:literal) => {{
        macro_rules! __linkage_blaze_start {
            () => {
                $crate::LinkageBuf::start()
            };
        }
        include!($path)
    }};
    ($path:literal, $dof:expr, $marks:expr) => {{
        let linkage: $crate::LinkageBuf<$dof, $marks> = $crate::linkage_buf!($path);
        linkage
    }};
}

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests {
    use super::{Arg, DrawItem, LinkageFixed, Pose, Step, Vec3};
    #[cfg(feature = "alloc")]
    use super::{LinkageBuf, Rgb888};
    use crate::test_helpers::{
        assert_png_matches_expected, assert_pose_approx_eq, assert_pose_trace_matches_expected,
        draw_linkage_xy_canvas,
    };
    use std::{boxed::Box, error::Error};

    //todo000 *_param might not be a good suffix.
    const LINKAGE0: LinkageFixed<6, 0, 24> = LinkageFixed::start()
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
    const LINKAGE1: LinkageFixed<3, 0, 16> = LinkageFixed::start()
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
        const LINKAGE: LinkageFixed<0, 0, 4> = LinkageFixed::start().pen_width(0.0).forward(1.0);

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

    #[cfg(feature = "alloc")]
    #[test]
    fn serializes_linkage_view_to_lb_rs() {
        const LINKAGE: LinkageFixed<1, 1, 10> = LinkageFixed::start()
            .define_param("distance", 0.5)
            .pen_up()
            .mark("origin")
            .pen_color(Rgb888::new(10, 20, 30))
            .pen_width(2.0)
            .pen_down()
            .forward_param("distance", 1.0, 5.0)
            .restore("origin");

        let source = LINKAGE.view().to_lb_rs();

        assert!(source.starts_with("// DOF="));
        assert!(source.contains("linkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(source.contains(".define_param(\"distance\", 0.5)"));
        assert!(source.contains(".pen_color(Rgb888::new(10, 20, 30))"));
        assert!(source.contains(".forward_param(\"distance\", 1.0, 5.0)"));
        assert!(source.contains(".restore(\"origin\")"));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn parses_lb_rs_into_linkage_buf() {
        let source = r#"linkage![
    .define_param("distance", 0.5)
    .pen_color(Rgb888::new(10, 20, 30))
    .forward_param("distance", 1.0, 5.0)
]"#;

        let linkage = LinkageBuf::<1, 0>::from_lb_rs(source).expect("source should parse");
        let pose = linkage.view().final_pose(&[0.5]);

        assert!(
            pose.position()
                .is_close_to(&Vec3::from([3.0, 0.0, 0.0]), 1e-5)
        );
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn lb_rs_parser_rejects_integer_arguments() {
        let error = match LinkageBuf::<0, 0>::from_lb_rs("linkage![\n.forward(1)\n]") {
            Ok(_) => panic!("integer argument should fail"),
            Err(error) => error,
        };

        assert!(error.contains("is an integer"));
    }

    #[test]
    fn forward_moves_along_positive_x() {
        const LINKAGE: LinkageFixed<0, 0, 2> = LinkageFixed::start().forward(10.0);

        let params = [];
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([10.0, 0.0, 0.0]), 1e-6));
    }

    #[test]
    fn yaw_then_forward_moves_along_positive_y() {
        const LINKAGE: LinkageFixed<0, 0, 3> = LinkageFixed::start().yaw(90.0).forward(10.0);

        let params = [];
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 10.0, 0.0]), 1e-5));
    }

    #[test]
    fn left_moves_along_positive_y() {
        const LINKAGE: LinkageFixed<0, 0, 2> = LinkageFixed::start().left(10.0);

        let params = [];
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 10.0, 0.0]), 1e-6));
    }

    #[test]
    fn up_moves_along_positive_z() {
        const LINKAGE: LinkageFixed<0, 0, 2> = LinkageFixed::start().up(10.0);

        let params = [];
        let actual = LINKAGE.view().final_pose(&params).position();

        assert!(actual.is_close_to(&Vec3::from([0.0, 0.0, 10.0]), 1e-6));
    }

    #[test]
    fn translation_params_move_along_named_axes() {
        const LINKAGE: LinkageFixed<3, 0, 7> = LinkageFixed::start()
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
        const LINKAGE: LinkageFixed<0, 0, 5> = LinkageFixed::start()
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
        const _: LinkageFixed<2, 0, 2> = LinkageFixed::start()
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
        const LINKAGE: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("angle", 0.0) // index 0, default 0.0
            .define_param("angle", 1.0) // index 1, default 1.0 — shadows index 0
            .yaw_param("angle", 0.0, 90.0) // binds to index 1 (most recent)
            .forward(10.0);

        let params = [1.0, 0.0]; // index 0 = full, index 1 = zero
        let pos = LINKAGE.view().final_pose(&params).position();
        // yaw driven by index 1 = 0.0 → 0° → moves along +X
        assert!(pos.is_close_to(&Vec3::from([10.0, 0.0, 0.0]), 1e-5));
    }

    // ── freeze_param / retain_params ─────────────────────────────────────────

    #[test]
    fn freeze_param_index_uses_raw_rotation_degrees() {
        const BASE: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .define_param("len", 0.5)
            .yaw_param("angle", -180.0, 180.0)
            .forward_param("len", 0.0, 10.0);

        const FROZEN: LinkageFixed<1, 0, 5> = BASE.freeze_param_index(0, 90.0);

        assert_specialized_matches_original(BASE, &[0.75, 1.0], FROZEN, &[1.0]);
        let pos = FROZEN.view().final_pose(&[1.0]).position();
        assert!(pos.is_close_to(&Vec3::from([0.0, 10.0, 0.0]), 1e-4));
    }

    #[test]
    fn freeze_param_name_matches_unique_slot() {
        const BASE: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("yaw", 0.25)
            .define_param("dist", 0.5)
            .yaw_param("yaw", 0.0, 180.0)
            .forward_param("dist", 0.0, 8.0);

        const FROZEN_BY_NAME: LinkageFixed<1, 0, 5> = BASE.freeze_param_name("yaw", 45.0);
        const FROZEN_BY_DEFAULT: LinkageFixed<1, 0, 5> = BASE.freeze_param_name_at_default("yaw");

        let pos_by_name = FROZEN_BY_NAME.view().final_pose(&[0.5]).position();
        let pos_by_default = FROZEN_BY_DEFAULT.view().final_pose(&[0.5]).position();
        assert!(pos_by_name.is_close_to(&pos_by_default, 1e-6));
    }

    #[test]
    fn retain_param_names_freezes_unlisted_at_default() {
        const BASE: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -90.0, 90.0)
            .forward_param("dist", 0.0, 4.0);

        const RETAINED: LinkageFixed<1, 0, 5> = BASE.retain_param_names(&["dist"]);

        assert_specialized_matches_original(BASE, &[0.5, 1.0], RETAINED, &[1.0]);
        let pos = RETAINED.view().final_pose(&[1.0]).position();
        assert!(pos.is_close_to(&Vec3::from([4.0, 0.0, 0.0]), 1e-4));
    }

    #[test]
    fn freeze_param_index_freezes_only_that_shadowed_slot() {
        const BASE: LinkageFixed<3, 0, 5> = LinkageFixed::start()
            .define_param("x", 0.1)
            .forward_param("x", 0.0, 10.0)
            .define_param("y", 0.2)
            .left_param("y", 0.0, 10.0)
            .define_param("x", 0.3)
            .up_param("x", 0.0, 10.0);

        const FIRST_X: LinkageFixed<2, 0, 5> = BASE.freeze_param_index(0, 4.0);
        const SECOND_X: LinkageFixed<2, 0, 5> = BASE.freeze_param_index(2, 6.0);

        assert_specialized_matches_original(BASE, &[0.4, 0.8, 0.6], FIRST_X, &[0.8, 0.6]);
        assert_specialized_matches_original(BASE, &[0.4, 0.8, 0.6], SECOND_X, &[0.4, 0.8]);
    }

    #[test]
    fn retain_param_names_retains_every_shadowed_slot_in_original_order() {
        const BASE: LinkageFixed<3, 0, 5> = LinkageFixed::start()
            .define_param("x", 0.25)
            .forward_param("x", 0.0, 10.0)
            .define_param("y", 0.5)
            .left_param("y", 0.0, 10.0)
            .define_param("x", 0.75)
            .up_param("x", 0.0, 10.0);

        const RETAINED: LinkageFixed<2, 0, 5> = BASE.retain_param_names(&["x"]);
        let params = RETAINED.view().params();
        assert_eq!(params[0].name(), "x");
        assert_eq!(params[0].default(), 0.25);
        assert_eq!(params[1].name(), "x");
        assert_eq!(params[1].default(), 0.75);

        assert_specialized_matches_original(BASE, &[1.0, 0.5, 0.0], RETAINED, &[1.0, 0.0]);
    }

    #[test]
    fn retain_param_indexes_keeps_exact_shadowed_slots() {
        const RETAINED: LinkageFixed<2, 0, 8> = BRUCE.retain_param_indexes(&[0, 2]);

        let params = RETAINED.view().params();
        assert_eq!(params[0].name(), "Bruce");
        assert_eq!(params[0].default(), 0.10);
        assert_eq!(params[1].name(), "Bruce");
        assert_eq!(params[1].default(), 0.30);

        assert_specialized_matches_original(
            BRUCE,
            &[0.11, 0.20, 0.33, 0.40],
            RETAINED,
            &[0.11, 0.33],
        );
    }

    #[test]
    fn retain_param_names_freezes_non_retained_shadowed_slots_at_their_own_defaults() {
        const BASE: LinkageFixed<3, 0, 5> = LinkageFixed::start()
            .define_param("x", 0.25)
            .forward_param("x", 0.0, 10.0)
            .define_param("y", 0.5)
            .left_param("y", 0.0, 10.0)
            .define_param("x", 0.75)
            .up_param("x", 0.0, 10.0);

        const RETAINED: LinkageFixed<1, 0, 5> = BASE.retain_param_names(&["y"]);

        assert_specialized_matches_original(BASE, &[0.25, 1.0, 0.75], RETAINED, &[1.0]);
    }

    #[test]
    fn specialization_matches_original_for_index_and_name_selectors() {
        const BASE: LinkageFixed<3, 0, 5> = LinkageFixed::start()
            .define_param("x", 0.25)
            .forward_param("x", 0.0, 10.0)
            .define_param("y", 0.5)
            .left_param("y", 0.0, 10.0)
            .define_param("z", 0.75)
            .up_param("z", 0.0, 10.0);

        const SPECIALIZED: LinkageFixed<1, 0, 5> = BASE
            .freeze_param_index::<2>(0, 4.0)
            .retain_param_names(&["y"]);

        assert_specialized_matches_original(BASE, &[0.4, 0.8, 0.75], SPECIALIZED, &[0.8]);
    }

    const BRUCE: LinkageFixed<4, 0, 8> = LinkageFixed::start()
        .define_param("Bruce", 0.10)
        .pen_down()
        .forward_param("Bruce", 0.0, 100.0)
        .define_param("Bruce", 0.20)
        .left_param("Bruce", 0.0, 100.0)
        .define_param("Bruce", 0.30)
        .up_param("Bruce", 0.0, 100.0)
        .define_param("Bruce", 0.40)
        .yaw_param("Bruce", -180.0, 180.0);

    #[test]
    fn bruce_full_evaluation_uses_each_slot_bound_at_step_creation() {
        let pose = BRUCE.view().final_pose(&[0.11, 0.22, 0.33, 0.44]);

        assert_pose_close(
            pose,
            Pose::new(pose.orientation(), Vec3::from([11.0, 22.0, 33.0])),
            1e-4,
        );
    }

    #[test]
    fn bruce_freeze_param_index_freezes_one_slot() {
        const FROZEN: LinkageFixed<3, 0, 8> = BRUCE.freeze_param_index(2, 33.0);

        assert_specialized_matches_original(
            BRUCE,
            &[0.11, 0.22, 0.33, 0.44],
            FROZEN,
            &[0.11, 0.22, 0.44],
        );
    }

    #[test]
    fn bruce_freeze_param_index_at_default_freezes_one_slot() {
        const FROZEN: LinkageFixed<3, 0, 8> = BRUCE.freeze_param_index_at_default(2);

        assert_specialized_matches_original(
            BRUCE,
            &[0.11, 0.22, 0.30, 0.44],
            FROZEN,
            &[0.11, 0.22, 0.44],
        );
    }

    #[test]
    fn bruce_retain_param_names_retains_all_slots_named_bruce() {
        const RETAINED: LinkageFixed<4, 0, 8> = BRUCE.retain_param_names(&["Bruce"]);

        let params = RETAINED.view().params();
        assert_eq!(params[0].name(), "Bruce");
        assert_eq!(params[1].name(), "Bruce");
        assert_eq!(params[2].name(), "Bruce");
        assert_eq!(params[3].name(), "Bruce");

        assert_specialized_matches_original(
            BRUCE,
            &[0.11, 0.22, 0.33, 0.44],
            RETAINED,
            &[0.11, 0.22, 0.33, 0.44],
        );
    }

    #[test]
    fn bruce_retain_param_names_freezes_non_bruce_at_own_default() {
        const BASE: LinkageFixed<3, 0, 7> = LinkageFixed::start()
            .define_param("Bruce", 0.10)
            .pen_down()
            .forward_param("Bruce", 0.0, 100.0)
            .define_param("Terry", 0.77)
            .left_param("Terry", 0.0, 100.0)
            .define_param("Bruce", 0.30)
            .up_param("Bruce", 0.0, 100.0);

        const RETAINED: LinkageFixed<2, 0, 7> = BASE.retain_param_names(&["Bruce"]);

        assert_specialized_matches_original(BASE, &[0.11, 0.77, 0.33], RETAINED, &[0.11, 0.33]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn linkage_buf_retain_param_indexes_drops_multiple_params() {
        // Mirror of the const test above but using LinkageBuf.
        let base: LinkageBuf<3, 0> = LinkageBuf::start()
            .define_param("yaw", 0.5)
            .define_param("pitch", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("yaw", -90.0, 90.0)
            .pitch_param("pitch", -90.0, 90.0)
            .forward_param("dist", 0.0, 6.0);

        let frozen: LinkageBuf<1, 0> = base.retain_param_indexes(&[2]);

        let pos = frozen.view().final_pose(&[1.0]).position();
        assert!(pos.is_close_to(&Vec3::from([6.0, 0.0, 0.0]), 1e-4));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn linkage_buf_retain_params_handles_shadowed_non_retained_params_by_slot() {
        let base: LinkageBuf<3, 0> = LinkageBuf::start()
            .define_param("x", 0.25)
            .forward_param("x", 0.0, 10.0)
            .define_param("y", 0.5)
            .left_param("y", 0.0, 10.0)
            .define_param("x", 0.75)
            .up_param("x", 0.0, 10.0);

        let retained: LinkageBuf<1, 0> = base.retain_param_names(&["y"]);

        let pos = retained.view().final_pose(&[1.0]).position();
        assert!(pos.is_close_to(&Vec3::from([2.5, 10.0, 7.5]), 1e-4));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn linkage_buf_retain_param_names_uses_each_shadowed_slots_own_default() {
        let base: LinkageBuf<3, 0> = LinkageBuf::start()
            .define_param("x", 0.25)
            .forward_param("x", 0.0, 10.0)
            .define_param("y", 0.5)
            .left_param("y", 0.0, 10.0)
            .define_param("x", 0.75)
            .up_param("x", 0.0, 10.0);

        let frozen: LinkageBuf<1, 0> = base.retain_param_names(&["y"]);

        let pos = frozen.view().final_pose(&[1.0]).position();
        assert!(pos.is_close_to(&Vec3::from([2.5, 10.0, 7.5]), 1e-4));
    }

    // ── freeze/retain validation: unknown names, duplicates, out-of-range ─────

    #[test]
    #[should_panic(expected = "freeze name not found in params")]
    fn freeze_param_name_rejects_unknown_name() {
        let linkage: LinkageFixed<1, 0, 4> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .yaw_param("angle", -90.0, 90.0);
        let _: LinkageFixed<0, 0, 4> = linkage.freeze_param_name("typo", 0.0);
    }

    #[test]
    #[should_panic(expected = "freeze param index out of bounds")]
    fn freeze_param_index_rejects_out_of_bounds_index() {
        let linkage: LinkageFixed<1, 0, 4> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .yaw_param("angle", -90.0, 90.0);
        let _: LinkageFixed<0, 0, 4> = linkage.freeze_param_index(9, 0.0);
    }

    #[test]
    #[should_panic(expected = "freeze name is ambiguous")]
    fn freeze_param_name_rejects_duplicate_name() {
        let linkage: LinkageFixed<2, 0, 4> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .define_param("angle", 0.5)
            .yaw_param("angle", -90.0, 90.0);
        let _: LinkageFixed<1, 0, 4> = linkage.freeze_param_name("angle", 0.0);
    }

    #[test]
    #[should_panic(expected = "raw freeze value out of range")]
    fn freeze_param_index_rejects_raw_value_above_range() {
        let linkage: LinkageFixed<1, 0, 4> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .yaw_param("angle", -180.0, 180.0);
        let _: LinkageFixed<0, 0, 4> = linkage.freeze_param_index(0, 999.0);
    }

    #[test]
    #[should_panic(expected = "raw freeze value out of range")]
    fn freeze_param_index_rejects_raw_value_outside_one_referenced_range() {
        let linkage: LinkageFixed<1, 0, 4> = LinkageFixed::start()
            .define_param("x", 0.5)
            .yaw_param("x", -180.0, 180.0)
            .forward_param("x", 0.0, 10.0);
        let _: LinkageFixed<0, 0, 4> = linkage.freeze_param_index(0, 90.0);
    }

    #[test]
    #[should_panic(expected = "retain name not found in params")]
    fn retain_params_rejects_unknown_name() {
        let linkage: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -90.0, 90.0)
            .forward_param("dist", 0.0, 10.0);
        let _: LinkageFixed<1, 0, 5> = linkage.retain_param_names(&["unknown_param"]);
    }

    #[test]
    #[should_panic(expected = "duplicate name in retain list")]
    fn retain_params_rejects_duplicate_name() {
        let linkage: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -90.0, 90.0)
            .forward_param("dist", 0.0, 10.0);
        let _: LinkageFixed<2, 0, 5> = linkage.retain_param_names(&["angle", "angle"]);
    }

    #[test]
    #[should_panic(expected = "duplicate index in retain list")]
    fn retain_param_indexes_rejects_duplicate_index() {
        let linkage: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -90.0, 90.0)
            .forward_param("dist", 0.0, 10.0);
        let _: LinkageFixed<2, 0, 5> = linkage.retain_param_indexes(&[0, 0]);
    }

    #[test]
    fn retain_params_output_follows_original_param_order() {
        // Define params in order [x, y, z].  Retain in reverse order ["z", "x"].
        // Output should still be [x, z] — original order, not retain-list order.
        const BASE: LinkageFixed<3, 0, 6> = LinkageFixed::start()
            .define_param("x", 0.5)
            .define_param("y", 0.5)
            .define_param("z", 0.5)
            .forward_param("x", 0.0, 3.0)
            .left_param("z", 0.0, 7.0);

        const RETAINED: LinkageFixed<2, 0, 6> = BASE.retain_param_names(&["z", "x"]);

        // Output param 0 is "x" (original order), param 1 is "z".
        // forward("x") at 1.0 = 3 units along +X; left("z") at 0.0 = 0 lateral.
        let pos = RETAINED.view().final_pose(&[1.0, 0.0]).position();
        assert!(pos.is_close_to(&Vec3::from([3.0, 0.0, 0.0]), 1e-4));

        // left("z") at 1.0 = 7 units left from where x took us.
        let pos2 = RETAINED.view().final_pose(&[1.0, 1.0]).position();
        assert!(pos2.is_close_to(&Vec3::from([3.0, 7.0, 0.0]), 1e-4));
    }

    #[test]
    fn freeze_covers_all_translation_and_rotation_step_types() {
        // One param used across move/left/up/yaw/pitch/roll with distinct ranges.
        // Retain none freezes it at its normalized default, which resolves through
        // each step's own range.
        const BASE: LinkageFixed<1, 0, 8> = LinkageFixed::start()
            .define_param("t", 0.5)
            .forward_param("t", 0.0, 1.0) // Move: high = 1.0
            .left_param("t", 0.0, 2.0) // Left: high = 2.0
            .up_param("t", 0.0, 3.0) // Up:   high = 3.0
            .yaw_param("t", 0.0, 0.0) // Yaw:  high = 0.0  (no rotation)
            .pitch_param("t", 0.0, 0.0) // Pitch: high = 0.0
            .roll_param("t", 0.0, 0.0); // Roll:  high = 0.0

        const FROZEN: LinkageFixed<0, 0, 8> = BASE.retain_param_indexes(&[]);

        // Zero rotations, then move(0.5) + left(1) + up(1.5).
        let pos = FROZEN.view().final_pose(&[]).position();
        assert!(pos.is_close_to(&Vec3::from([0.5, 1.0, 1.5]), 1e-4));
    }

    #[test]
    fn freeze_param_index_at_default_supports_multiple_step_ranges() {
        // "t" appears twice with completely different low/high.
        // Frozen at 0.5 must use each step's own span, not a shared physical value.
        const BASE: LinkageFixed<1, 0, 4> = LinkageFixed::start()
            .define_param("t", 0.5)
            .forward_param("t", 0.0, 10.0) // at 0.5 → 5.0 units
            .left_param("t", 0.0, 20.0); // at 0.5 → 10.0 units

        const FROZEN: LinkageFixed<0, 0, 4> = BASE.freeze_param_index_at_default(0);

        let pos = FROZEN.view().final_pose(&[]).position();
        assert!(pos.is_close_to(&Vec3::from([5.0, 10.0, 0.0]), 1e-4));
    }

    #[test]
    fn linkage_fixed_strip_fixed_noops_removes_identity_motion_steps() {
        const BASE: LinkageFixed<0, 0, 5> = LinkageFixed::start()
            .yaw(0.0)
            .forward(2.0)
            .left(0.0)
            .up(1.0);

        const STRIPPED: LinkageFixed<0, 0, 3> = BASE.strip_fixed_noops();

        let steps = STRIPPED.view().steps();
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[0], Step::Start));
        assert_fixed_move(steps[1], 2.0);
        assert_fixed_up(steps[2], 1.0);
    }

    #[test]
    fn linkage_fixed_freeze_runs_cleanup_passes() {
        const BASE: LinkageFixed<1, 0, 6> = LinkageFixed::start()
            .define_param("t", 0.5)
            .yaw_param("t", -90.0, 90.0)
            .forward_param("t", 0.0, 4.0)
            .forward(6.0)
            .left_param("t", -2.0, 2.0)
            .up(1.0);

        const FROZEN: LinkageFixed<0, 0, 6> = BASE.freeze_param_name_at_default("t");

        let steps = FROZEN.view().steps();
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[0], Step::Start));
        assert_fixed_move(steps[1], 8.0);
        assert_fixed_up(steps[2], 1.0);

        let original_pose = BASE.view().final_pose(&[0.5]);
        let frozen_pose = FROZEN.view().final_pose(&[]);
        assert_pose_close(original_pose, frozen_pose, 1e-5);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn linkage_buf_freeze_runs_cleanup_passes() {
        let base: LinkageBuf<1, 0> = LinkageBuf::start()
            .define_param("t", 0.5)
            .yaw_param("t", -90.0, 90.0)
            .forward_param("t", 0.0, 4.0)
            .forward(6.0)
            .left_param("t", -2.0, 2.0)
            .up(1.0);

        let frozen: LinkageBuf<0, 0> = base.clone().freeze_param_name_at_default("t");

        let steps = frozen.view().steps();
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[0], Step::Start));
        assert_fixed_move(steps[1], 8.0);
        assert_fixed_up(steps[2], 1.0);

        let original_pose = base.view().final_pose(&[0.5]);
        let frozen_pose = frozen.view().final_pose(&[]);
        assert_pose_close(original_pose, frozen_pose, 1e-5);
    }

    #[cfg(feature = "alloc")]
    #[test]
    #[should_panic(expected = "freeze name not found in params")]
    fn linkage_buf_freeze_param_name_rejects_unknown_name() {
        let linkage: LinkageBuf<1, 0> = LinkageBuf::start()
            .define_param("angle", 0.5)
            .yaw_param("angle", -90.0, 90.0);
        let _: LinkageBuf<0, 0> = linkage.freeze_param_name("typo", 0.0);
    }

    #[cfg(feature = "alloc")]
    #[test]
    #[should_panic(expected = "retain name not found in params")]
    fn linkage_buf_retain_params_rejects_unknown_name() {
        let linkage: LinkageBuf<2, 0> = LinkageBuf::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -90.0, 90.0)
            .forward_param("dist", 0.0, 10.0);
        let _: LinkageBuf<1, 0> = linkage.retain_param_names(&["unknown_param"]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    #[should_panic(expected = "duplicate name in retain list")]
    fn linkage_buf_retain_params_rejects_duplicate_name() {
        let linkage: LinkageBuf<2, 0> = LinkageBuf::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -90.0, 90.0)
            .forward_param("dist", 0.0, 10.0);
        let _: LinkageBuf<2, 0> = linkage.retain_param_names(&["angle", "angle"]);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn linkage_buf_freeze_output_matches_linkage_fixed_freeze_output() {
        // Verify LinkageBuf::freeze_param_name produces identical poses
        // to the equivalent LinkageFixed specialization.
        const FIXED_BASE: LinkageFixed<2, 0, 5> = LinkageFixed::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -180.0, 180.0)
            .forward_param("dist", 0.0, 8.0);

        const FIXED_FROZEN: LinkageFixed<1, 0, 5> = FIXED_BASE.freeze_param_name("angle", -90.0);

        let buf_base: LinkageBuf<2, 0> = LinkageBuf::start()
            .define_param("angle", 0.5)
            .define_param("dist", 0.5)
            .yaw_param("angle", -180.0, 180.0)
            .forward_param("dist", 0.0, 8.0);
        let buf_frozen: LinkageBuf<1, 0> = buf_base.freeze_param_name("angle", -90.0);

        for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let pos_fixed = FIXED_FROZEN.view().final_pose(&[t]).position();
            let pos_buf = buf_frozen.view().final_pose(&[t]).position();
            assert!(pos_fixed.is_close_to(&pos_buf, 1e-5));
        }
    }

    fn assert_specialized_matches_original<
        const DOF: usize,
        const OUT_DOF: usize,
        const MARKS: usize,
        const N: usize,
    >(
        original: LinkageFixed<DOF, MARKS, N>,
        original_params: &[f32; DOF],
        specialized: LinkageFixed<OUT_DOF, MARKS, N>,
        specialized_params: &[f32; OUT_DOF],
    ) {
        assert_pose_close(
            original.view().final_pose(original_params),
            specialized.view().final_pose(specialized_params),
            1e-4,
        );
        assert_draw_items_close(
            original.view().draw_items(original_params),
            specialized.view().draw_items(specialized_params),
            1e-4,
        );
    }

    fn assert_draw_items_close(
        mut left: impl Iterator<Item = DrawItem>,
        mut right: impl Iterator<Item = DrawItem>,
        tolerance: f32,
    ) {
        loop {
            match (left.next(), right.next()) {
                (Some(left), Some(right)) => assert_draw_item_close(left, right, tolerance),
                (None, None) => break,
                (Some(_), None) => panic!("specialized linkage emitted fewer draw items"),
                (None, Some(_)) => panic!("specialized linkage emitted more draw items"),
            }
        }
    }

    fn assert_draw_item_close(left: DrawItem, right: DrawItem, tolerance: f32) {
        match (left, right) {
            (DrawItem::Stroke(left), DrawItem::Stroke(right)) => {
                assert_pose_close(left.start(), right.start(), tolerance);
                assert_pose_close(left.end(), right.end(), tolerance);
                assert!((left.width() - right.width()).abs() <= tolerance);
            }
            (DrawItem::Disk(left), DrawItem::Disk(right)) => {
                assert_pose_close(left.pose(), right.pose(), tolerance);
                assert!((left.radius() - right.radius()).abs() <= tolerance);
            }
            (DrawItem::Ring(left), DrawItem::Ring(right)) => {
                assert_pose_close(left.pose(), right.pose(), tolerance);
                assert!((left.radius() - right.radius()).abs() <= tolerance);
                assert!((left.width() - right.width()).abs() <= tolerance);
            }
            (DrawItem::Sphere(left), DrawItem::Sphere(right)) => {
                assert_pose_close(left.pose(), right.pose(), tolerance);
                assert!((left.radius() - right.radius()).abs() <= tolerance);
            }
            _ => panic!("draw item variants differ"),
        }
    }

    fn assert_pose_close(left: Pose, right: Pose, tolerance: f32) {
        assert!(left.position().is_close_to(&right.position(), tolerance));
        assert!(
            left.orientation()
                .is_close_to(&right.orientation(), tolerance)
        );
    }

    fn assert_fixed_move(step: Step, expected: f32) {
        match step {
            Step::Move(Arg::Fixed(actual)) => assert!((actual - expected).abs() <= 1e-6),
            _ => panic!("expected fixed move step"),
        }
    }

    fn assert_fixed_up(step: Step, expected: f32) {
        match step {
            Step::Up(Arg::Fixed(actual)) => assert!((actual - expected).abs() <= 1e-6),
            _ => panic!("expected fixed up step"),
        }
    }
}
