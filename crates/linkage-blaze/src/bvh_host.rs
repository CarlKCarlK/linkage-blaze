//! Host-side parsing and conversion for the Biovision Hierarchy (BVH)
//! motion-capture file format.

use std::collections::HashSet;
use std::fmt;
use std::sync::{Mutex, OnceLock};

use crate::LinkageBuf;

/// Promote a runtime joint name to `&'static str` for `LinkageBuf::mark`, which
/// shares its `mark_names: [&'static str; MARKS]` field with the const/no_std
/// `LinkageFixed` type. Names are interned so repeated joint names (BVH
/// skeletons reuse standard names like "Hips" or "Spine1" across clips) leak
/// only once each rather than once per call.
fn intern_mark_name(name: &str) -> &'static str {
    static INTERNED: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut set = INTERNED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap();
    if let Some(&existing) = set.get(name) {
        return existing;
    }
    // TODO  still leaks one allocation per unique name (bounded by the
    // interner now, rather than one per call). Consider whether LinkageBuf could
    // store owned names to avoid the leak entirely.
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

/// Parsed Biovision Hierarchy motion-capture clip: hierarchy plus samples.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    /// Joints in hierarchy order, including end sites.
    pub joints: Vec<Joint>,
    /// Raw motion samples in the file's channel order.
    pub samples: Vec<MotionSample>,
    /// Duration of one motion frame in seconds.
    pub sample_time: f32,
    channel_count: usize,
}

/// One joint or end site from a Biovision Hierarchy skeleton.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    /// Joint name from the source file.
    pub name: String,
    /// Index of the parent joint, or `None` for the root.
    pub parent: Option<usize>,
    /// Model-space offset from the parent, in source units.
    pub offset: [f32; 3],
    /// Ordered position and rotation channels for this joint.
    pub channels: Vec<Channel>,
}

/// A position or rotation channel in a Biovision Hierarchy joint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Channel {
    /// Position along the source X axis.
    Xposition,
    /// Position along the source Y axis.
    Yposition,
    /// Position along the source Z axis.
    Zposition,
    /// Rotation about the source X axis, in degrees.
    Xrotation,
    /// Rotation about the source Y axis, in degrees.
    Yrotation,
    /// Rotation about the source Z axis, in degrees.
    Zrotation,
}

/// One raw motion frame in the file's channel order.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionSample {
    /// Channel values; positions use source distance units and rotations use degrees.
    pub values: Vec<f32>,
}

/// Linkage-parameter mapping discovered from a parsed BVH clip.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParameterLayout {
    /// Parameters in the order used by the generated linkage.
    pub parameters: Vec<Parameter>,
}

impl ParameterLayout {
    /// Return the number of discovered linkage parameters.
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Return whether no linkage parameters were discovered.
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}

/// One linkage parameter mapped back to a source joint and channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    /// Zero-based index in the generated linkage parameter array.
    pub index: usize,
    /// Name used by the generated linkage DSL.
    pub linkage_name: &'static str,
    /// Index of the source joint in [`Clip::joints`].
    pub joint_index: usize,
    /// Source channel represented by this parameter.
    pub channel: Channel,
}

/// Discover ordered linkage parameters from a parsed BVH clip.
///
/// The returned layout retains the source joint and channel for each generated
/// parameter so samples can be converted consistently.
pub fn discover_bvh_parameters(clip: &Clip) -> Result<ParameterLayout, Error> {
    let mut parameters = Vec::new();

    for (joint_index, joint) in clip.joints.iter().enumerate() {
        for &channel in &joint.channels {
            push_bvh_parameter(&mut parameters, joint_index, &joint.name, channel);
        }
    }

    Ok(ParameterLayout { parameters })
}

/// Create a parameterized [`LinkageBuf`] from a parsed BVH clip.
///
/// `mark_joints` lists joint names that receive named marks at their
/// position after their own transform is applied.  These marks persist in the
/// output linkage so callers can look up the pose of specific body parts after
/// evaluation.  Hierarchical depth marks are always emitted in addition.
pub fn build_bvh_linkage_buf<const DOF: usize, const MARKS: usize>(
    clip: &Clip,
    layout: &ParameterLayout,
    mark_joints: &[&str],
) -> Result<LinkageBuf<DOF, MARKS>, Error> {
    let defaults = clip.samples.first().map_or_else(
        || Ok(Vec::new()),
        |sample| bvh_parameter_defaults(layout, sample),
    )?;
    build_bvh_linkage_buf_with_defaults(clip, layout, &defaults, mark_joints)
}

