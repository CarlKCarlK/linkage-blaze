//! BVH motion-capture parsing for Linkage Blaze.

use std::fmt;

use linkage_blaze_core::LinkageBuf;

/// Parsed BVH clip: hierarchy plus motion frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BvhClip {
    pub joints: Vec<BvhJoint>,
    pub frames: Vec<BvhFrame>,
    pub frame_time: f32,
    channel_count: usize,
}

/// One BVH joint or end site.
#[derive(Clone, Debug, PartialEq)]
pub struct BvhJoint {
    pub name: String,
    pub parent: Option<usize>,
    pub offset: [f32; 3],
    pub channels: Vec<BvhChannel>,
}

/// BVH channel definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BvhChannel {
    Xposition,
    Yposition,
    Zposition,
    Xrotation,
    Yrotation,
    Zrotation,
}

/// One BVH motion frame.
#[derive(Clone, Debug, PartialEq)]
pub struct BvhFrame {
    pub values: Vec<f32>,
}

/// Parameter layout discovered from a BVH clip.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BvhParameterLayout {
    pub parameters: Vec<BvhParameter>,
}

impl BvhParameterLayout {
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}

/// One Linkage parameter mapped back to a BVH channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BvhParameter {
    pub index: usize,
    pub linkage_name: &'static str,
    pub joint_index: usize,
    pub channel: BvhChannel,
}

/// Discover Linkage parameter slots from BVH channels.
pub fn discover_bvh_parameters(clip: &BvhClip) -> Result<BvhParameterLayout, MocapParseError> {
    let mut parameters = Vec::new();

    for (joint_index, joint) in clip.joints.iter().enumerate() {
        for &channel in &joint.channels {
            push_bvh_parameter(&mut parameters, joint_index, &joint.name, channel);
        }
    }

    Ok(BvhParameterLayout { parameters })
}

/// Create a parameterized LinkageBuf from a BVH clip.
///
/// `mark_joints` is a list of joint names that should get a named mark at their
/// position after their own transform is applied.  These marks persist in the
/// output linkage so callers can look up the pose of specific body parts after
/// evaluation.  Hierarchical depth marks are always emitted in addition.
pub fn build_bvh_linkage_buf<const DOF: usize, const MARKS: usize>(
    clip: &BvhClip,
    layout: &BvhParameterLayout,
    mark_joints: &[&str],
) -> Result<LinkageBuf<DOF, MARKS>, MocapParseError> {
    let defaults = clip.frames.first().map_or_else(
        || Ok(Vec::new()),
        |frame| bvh_parameter_defaults(layout, frame),
    )?;
    build_bvh_linkage_buf_with_defaults(clip, layout, &defaults, mark_joints)
}