fn build_bvh_linkage_buf_with_defaults<const DOF: usize, const MARKS: usize>(
    clip: &Clip,
    layout: &ParameterLayout,
    defaults: &[f32],
    mark_joints: &[&str],
) -> Result<LinkageBuf<DOF, MARKS>, Error> {
    if layout.len() > DOF {
        return Err(Error::new(format!(
            "BVH parameter layout has {} parameter(s), but LinkageBuf DOF is {DOF}",
            layout.len()
        )));
    }

    let children = bvh_children(clip);
    let root_indices: Vec<usize> = clip
        .joints
        .iter()
        .enumerate()
        .filter(|(_, joint)| joint.parent.is_none())
        .map(|(joint_index, _)| joint_index)
        .collect();
    let multiple_roots = root_indices.len() >= 2;

    let needed_mark_count = bvh_needed_mark_count(clip, &children, mark_joints);
    if needed_mark_count > MARKS {
        return Err(Error::new(format!(
            "BVH needs {needed_mark_count} mark slot(s), but LinkageBuf MARKS is {MARKS}"
        )));
    }

    let mut linkage = LinkageBuf::start().pen_up();
    if multiple_roots {
        linkage = linkage.mark("origin");
    }
    for (parameter_index, parameter) in layout.parameters.iter().enumerate() {
        let default = defaults.get(parameter_index).copied().unwrap_or(0.5);
        linkage = linkage.define_param(parameter.linkage_name, default);
    }

    for (root_ordinal, joint_index) in root_indices.iter().enumerate() {
        if root_ordinal > 0 {
            linkage = linkage.restore("origin");
        }
        linkage = append_bvh_joint(
            linkage,
            clip,
            layout,
            &children,
            *joint_index,
            0,
            mark_joints,
        )?;
    }

    Ok(linkage)
}

fn bvh_needed_mark_count(clip: &Clip, children: &[Vec<usize>], mark_joints: &[&str]) -> usize {
    let root_count = clip.joints.iter().filter(|j| j.parent.is_none()).count();
    let origin_slots = usize::from(root_count >= 2);

    let joint_depths = bvh_joint_depths(clip);
    let mut branching_depths = std::collections::BTreeSet::new();
    for (joint_index, _) in clip.joints.iter().enumerate() {
        if children[joint_index].len() >= 2 {
            branching_depths.insert(joint_depths[joint_index]);
        }
    }

    let named_mark_count = mark_joints
        .iter()
        .filter(|&&name| clip.joints.iter().any(|j| j.name == name))
        .count();

    origin_slots + branching_depths.len() + named_mark_count
}

fn bvh_joint_depths(clip: &Clip) -> Vec<usize> {
    let mut depths = vec![0usize; clip.joints.len()];
    for (joint_index, joint) in clip.joints.iter().enumerate() {
        if let Some(parent) = joint.parent {
            depths[joint_index] = depths[parent] + 1;
        }
    }
    depths
}

fn bvh_annotations(clip: &Clip, children: &[Vec<usize>]) -> (Vec<String>, Vec<String>) {
    let mut mark_annotations = Vec::new();
    let mut restore_annotations = Vec::new();
    for (joint_index, joint) in clip.joints.iter().enumerate() {
        if joint.parent.is_none() {
            collect_annotations(
                clip,
                children,
                joint_index,
                &mut mark_annotations,
                &mut restore_annotations,
            );
        }
    }
    (mark_annotations, restore_annotations)
}

fn collect_annotations(
    clip: &Clip,
    children: &[Vec<usize>],
    joint_index: usize,
    mark_annotations: &mut Vec<String>,
    restore_annotations: &mut Vec<String>,
) {
    let joint_children = &children[joint_index];
    if joint_children.is_empty() {
        return;
    }
    if joint_children.len() >= 2 {
        mark_annotations.push(clip.joints[joint_index].name.clone());
    }
    for (child_ordinal, &child_index) in joint_children.iter().enumerate() {
        if child_ordinal > 0 {
            restore_annotations.push(clip.joints[joint_index].name.clone());
        }
        collect_annotations(
            clip,
            children,
            child_index,
            mark_annotations,
            restore_annotations,
        );
    }
}

fn annotate_depth_step_lines(
    lb_rs: String,
    mark_annotations: Vec<String>,
    restore_annotations: Vec<String>,
) -> String {
    let mut mark_iter = mark_annotations.into_iter();
    let mut restore_iter = restore_annotations.into_iter();
    let mut result = String::with_capacity(lb_rs.len());
    for line in lb_rs.lines() {
        let trimmed = line.trim_start();
        let annotation = if trimmed.starts_with(".mark(\"depth ") {
            mark_iter.next()
        } else if trimmed.starts_with(".restore(\"depth ") {
            restore_iter.next()
        } else {
            None
        };
        if let Some(joint_name) = annotation {
            result.push_str(line.trim_end());
            result.push_str(" // ");
            result.push_str(&joint_name);
            result.push('\n');
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

/// Return normalized linkage-parameter values for one BVH motion-capture frame.
pub fn bvh_sample_params<const DOF: usize>(
    layout: &ParameterLayout,
    sample: &MotionSample,
) -> Result<[f32; DOF], Error> {
    if layout.len() > DOF {
        return Err(Error::new(format!(
            "BVH parameter layout has {} parameter(s), but parameter array DOF is {DOF}",
            layout.len()
        )));
    }

    let defaults = bvh_parameter_defaults(layout, sample)?;
    let mut params = [0.5; DOF];
    for (parameter_index, default) in defaults.into_iter().enumerate() {
        params[parameter_index] = default;
    }

    Ok(params)
}

/// Convert Biovision Hierarchy motion-capture text into generated `.lb.rs` source.
///
/// The generated linkage uses defaults from the first BVH motion sample, so
/// loading the generated file starts in a captured pose. Mark names use
/// `"depth N"` slots (one per tree level), and each `.restore` line carries
/// a comment naming the joint being restored.
pub fn bvh_to_lb_rs<const DOF: usize, const MARKS: usize>(
    source: &str,
    mark_joints: &[&str],
) -> Result<String, Error> {
    let clip = parse_bvh(source)?;
    let layout = discover_bvh_parameters(&clip)?;
    let linkage = build_bvh_linkage_buf::<DOF, MARKS>(&clip, &layout, mark_joints)?;
    let children = bvh_children(&clip);
    let (mark_annotations, restore_annotations) = bvh_annotations(&clip, &children);
    Ok(annotate_depth_step_lines(
        linkage.view().to_lb_rs(),
        mark_annotations,
        restore_annotations,
    ))
}

/// Parse Biovision Hierarchy skeleton and motion text into a [`Clip`].
pub fn parse_bvh(source: &str) -> Result<Clip, Error> {
    let mut parser = BvhParser::new(source);

    parser.expect("HIERARCHY")?;
    parser.expect("ROOT")?;
    let root_name = parser.next_string("root name")?;
    parser.parse_joint(root_name, None)?;
    parser.expect("MOTION")?;
    parser.expect("Frames:")?;
    let sample_count = parser.next_usize("sample count")?;
    parser.expect("Frame")?; // BVH file syntax: "Frame Time:"
    parser.expect("Time:")?;
    let sample_time = parser.next_f32("sample time")?;

    let mut samples = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let mut values = Vec::with_capacity(parser.channel_count);
        for _ in 0..parser.channel_count {
            values.push(parser.next_f32("BVH sample channel value")?);
        }
        if values.len() != parser.channel_count {
            return Err(Error::new(format!(
                "BVH sample {sample_index} has {} values, expected {}",
                values.len(),
                parser.channel_count
            )));
        }
        samples.push(MotionSample { values });
    }

    Ok(Clip {
        joints: parser.joints,
        samples,
        sample_time,
        channel_count: parser.channel_count,
    })
}

fn push_bvh_parameter(
    parameters: &mut Vec<Parameter>,
    joint_index: usize,
    joint_name: &str,
    channel: Channel,
) {
    let index = parameters.len();
    let linkage_name = bvh_linkage_name(joint_name, channel);
    parameters.push(Parameter {
        index,
        linkage_name,
        joint_index,
        channel,
    });
}

fn bvh_linkage_name(joint_name: &str, channel: Channel) -> &'static str {
    let mut name = String::with_capacity(joint_name.len() + 1 + bvh_channel_name(channel).len());
    push_sanitized_name_part(&mut name, joint_name);
    name.push('_');
    name.push_str(bvh_channel_name(channel));
    Box::leak(name.into_boxed_str())
}

fn depth_mark_name(depth: usize) -> &'static str {
    const NAMES: &[&str] = &[
        "depth 0", "depth 1", "depth 2", "depth 3", "depth 4", "depth 5", "depth 6", "depth 7",
        "depth 8", "depth 9",
    ];
    if depth < NAMES.len() {
        NAMES[depth]
    } else {
        intern_mark_name(&format!("depth {depth}"))
    }
}