fn build_bvh_linkage_buf_with_defaults<const DOF: usize, const MARKS: usize>(
    clip: &BvhClip,
    layout: &BvhParameterLayout,
    defaults: &[f32],
    mark_joints: &[&str],
) -> Result<LinkageBuf<DOF, MARKS>, MocapParseError> {
    if layout.len() > DOF {
        return Err(MocapParseError::new(format!(
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
        return Err(MocapParseError::new(format!(
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

fn bvh_needed_mark_count(clip: &BvhClip, children: &[Vec<usize>], mark_joints: &[&str]) -> usize {
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

fn bvh_joint_depths(clip: &BvhClip) -> Vec<usize> {
    let mut depths = vec![0usize; clip.joints.len()];
    for (joint_index, joint) in clip.joints.iter().enumerate() {
        if let Some(parent) = joint.parent {
            depths[joint_index] = depths[parent] + 1;
        }
    }
    depths
}

fn bvh_annotations(clip: &BvhClip, children: &[Vec<usize>]) -> (Vec<String>, Vec<String>) {
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
    clip: &BvhClip,
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

/// Return normalized Linkage parameter values for one BVH frame.
pub fn bvh_frame_params<const DOF: usize>(
    layout: &BvhParameterLayout,
    frame: &BvhFrame,
) -> Result<[f32; DOF], MocapParseError> {
    if layout.len() > DOF {
        return Err(MocapParseError::new(format!(
            "BVH parameter layout has {} parameter(s), but parameter array DOF is {DOF}",
            layout.len()
        )));
    }

    let defaults = bvh_parameter_defaults(layout, frame)?;
    let mut params = [0.5; DOF];
    for (parameter_index, default) in defaults.into_iter().enumerate() {
        params[parameter_index] = default;
    }

    Ok(params)
}

/// Convert BVH motion text into generated `.lb.rs` source.
///
/// The generated linkage uses defaults from the first BVH motion frame, so
/// loading the generated file starts in a captured pose. Mark names use
/// `"depth N"` slots (one per tree level), and each `.restore` line carries
/// a comment naming the joint being restored.
pub fn bvh_to_lb_rs<const DOF: usize, const MARKS: usize>(
    source: &str,
    mark_joints: &[&str],
) -> Result<String, MocapParseError> {
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

/// Parse BVH hierarchy and motion text.
pub fn parse_bvh(source: &str) -> Result<BvhClip, MocapParseError> {
    let mut parser = BvhParser::new(source);

    parser.expect("HIERARCHY")?;
    parser.expect("ROOT")?;
    let root_name = parser.next_string("root name")?;
    parser.parse_joint(root_name, None)?;
    parser.expect("MOTION")?;
    parser.expect("Frames:")?;
    let frame_count = parser.next_usize("frame count")?;
    parser.expect("Frame")?;
    parser.expect("Time:")?;
    let frame_time = parser.next_f32("frame time")?;

    let mut frames = Vec::with_capacity(frame_count);
    for frame_index in 0..frame_count {
        let mut values = Vec::with_capacity(parser.channel_count);
        for _ in 0..parser.channel_count {
            values.push(parser.next_f32("BVH frame channel value")?);
        }
        if values.len() != parser.channel_count {
            return Err(MocapParseError::new(format!(
                "BVH frame {frame_index} has {} values, expected {}",
                values.len(),
                parser.channel_count
            )));
        }
        frames.push(BvhFrame { values });
    }

    Ok(BvhClip {
        joints: parser.joints,
        frames,
        frame_time,
        channel_count: parser.channel_count,
    })
}

fn push_bvh_parameter(
    parameters: &mut Vec<BvhParameter>,
    joint_index: usize,
    joint_name: &str,
    channel: BvhChannel,
) {
    let index = parameters.len();
    let linkage_name = bvh_linkage_name(joint_name, channel);
    parameters.push(BvhParameter {
        index,
        linkage_name,
        joint_index,
        channel,
    });
}

fn bvh_linkage_name(joint_name: &str, channel: BvhChannel) -> &'static str {
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
        Box::leak(format!("depth {depth}").into_boxed_str())
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

fn bvh_channel_name(channel: BvhChannel) -> &'static str {
    match channel {
        BvhChannel::Xposition => "xposition",
        BvhChannel::Yposition => "yposition",
        BvhChannel::Zposition => "zposition",
        BvhChannel::Xrotation => "xrotation",
        BvhChannel::Yrotation => "yrotation",
        BvhChannel::Zrotation => "zrotation",
    }
}

fn apply_bvh_joint_parameters<const DOF: usize, const MARKS: usize>(
    mut linkage: LinkageBuf<DOF, MARKS>,
    layout: &BvhParameterLayout,
    joint_index: usize,
) -> LinkageBuf<DOF, MARKS> {
    for parameter in layout
        .parameters
        .iter()
        .filter(|parameter| parameter.joint_index == joint_index)
    {
        let (low, high) = bvh_parameter_range(parameter.channel);
        linkage = match parameter.channel {
            BvhChannel::Xposition => linkage.left_param(parameter.linkage_name, low, high),
            BvhChannel::Yposition => linkage.up_param(parameter.linkage_name, low, high),
            BvhChannel::Zposition => linkage.forward_param(parameter.linkage_name, low, high),
            BvhChannel::Xrotation => linkage.pitch_param(parameter.linkage_name, low, high),
            BvhChannel::Yrotation => linkage.yaw_param(parameter.linkage_name, low, high),
            BvhChannel::Zrotation => linkage.roll_param(parameter.linkage_name, low, high),
        };
    }

    linkage
}

fn bvh_parameter_defaults(
    layout: &BvhParameterLayout,
    frame: &BvhFrame,
) -> Result<Vec<f32>, MocapParseError> {
    let mut defaults = Vec::with_capacity(layout.len());

    for parameter in &layout.parameters {
        let value = frame.values.get(parameter.index).copied().ok_or_else(|| {
            MocapParseError::new(format!("BVH frame missing channel {}", parameter.index))
        })?;
        defaults.push(normalize_bvh_parameter_default(parameter, value)?);
    }

    Ok(defaults)
}

fn normalize_bvh_parameter_default(
    parameter: &BvhParameter,
    value: f32,
) -> Result<f32, MocapParseError> {
    let (low, high) = bvh_parameter_range(parameter.channel);
    let default = snap_centered_default((value - low) / (high - low));

    if !(0.0..=1.0).contains(&default) {
        return Err(MocapParseError::new(format!(
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

fn bvh_parameter_range(channel: BvhChannel) -> (f32, f32) {
    match channel {
        BvhChannel::Xposition | BvhChannel::Yposition | BvhChannel::Zposition => (-300.0, 300.0),
        BvhChannel::Xrotation | BvhChannel::Yrotation | BvhChannel::Zrotation => (-720.0, 720.0),
    }
}

fn bvh_children(clip: &BvhClip) -> Vec<Vec<usize>> {
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
    clip: &BvhClip,
    layout: &BvhParameterLayout,
    children: &[Vec<usize>],
    joint_index: usize,
    depth: usize,
    mark_joints: &[&str],
) -> Result<LinkageBuf<DOF, MARKS>, MocapParseError> {
    linkage = apply_bvh_joint_parameters(linkage, layout, joint_index);

    let joint_name = clip.joints[joint_index].name.as_str();
    if mark_joints.iter().any(|&name| name == joint_name) {
        // TODO000 review: Box::leak is used because LinkageBuf::mark requires &'static str
        // (mark_names field is [&'static str; MARKS] shared with LinkageFixed). Consider
        // whether LinkageBuf could store owned names to avoid the leak.
        let static_name: &'static str = Box::leak(joint_name.to_string().into_boxed_str());
        linkage = linkage.mark(static_name);
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
    joints: Vec<BvhJoint>,
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

    fn parse_joint(
        &mut self,
        name: String,
        parent: Option<usize>,
    ) -> Result<usize, MocapParseError> {
        let joint_index = self.joints.len();
        self.joints.push(BvhJoint {
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
                    return Err(MocapParseError::new(format!(
                        "unexpected BVH token `{token}`"
                    )));
                }
                None => return Err(MocapParseError::new("unexpected end of BVH hierarchy")),
            }
        }
    }

    fn parse_end_site(&mut self, parent: usize) -> Result<usize, MocapParseError> {
        let end_index = self.joints.len();
        let name = format!("{}_end_{}", self.joints[parent].name, end_index);
        self.joints.push(BvhJoint {
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

    fn expect(&mut self, expected: &str) -> Result<(), MocapParseError> {
        let token = self.next_string(expected)?;
        if token != expected {
            return Err(MocapParseError::new(format!(
                "expected BVH token `{expected}`, got `{token}`"
            )));
        }

        Ok(())
    }

    fn next_channel(&mut self) -> Result<BvhChannel, MocapParseError> {
        let token = self.next_string("BVH channel")?;
        match token.as_str() {
            "Xposition" => Ok(BvhChannel::Xposition),
            "Yposition" => Ok(BvhChannel::Yposition),
            "Zposition" => Ok(BvhChannel::Zposition),
            "Xrotation" => Ok(BvhChannel::Xrotation),
            "Yrotation" => Ok(BvhChannel::Yrotation),
            "Zrotation" => Ok(BvhChannel::Zrotation),
            _ => Err(MocapParseError::new(format!(
                "unknown BVH channel `{token}`"
            ))),
        }
    }

    fn next_f32(&mut self, field_name: &str) -> Result<f32, MocapParseError> {
        let token = self.next_string(field_name)?;
        token
            .parse::<f32>()
            .map_err(|_| MocapParseError::new(format!("expected f32 {field_name}, got `{token}`")))
    }

    fn next_usize(&mut self, field_name: &str) -> Result<usize, MocapParseError> {
        let token = self.next_string(field_name)?;
        token.parse::<usize>().map_err(|_| {
            MocapParseError::new(format!("expected integer {field_name}, got `{token}`"))
        })
    }

    fn next_string(&mut self, field_name: &str) -> Result<String, MocapParseError> {
        let token = self
            .tokens
            .get(self.index)
            .ok_or_else(|| MocapParseError::new(format!("missing {field_name}")))?;
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

/// BVH parser error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MocapParseError {
    line_number: Option<usize>,
    message: String,
}

impl MocapParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            line_number: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for MocapParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line_number) = self.line_number {
            write!(formatter, "line {line_number}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for MocapParseError {}

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
        assert_eq!(clip.frames.len(), 2);
        assert_eq!(clip.channel_count, 15);
        assert_eq!(clip.joints[0].name, "hip");
        assert_eq!(clip.joints[2].name, "leftArm");
        assert_eq!(clip.joints[3].parent, Some(2));
        assert_eq!(clip.frames[1].values[14], 120.0);
    }

    #[test]
    fn builds_bvh_linkage_buf_and_frame_params() {
        let clip = parse_bvh(BVH).expect("BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("BVH layout should parse");
        let linkage =
            build_bvh_linkage_buf::<32, 8>(&clip, &layout, &[]).expect("BVH linkage should build");
        let params = bvh_frame_params::<32>(&layout, &clip.frames[1]).expect("params should build");

        assert_eq!(layout.len(), 15);
        assert_eq!(params[0], 0.5);
        assert_eq!(params[1], 0.5);
        assert_eq!(params[2], 0.5);
        assert!(params[6] > 0.5);
        assert!(linkage.view().draw_items(&params).count() >= 5);
    }

    #[test]
    fn converts_bvh_to_lb_rs_source() {
        let source = bvh_to_lb_rs::<32, 8>(BVH, &[]).expect("BVH should serialize");
        let linkage =
            LinkageBuf::<32, 8>::from_lb_rs(&source).expect("generated source should parse");

        assert!(source.starts_with("linkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(source.contains(".define_param(\"hip_xposition\""));
        assert!(source.contains(".define_param(\"chest_zrotation\""));
        assert!(
            !source.contains(".mark(\"depth 0\""),
            "single-child hip should not be marked"
        );
        assert!(source.contains(".mark(\"depth 1\") // chest"));
        assert!(source.contains(".restore(\"depth 1\") // chest"));
        assert!(linkage.view().draw_items(&[0.5; 32]).count() >= 5);
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
            bvh_linkage_name("rThumb1", BvhChannel::Zrotation),
            "r_thumb1_zrotation"
        );
        assert_eq!(
            bvh_linkage_name("leftEye", BvhChannel::Xposition),
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
    fn bvh_rotation_axes_are_remapped_to_linkage_axes() {
        let clip = parse_bvh(BVH_X_ROTATION).expect("BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("BVH layout should parse");
        let linkage =
            build_bvh_linkage_buf::<1, 4>(&clip, &layout, &[]).expect("BVH linkage should build");
        let params = bvh_frame_params::<1>(&layout, &clip.frames[0]).expect("params should build");
        let stroke = linkage
            .view()
            .draw_items(&params)
            .find_map(|draw_item| match draw_item {
                linkage_blaze_core::DrawItem::Stroke(stroke) => Some(stroke),
                _ => None,
            })
            .expect("offset should draw a stroke");

        assert!(
            stroke
                .end()
                .position()
                .is_close_to(&linkage_blaze_core::Vec3::from([10.0, 0.0, 0.0]), 1e-4)
        );
    }

    #[test]
    fn builds_real_bvh_linkage_when_present() {
        let Ok(bvh) = std::fs::read_to_string("samples/pirouette.bvh") else {
            return;
        };

        let clip = parse_bvh(&bvh).expect("real BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("real BVH layout should parse");
        let linkage = build_bvh_linkage_buf::<256, 64>(&clip, &layout, &[])
            .expect("real BVH linkage should build");
        let params =
            bvh_frame_params::<256>(&layout, &clip.frames[0]).expect("real params should build");

        assert!(clip.joints.len() > 40);
        assert!(clip.frames.len() > 500);
        assert!(layout.len() > 120);
        assert!(linkage.view().draw_items(&params).count() > 40);
    }

    #[test]
    fn converts_real_bvh_to_lb_rs_when_present() {
        let Ok(bvh) = std::fs::read_to_string("samples/pirouette.bvh") else {
            return;
        };

        let source = bvh_to_lb_rs::<256, 64>(&bvh, &[]).expect("real BVH should serialize");
        let linkage =
            LinkageBuf::<256, 64>::from_lb_rs(&source).expect("real generated source should parse");

        assert!(source.starts_with("linkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(linkage.view().draw_items(&[0.5; 256]).count() > 40);
    }
}