fn push_sanitized_name_part(name: &mut String, value: &str) {
    let mut previous_was_underscore = false;
    let mut previous_was_lowercase_or_digit = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase()
                && previous_was_lowercase_or_digit
                && !previous_was_underscore
            {
                name.push('_');
            }
            name.push(character.to_ascii_lowercase());
            previous_was_underscore = false;
            previous_was_lowercase_or_digit =
                character.is_ascii_lowercase() || character.is_ascii_digit();
        } else if !previous_was_underscore {
            name.push('_');
            previous_was_underscore = true;
            previous_was_lowercase_or_digit = false;
        }
    }
    while name.ends_with('_') {
        name.pop();
    }
}

fn bvh_channel_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Xposition => "xposition",
        Channel::Yposition => "yposition",
        Channel::Zposition => "zposition",
        Channel::Xrotation => "xrotation",
        Channel::Yrotation => "yrotation",
        Channel::Zrotation => "zrotation",
    }
}

fn apply_bvh_joint_parameters<const DOF: usize, const MARKS: usize>(
    mut linkage: LinkageBuf<DOF, MARKS>,
    layout: &ParameterLayout,
    joint_index: usize,
) -> LinkageBuf<DOF, MARKS> {
    for parameter in layout
        .parameters
        .iter()
        .filter(|parameter| parameter.joint_index == joint_index)
    {
        let (low, high) = bvh_parameter_range(parameter.channel);
        linkage = match parameter.channel {
            Channel::Xposition => linkage.left_param(parameter.linkage_name, low, high),
            Channel::Yposition => linkage.up_param(parameter.linkage_name, low, high),
            Channel::Zposition => linkage.forward_param(parameter.linkage_name, low, high),
            Channel::Xrotation => linkage.pitch_param(parameter.linkage_name, low, high),
            Channel::Yrotation => linkage.yaw_param(parameter.linkage_name, low, high),
            Channel::Zrotation => linkage.roll_param(parameter.linkage_name, low, high),
        };
    }

    linkage
}

fn bvh_parameter_defaults(
    layout: &ParameterLayout,
    sample: &MotionSample,
) -> Result<Vec<f32>, Error> {
    let mut defaults = Vec::with_capacity(layout.len());

    for parameter in &layout.parameters {
        let value =
            sample.values.get(parameter.index).copied().ok_or_else(|| {
                Error::new(format!("BVH sample missing channel {}", parameter.index))
            })?;
        defaults.push(normalize_bvh_parameter_default(parameter, value)?);
    }

    Ok(defaults)
}

fn normalize_bvh_parameter_default(parameter: &Parameter, value: f32) -> Result<f32, Error> {
    let (low, high) = bvh_parameter_range(parameter.channel);
    let default = snap_centered_default((value - low) / (high - low));

    if !(0.0..=1.0).contains(&default) {
        return Err(Error::new(format!(
            "BVH value {value} for channel {:?} is outside [{low}, {high}]",
            parameter.channel
        )));
    }

    Ok(default)
}

fn snap_centered_default(default: f32) -> f32 {
    const CENTER_DEFAULT: f32 = 0.5;
    const CENTER_DEFAULT_EPSILON: f32 = 0.01;

    if (default - CENTER_DEFAULT).abs() <= CENTER_DEFAULT_EPSILON {
        CENTER_DEFAULT
    } else {
        default
    }
}

fn bvh_parameter_range(channel: Channel) -> (f32, f32) {
    match channel {
        Channel::Xposition | Channel::Yposition | Channel::Zposition => (-300.0, 300.0),
        Channel::Xrotation | Channel::Yrotation | Channel::Zrotation => (-720.0, 720.0),
    }
}

fn bvh_children(clip: &Clip) -> Vec<Vec<usize>> {
    let mut children = vec![Vec::new(); clip.joints.len()];
    for (joint_index, joint) in clip.joints.iter().enumerate() {
        if let Some(parent_index) = joint.parent {
            children[parent_index].push(joint_index);
        }
    }

    children
}

fn append_bvh_joint<const DOF: usize, const MARKS: usize>(
    mut linkage: LinkageBuf<DOF, MARKS>,
    clip: &Clip,
    layout: &ParameterLayout,
    children: &[Vec<usize>],
    joint_index: usize,
    depth: usize,
    mark_joints: &[&str],
) -> Result<LinkageBuf<DOF, MARKS>, Error> {
    linkage = apply_bvh_joint_parameters(linkage, layout, joint_index);

    let joint_name = clip.joints[joint_index].name.as_str();
    if mark_joints.contains(&joint_name) {
        linkage = linkage.mark(intern_mark_name(joint_name));
    }

    let joint_children = &children[joint_index];
    if joint_children.is_empty() {
        return Ok(linkage);
    }

    let branching = joint_children.len() >= 2;
    if branching {
        linkage = linkage.mark(depth_mark_name(depth));
    }

    for (child_ordinal, &child_index) in joint_children.iter().enumerate() {
        if child_ordinal > 0 {
            linkage = linkage.restore(depth_mark_name(depth));
        }
        linkage = append_offset_segment(linkage, clip.joints[child_index].offset);
        linkage = append_bvh_joint(
            linkage,
            clip,
            layout,
            children,
            child_index,
            depth + 1,
            mark_joints,
        )?;
    }

    Ok(linkage)
}

struct BvhParser {
    tokens: Vec<String>,
    index: usize,
    joints: Vec<Joint>,
    channel_count: usize,
}

impl BvhParser {
    fn new(source: &str) -> Self {
        let tokens = source
            .split_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        Self {
            tokens,
            index: 0,
            joints: Vec::new(),
            channel_count: 0,
        }
    }

    fn parse_joint(&mut self, name: String, parent: Option<usize>) -> Result<usize, Error> {
        let joint_index = self.joints.len();
        self.joints.push(Joint {
            name,
            parent,
            offset: [0.0, 0.0, 0.0],
            channels: Vec::new(),
        });

        self.expect("{")?;
        loop {
            match self.peek() {
                Some("OFFSET") => {
                    self.index += 1;
                    self.joints[joint_index].offset = [
                        self.next_f32("BVH offset x")?,
                        self.next_f32("BVH offset y")?,
                        self.next_f32("BVH offset z")?,
                    ];
                }
                Some("CHANNELS") => {
                    self.index += 1;
                    let channel_count = self.next_usize("BVH channel count")?;
                    let mut channels = Vec::with_capacity(channel_count);
                    for _ in 0..channel_count {
                        channels.push(self.next_channel()?);
                    }
                    self.channel_count += channels.len();
                    self.joints[joint_index].channels = channels;
                }
                Some("JOINT") => {
                    self.index += 1;
                    let child_name = self.next_string("BVH joint name")?;
                    self.parse_joint(child_name, Some(joint_index))?;
                }
                Some("End") => {
                    self.index += 1;
                    self.expect("Site")?;
                    self.parse_end_site(joint_index)?;
                }
                Some("}") => {
                    self.index += 1;
                    return Ok(joint_index);
                }
                Some(token) => {
                    return Err(Error::new(format!("unexpected BVH token `{token}`")));
                }
                None => return Err(Error::new("unexpected end of BVH hierarchy")),
            }
        }
    }

    fn parse_end_site(&mut self, parent: usize) -> Result<usize, Error> {
        let end_index = self.joints.len();
        let name = format!("{}_end_{}", self.joints[parent].name, end_index);
        self.joints.push(Joint {
            name,
            parent: Some(parent),
            offset: [0.0, 0.0, 0.0],
            channels: Vec::new(),
        });

        self.expect("{")?;
        self.expect("OFFSET")?;
        self.joints[end_index].offset = [
            self.next_f32("BVH end offset x")?,
            self.next_f32("BVH end offset y")?,
            self.next_f32("BVH end offset z")?,
        ];
        self.expect("}")?;

        Ok(end_index)
    }

    fn expect(&mut self, expected: &str) -> Result<(), Error> {
        let token = self.next_string(expected)?;
        if token != expected {
            return Err(Error::new(format!(
                "expected BVH token `{expected}`, got `{token}`"
            )));
        }

        Ok(())
    }

    fn next_channel(&mut self) -> Result<Channel, Error> {
        let token = self.next_string("BVH channel")?;
        match token.as_str() {
            "Xposition" => Ok(Channel::Xposition),
            "Yposition" => Ok(Channel::Yposition),
            "Zposition" => Ok(Channel::Zposition),
            "Xrotation" => Ok(Channel::Xrotation),
            "Yrotation" => Ok(Channel::Yrotation),
            "Zrotation" => Ok(Channel::Zrotation),
            _ => Err(Error::new(format!("unknown BVH channel `{token}`"))),
        }
    }

    fn next_f32(&mut self, field_name: &str) -> Result<f32, Error> {
        let token = self.next_string(field_name)?;
        token
            .parse::<f32>()
            .map_err(|_| Error::new(format!("expected f32 {field_name}, got `{token}`")))
    }

    fn next_usize(&mut self, field_name: &str) -> Result<usize, Error> {
        let token = self.next_string(field_name)?;
        token
            .parse::<usize>()
            .map_err(|_| Error::new(format!("expected integer {field_name}, got `{token}`")))
    }

    fn next_string(&mut self, field_name: &str) -> Result<String, Error> {
        let token = self
            .tokens
            .get(self.index)
            .ok_or_else(|| Error::new(format!("missing {field_name}")))?;
        self.index += 1;

        Ok(token.clone())
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.index).map(String::as_str)
    }
}

fn append_offset_segment<const DOF: usize, const MARKS: usize>(
    mut linkage: LinkageBuf<DOF, MARKS>,
    offset: [f32; 3],
) -> LinkageBuf<DOF, MARKS> {
    let [bvh_x, bvh_y, bvh_z] = offset;
    let direction_x = bvh_z;
    let direction_y = bvh_x;
    let direction_z = bvh_y;
    let length = direction_x.hypot(direction_y).hypot(direction_z);
    if length < 0.0001 {
        return linkage;
    }

    let horizontal_length = direction_x.hypot(direction_y);
    let yaw_degrees = direction_y.atan2(direction_x).to_degrees();
    let pitch_degrees = -direction_z.atan2(horizontal_length).to_degrees();

    if !is_nearly_zero_degrees(yaw_degrees) {
        linkage = linkage.yaw(yaw_degrees);
    }
    if !is_nearly_zero_degrees(pitch_degrees) {
        linkage = linkage.pitch(pitch_degrees);
    }

    linkage = linkage.pen_down().forward(length).pen_up();

    if !is_nearly_zero_degrees(pitch_degrees) {
        linkage = linkage.pitch(-pitch_degrees);
    }
    if !is_nearly_zero_degrees(yaw_degrees) {
        linkage = linkage.yaw(-yaw_degrees);
    }

    linkage
}

fn is_nearly_zero_degrees(degrees: f32) -> bool {
    const ANGLE_EPSILON_DEGREES: f32 = 0.0001;

    degrees.abs() < ANGLE_EPSILON_DEGREES
}

/// Diagnostic returned by host-side Biovision Hierarchy parsing and conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    line_number: Option<usize>,
    message: String,
}

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self {
            line_number: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line_number) = self.line_number {
            write!(formatter, "line {line_number}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    const BVH: &str = r#"
HIERARCHY
ROOT hip
{
  OFFSET 0 0 0
  CHANNELS 6 Xposition Yposition Zposition Zrotation Yrotation Xrotation
  JOINT chest
  {
    OFFSET 0 10 0
    CHANNELS 3 Zrotation Xrotation Yrotation
    JOINT leftArm
    {
      OFFSET 5 4 0
      CHANNELS 3 Zrotation Xrotation Yrotation
      End Site
      {
        OFFSET 5 0 0
      }
    }
    JOINT rightArm
    {
      OFFSET -5 4 0
      CHANNELS 3 Zrotation Xrotation Yrotation
      End Site
      {
        OFFSET -5 0 0
      }
    }
  }
}
MOTION
Frames: 2
Frame Time: 0.0333333
0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
1 2 3 10 20 30 40 50 60 70 80 90 100 110 120
"#;

    const BVH_X_ROTATION: &str = r#"
HIERARCHY
ROOT root
{
  OFFSET 0 0 0
  CHANNELS 1 Xrotation
  JOINT child
  {
    OFFSET 0 10 0
    CHANNELS 0
    End Site
    {
      OFFSET 0 0 0
    }
  }
}
MOTION
Frames: 1
Frame Time: 0.0333333
90
"#;

    #[test]
    fn parses_bvh_hierarchy_and_motion() {
        let clip = parse_bvh(BVH).expect("BVH should parse");

        assert_eq!(clip.joints.len(), 6);
        assert_eq!(clip.samples.len(), 2);
        assert_eq!(clip.channel_count, 15);
        assert_eq!(clip.joints[0].name, "hip");
        assert_eq!(clip.joints[2].name, "leftArm");
        assert_eq!(clip.joints[3].parent, Some(2));
        assert_eq!(clip.samples[1].values[14], 120.0);
    }

    #[test]
    fn builds_bvh_linkage_buf_and_sample_params() -> Result<(), crate::Error> {
        let clip = parse_bvh(BVH).expect("BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("BVH layout should parse");
        let linkage =
            build_bvh_linkage_buf::<32, 8>(&clip, &layout, &[]).expect("BVH linkage should build");
        let params =
            bvh_sample_params::<32>(&layout, &clip.samples[1]).expect("params should build");

        assert_eq!(layout.len(), 15);
        assert_eq!(params[0], 0.5);
        assert_eq!(params[1], 0.5);
        assert_eq!(params[2], 0.5);
        assert!(params[6] > 0.5);
        assert!(linkage.view().draw_items_3d(&params)?.count() >= 5);
        Ok(())
    }

    #[test]
    fn converts_bvh_to_lb_rs_source() -> Result<(), crate::Error> {
        let source = bvh_to_lb_rs::<32, 8>(BVH, &[]).expect("BVH should serialize");
        let linkage =
            LinkageBuf::<32, 8>::from_lb_rs(&source).expect("generated source should parse");

        assert!(source.contains("\nlinkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(source.contains(".define_param(\"hip_xposition\""));
        assert!(source.contains(".define_param(\"chest_zrotation\""));
        assert!(
            !source.contains(".mark(\"depth 0\""),
            "single-child hip should not be marked"
        );
        assert!(source.contains(".mark(\"depth 1\") // chest"));
        assert!(source.contains(".restore(\"depth 1\") // chest"));
        assert!(linkage.view().draw_items_3d(&[0.5; 32])?.count() >= 5);
        Ok(())
    }

    #[test]
    fn snaps_near_centered_bvh_defaults_to_half() {
        assert_eq!(snap_centered_default(0.5006703), 0.5);
        assert_eq!(snap_centered_default(0.4979823), 0.5);
        assert_eq!(snap_centered_default(0.5101), 0.5101);
        assert_eq!(snap_centered_default(0.4899), 0.4899);
    }

    #[test]
    fn bvh_parameter_names_use_joint_and_channel_names() {
        assert_eq!(
            bvh_linkage_name("rThumb1", Channel::Zrotation),
            "r_thumb1_zrotation"
        );
        assert_eq!(
            bvh_linkage_name("leftEye", Channel::Xposition),
            "left_eye_xposition"
        );
    }

    #[test]
    fn depth_mark_names_are_depth_prefixed() {
        assert_eq!(depth_mark_name(0), "depth 0");
        assert_eq!(depth_mark_name(5), "depth 5");
        assert_eq!(depth_mark_name(9), "depth 9");
        assert_eq!(depth_mark_name(10), "depth 10");
    }

    #[test]
    fn bvh_rotation_axes_are_remapped_to_linkage_axes() -> Result<(), crate::Error> {
        let clip = parse_bvh(BVH_X_ROTATION).expect("BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("BVH layout should parse");
        let linkage =
            build_bvh_linkage_buf::<1, 4>(&clip, &layout, &[]).expect("BVH linkage should build");
        let params =
            bvh_sample_params::<1>(&layout, &clip.samples[0]).expect("params should build");
        let stroke = linkage
            .view()
            .draw_items_3d(&params)?
            .find_map(|draw_item_3d| match draw_item_3d {
                crate::render::Item3d::Stroke(stroke) => Some(stroke),
                _ => None,
            })
            .expect("offset should draw a stroke");

        assert!(
            stroke
                .end()
                .position()
                .is_close_to(&crate::Vec3::from([10.0, 0.0, 0.0]), 1e-4)
        );
        Ok(())
    }

    const REAL_PIROUETTE_BVH_PATH: &str = "src/assets/mocap/pirouette.bvh";
    const PIROUETTE_GOLDEN_LB_RS_PATH: &str = "tests/golden/pirouette.lb.rs";

    fn read_real_pirouette_bvh() -> String {
        std::fs::read_to_string(REAL_PIROUETTE_BVH_PATH)
            .unwrap_or_else(|error| panic!("failed to read `{REAL_PIROUETTE_BVH_PATH}`: {error}"))
    }

    #[test]
    fn builds_real_bvh_linkage() -> Result<(), crate::Error> {
        let bvh = read_real_pirouette_bvh();

        let clip = parse_bvh(&bvh).expect("real BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("real BVH layout should parse");
        let linkage = build_bvh_linkage_buf::<256, 64>(&clip, &layout, &[])
            .expect("real BVH linkage should build");
        let params =
            bvh_sample_params::<256>(&layout, &clip.samples[0]).expect("real params should build");

        assert!(clip.joints.len() > 40);
        assert!(clip.samples.len() > 500);
        assert!(layout.len() > 120);
        assert!(linkage.view().draw_items_3d(&params)?.count() > 40);
        Ok(())
    }

    #[test]
    fn converts_real_bvh_to_lb_rs() -> Result<(), crate::Error> {
        let bvh = read_real_pirouette_bvh();

        let source = bvh_to_lb_rs::<256, 64>(&bvh, &[]).expect("real BVH should serialize");
        let linkage =
            LinkageBuf::<256, 64>::from_lb_rs(&source).expect("real generated source should parse");

        assert!(source.contains("\nlinkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(linkage.view().draw_items_3d(&[0.5; 256])?.count() > 40);
        Ok(())
    }

    /// Golden-file regression test: converting the real pirouette motion capture
    /// clip must keep producing byte-for-byte the same `.lb.rs` source. A diff here
    /// means the converter's output changed — intentional format changes should
    /// regenerate the golden file (set `LINKAGE_BLAZE_UPDATE_BVH_GOLDEN=1`) rather
    /// than papering over an accidental behavior change.
    #[test]
    fn converts_real_bvh_to_lb_rs_matches_golden_output() {
        let bvh = read_real_pirouette_bvh();
        let source = bvh_to_lb_rs::<256, 64>(&bvh, &[]).expect("real BVH should serialize");

        if std::env::var_os("LINKAGE_BLAZE_UPDATE_BVH_GOLDEN").is_some() {
            std::fs::create_dir_all("tests/golden").expect("failed to create tests/golden");
            std::fs::write(PIROUETTE_GOLDEN_LB_RS_PATH, &source)
                .expect("failed to write golden file");
            return;
        }

        let golden = std::fs::read_to_string(PIROUETTE_GOLDEN_LB_RS_PATH).unwrap_or_else(|error| {
            panic!("failed to read `{PIROUETTE_GOLDEN_LB_RS_PATH}`: {error}")
        });
        assert_eq!(
            source, golden,
            "bvh-to-lb output for pirouette.bvh no longer matches the golden file at \
             `{PIROUETTE_GOLDEN_LB_RS_PATH}`. If this change is intentional, regenerate it with \
             `LINKAGE_BLAZE_UPDATE_BVH_GOLDEN=1 cargo test -p linkage-blaze --lib \
             converts_real_bvh_to_lb_rs_matches_golden_output`."
        );
    }
}
