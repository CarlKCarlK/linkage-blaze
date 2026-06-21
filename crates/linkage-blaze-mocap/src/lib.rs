//! CMU ASF/AMC motion-capture parsing for Linkage Blaze.
//!
//! ASF describes a skeleton: hierarchy, bone lengths, axes, and degrees of
//! freedom. AMC describes per-frame joint parameters for that skeleton.

use std::fmt;
use std::str::FromStr;

use linkage_blaze_core::LinkageBuf;

/// Parsed CMU ASF skeleton data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AsfSkeleton {
    pub version: Option<String>,
    pub name: Option<String>,
    pub root: AsfRoot,
    pub bones: Vec<AsfBone>,
    pub hierarchy: Vec<HierarchyEdge>,
}

impl AsfSkeleton {
    /// Return a bone by ASF name.
    pub fn bone(&self, name: &str) -> Option<&AsfBone> {
        self.bones.iter().find(|bone| bone.name == name)
    }

    /// Build a simple static stick-figure linkage from the ASF hierarchy.
    ///
    /// This is an MVP skeleton view. It uses bone directions and lengths, but
    /// does not yet apply ASF axis rotations or AMC frame parameters.
    pub fn static_linkage(&self) -> LinkageBuf<0> {
        let mut linkage = LinkageBuf::start().pen_up().mark("root");
        for edge in &self.hierarchy {
            if edge.parent == "root" {
                linkage = linkage.restore("root");
            } else if let Some(parent_index) = self.bone_index(&edge.parent) {
                let Some(parent_mark_name) = static_mark_name(parent_index) else {
                    continue;
                };
                linkage = linkage.restore(parent_mark_name);
            }

            if let Some(child_bone) = self.bone(&edge.child) {
                let Some(child_index) = self.bone_index(&edge.child) else {
                    continue;
                };
                let Some(child_mark_name) = static_mark_name(child_index) else {
                    continue;
                };
                linkage = append_bone_segment(linkage, child_bone, child_mark_name);
            }
        }
        linkage
    }

    fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|bone| bone.name == name)
    }
}

/// ASF root channel definition.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AsfRoot {
    pub order: Vec<Dof>,
    pub axis_order: Option<String>,
    pub position: [f32; 3],
    pub orientation: [f32; 3],
}

/// One ASF bone definition.
#[derive(Clone, Debug, PartialEq)]
pub struct AsfBone {
    pub id: Option<u32>,
    pub name: String,
    pub direction: [f32; 3],
    pub length: f32,
    pub axis: [f32; 3],
    pub axis_order: Option<String>,
    pub dof: Vec<Dof>,
}

impl Default for AsfBone {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            direction: [0.0, 0.0, 0.0],
            length: 0.0,
            axis: [0.0, 0.0, 0.0],
            axis_order: None,
            dof: Vec::new(),
        }
    }
}

/// Parent-child ASF hierarchy relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HierarchyEdge {
    pub parent: String,
    pub child: String,
}

/// ASF joint degree of freedom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dof {
    Rx,
    Ry,
    Rz,
    Tx,
    Ty,
    Tz,
    L,
}

impl FromStr for Dof {
    type Err = MocapParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "rx" => Ok(Self::Rx),
            "ry" => Ok(Self::Ry),
            "rz" => Ok(Self::Rz),
            "tx" => Ok(Self::Tx),
            "ty" => Ok(Self::Ty),
            "tz" => Ok(Self::Tz),
            "l" => Ok(Self::L),
            _ => Err(MocapParseError::new(format!("unknown DOF `{value}`"))),
        }
    }
}

/// Parsed CMU AMC motion data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmcMotion {
    pub frames: Vec<AmcFrame>,
}

/// One AMC frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AmcFrame {
    pub index: u32,
    pub joints: Vec<AmcJointFrame>,
}

/// Joint parameters for one AMC frame.
#[derive(Clone, Debug, PartialEq)]
pub struct AmcJointFrame {
    pub name: String,
    pub values: Vec<f32>,
}

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

/// Parameter layout discovered from an ASF skeleton.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AsfParameterLayout {
    pub parameters: Vec<AsfParameter>,
}

impl AsfParameterLayout {
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}

/// One Linkage parameter mapped back to an ASF joint channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsfParameter {
    pub index: usize,
    pub linkage_name: &'static str,
    pub joint_name: String,
    pub dof: Dof,
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

/// First ASF pass: discover AMC/Linkage parameter slots from root order and bone DOFs.
pub fn discover_asf_parameters(source: &str) -> Result<AsfParameterLayout, MocapParseError> {
    let skeleton = parse_asf(source)?;
    let mut parameters = Vec::new();

    for &dof in &skeleton.root.order {
        push_parameter(&mut parameters, "root", dof)?;
    }

    for bone in &skeleton.bones {
        for &dof in &bone.dof {
            push_parameter(&mut parameters, &bone.name, dof)?;
        }
    }

    Ok(AsfParameterLayout { parameters })
}

/// Later ASF pass: create a parameterized LinkageBuf from the skeleton.
///
/// `DOF` must be at least `layout.len()`. Parameter names are stable internal
/// names from [`AsfParameter::linkage_name`], in ASF/AMC channel order.
pub fn build_asf_linkage_buf<const DOF: usize>(
    source: &str,
    layout: &AsfParameterLayout,
) -> Result<LinkageBuf<DOF>, MocapParseError> {
    build_asf_linkage_buf_with_defaults(source, layout, &[])
}

fn build_asf_linkage_buf_with_defaults<const DOF: usize>(
    source: &str,
    layout: &AsfParameterLayout,
    defaults: &[f32],
) -> Result<LinkageBuf<DOF>, MocapParseError> {
    if layout.len() > DOF {
        return Err(MocapParseError::new(format!(
            "ASF parameter layout has {} parameter(s), but LinkageBuf DOF is {DOF}",
            layout.len()
        )));
    }

    let skeleton = parse_asf(source)?;
    let mut linkage = LinkageBuf::start().pen_up().mark("root");
    for (parameter_index, parameter) in layout.parameters.iter().enumerate() {
        let default = defaults.get(parameter_index).copied().unwrap_or(0.5);
        linkage = linkage.define_param(parameter.linkage_name, default);
    }

    for edge in &skeleton.hierarchy {
        if edge.parent == "root" {
            linkage = linkage.restore("root");
            linkage = apply_fixed_axis(
                linkage,
                skeleton.root.axis_order.as_deref(),
                skeleton.root.orientation,
            );
            linkage = apply_joint_parameters(linkage, layout, "root");
        } else if let Some(parent_index) = skeleton.bone_index(&edge.parent) {
            let Some(parent_mark_name) = static_mark_name(parent_index) else {
                continue;
            };
            linkage = linkage.restore(parent_mark_name);
        }

        if let Some(child_bone) = skeleton.bone(&edge.child) {
            let Some(child_index) = skeleton.bone_index(&edge.child) else {
                continue;
            };
            let Some(child_mark_name) = static_mark_name(child_index) else {
                continue;
            };
            linkage = apply_fixed_axis(linkage, child_bone.axis_order.as_deref(), child_bone.axis);
            linkage = apply_joint_parameters(linkage, layout, &child_bone.name);
            linkage = apply_inverse_fixed_axis(
                linkage,
                child_bone.axis_order.as_deref(),
                child_bone.axis,
            );
            linkage = append_bone_segment(linkage, child_bone, child_mark_name);
        }
    }

    Ok(linkage)
}

/// Convert ASF skeleton text into generated `.lb.rs` source.
///
/// This performs the intended multi-pass flow: first discover parameters, then
/// reread the ASF to build a parameterized linkage, then serialize it as
/// `linkage![ ... ]` source.
pub fn asf_to_lb_rs<const DOF: usize>(source: &str) -> Result<String, MocapParseError> {
    let layout = discover_asf_parameters(source)?;
    let linkage = build_asf_linkage_buf::<DOF>(source, &layout)?;
    Ok(linkage.view().to_lb_rs())
}

/// Convert ASF skeleton text and AMC motion text into generated `.lb.rs` source.
///
/// The ASF file supplies the skeleton and parameter layout. The AMC file
/// supplies parameter defaults from `frame_index`, so loading the generated
/// file starts in a real captured pose rather than the zero/rest pose.
pub fn asf_and_amc_to_lb_rs<const DOF: usize>(
    asf_source: &str,
    amc_source: &str,
    frame_index: u32,
) -> Result<String, MocapParseError> {
    let layout = discover_asf_parameters(asf_source)?;
    let motion = parse_amc(amc_source)?;
    let frame = motion
        .frames
        .iter()
        .find(|frame| frame.index == frame_index)
        .ok_or_else(|| MocapParseError::new(format!("AMC frame {frame_index} not found")))?;
    let defaults = parameter_defaults_from_frame(&layout, frame)?;
    let linkage = build_asf_linkage_buf_with_defaults::<DOF>(asf_source, &layout, &defaults)?;

    Ok(linkage.view().to_lb_rs())
}

/// Return normalized Linkage parameter values for one AMC frame.
///
/// `DOF` must be at least `layout.len()`. Unused trailing values are set to
/// `0.5`.
pub fn amc_frame_params<const DOF: usize>(
    layout: &AsfParameterLayout,
    frame: &AmcFrame,
) -> Result<[f32; DOF], MocapParseError> {
    if layout.len() > DOF {
        return Err(MocapParseError::new(format!(
            "ASF parameter layout has {} parameter(s), but parameter array DOF is {DOF}",
            layout.len()
        )));
    }

    let defaults = parameter_defaults_from_frame(layout, frame)?;
    let mut params = [0.5; DOF];
    for (parameter_index, default) in defaults.into_iter().enumerate() {
        params[parameter_index] = default;
    }

    Ok(params)
}

/// Discover Linkage parameter slots from BVH channels.
pub fn discover_bvh_parameters(clip: &BvhClip) -> Result<BvhParameterLayout, MocapParseError> {
    let mut parameters = Vec::new();

    for (joint_index, joint) in clip.joints.iter().enumerate() {
        for &channel in &joint.channels {
            push_bvh_parameter(&mut parameters, joint_index, channel)?;
        }
    }

    Ok(BvhParameterLayout { parameters })
}

/// Create a parameterized LinkageBuf from a BVH clip.
pub fn build_bvh_linkage_buf<const DOF: usize>(
    clip: &BvhClip,
    layout: &BvhParameterLayout,
) -> Result<LinkageBuf<DOF>, MocapParseError> {
    let defaults = clip.frames.first().map_or_else(
        || Ok(Vec::new()),
        |frame| bvh_parameter_defaults(layout, frame),
    )?;
    build_bvh_linkage_buf_with_defaults(clip, layout, &defaults)
}

fn build_bvh_linkage_buf_with_defaults<const DOF: usize>(
    clip: &BvhClip,
    layout: &BvhParameterLayout,
    defaults: &[f32],
) -> Result<LinkageBuf<DOF>, MocapParseError> {
    if layout.len() > DOF {
        return Err(MocapParseError::new(format!(
            "BVH parameter layout has {} parameter(s), but LinkageBuf DOF is {DOF}",
            layout.len()
        )));
    }

    let mut linkage = LinkageBuf::start().pen_up().mark("origin");
    for (parameter_index, parameter) in layout.parameters.iter().enumerate() {
        let default = defaults.get(parameter_index).copied().unwrap_or(0.5);
        linkage = linkage.define_param(parameter.linkage_name, default);
    }

    let children = bvh_children(clip);
    let marked_joint_count = children
        .iter()
        .filter(|joint_children| !joint_children.is_empty())
        .count();
    if marked_joint_count >= 64 {
        return Err(MocapParseError::new(format!(
            "BVH needs {marked_joint_count} marked joints, but LinkageBuf views currently support 63"
        )));
    }

    for (joint_index, joint) in clip.joints.iter().enumerate() {
        if joint.parent.is_none() {
            linkage = linkage.restore("origin");
            linkage = append_bvh_joint(linkage, clip, layout, &children, joint_index)?;
        }
    }

    Ok(linkage)
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

/// Parse CMU ASF skeleton text.
pub fn parse_asf(source: &str) -> Result<AsfSkeleton, MocapParseError> {
    let mut skeleton = AsfSkeleton::default();
    let mut lines = source.lines().enumerate().peekable();

    while let Some((line_index, line)) = lines.next() {
        let line = clean_line(line);
        if line.is_empty() {
            continue;
        }

        if let Some(value) = line.strip_prefix(":version") {
            skeleton.version = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix(":name") {
            skeleton.name = Some(value.trim().to_string());
        } else if line == ":root" {
            parse_root(&mut lines, &mut skeleton)?;
        } else if line == ":bonedata" {
            parse_bonedata(&mut lines, &mut skeleton)?;
        } else if line == ":hierarchy" {
            parse_hierarchy(&mut lines, &mut skeleton)?;
        } else if line.starts_with(':') {
            skip_asf_section(&mut lines);
            continue;
        } else {
            return Err(MocapParseError::at(
                line_index + 1,
                format!("unexpected ASF line `{line}`"),
            ));
        }
    }

    Ok(skeleton)
}

/// Parse CMU AMC motion text.
pub fn parse_amc(source: &str) -> Result<AmcMotion, MocapParseError> {
    let mut motion = AmcMotion::default();
    let mut current_frame: Option<AmcFrame> = None;

    for (line_index, line) in source.lines().enumerate() {
        let line = clean_line(line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if let Ok(index) = line.parse::<u32>() {
            if let Some(frame) = current_frame.take() {
                motion.frames.push(frame);
            }
            current_frame = Some(AmcFrame {
                index,
                joints: Vec::new(),
            });
            continue;
        }

        let frame = current_frame.as_mut().ok_or_else(|| {
            MocapParseError::at(
                line_index + 1,
                "AMC joint line appeared before a frame index",
            )
        })?;
        let mut parts = line.split_whitespace();
        let name = parts
            .next()
            .ok_or_else(|| MocapParseError::at(line_index + 1, "missing AMC joint name"))?;
        let mut values = Vec::new();
        for part in parts {
            values.push(parse_f32(line_index + 1, part)?);
        }
        frame.joints.push(AmcJointFrame {
            name: name.to_string(),
            values,
        });
    }

    if let Some(frame) = current_frame {
        motion.frames.push(frame);
    }

    Ok(motion)
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

fn push_parameter(
    parameters: &mut Vec<AsfParameter>,
    joint_name: &str,
    dof: Dof,
) -> Result<(), MocapParseError> {
    let index = parameters.len();
    let Some(linkage_name) = static_parameter_name(index) else {
        return Err(MocapParseError::new(format!(
            "too many ASF parameters; supported maximum is {}",
            STATIC_PARAMETER_NAMES.len()
        )));
    };
    parameters.push(AsfParameter {
        index,
        linkage_name,
        joint_name: joint_name.to_string(),
        dof,
    });
    Ok(())
}

fn push_bvh_parameter(
    parameters: &mut Vec<BvhParameter>,
    joint_index: usize,
    channel: BvhChannel,
) -> Result<(), MocapParseError> {
    let index = parameters.len();
    let Some(linkage_name) = static_parameter_name(index) else {
        return Err(MocapParseError::new(format!(
            "too many BVH parameters; supported maximum is {}",
            STATIC_PARAMETER_NAMES.len()
        )));
    };
    parameters.push(BvhParameter {
        index,
        linkage_name,
        joint_index,
        channel,
    });
    Ok(())
}

fn apply_joint_parameters<const DOF: usize>(
    mut linkage: LinkageBuf<DOF>,
    layout: &AsfParameterLayout,
    joint_name: &str,
) -> LinkageBuf<DOF> {
    for parameter in layout
        .parameters
        .iter()
        .filter(|parameter| parameter.joint_name == joint_name)
    {
        let (low, high) = parameter_range(parameter.dof);
        linkage = match parameter.dof {
            Dof::Rx => linkage.roll_param(parameter.linkage_name, low, high),
            Dof::Ry => linkage.pitch_param(parameter.linkage_name, low, high),
            Dof::Rz => linkage.yaw_param(parameter.linkage_name, low, high),
            Dof::Tx => linkage.forward_param(parameter.linkage_name, low, high),
            Dof::Ty => linkage.left_param(parameter.linkage_name, low, high),
            Dof::Tz => linkage.up_param(parameter.linkage_name, low, high),
            Dof::L => linkage.forward_param(parameter.linkage_name, low, high),
        };
    }
    linkage
}

fn apply_bvh_joint_parameters<const DOF: usize>(
    mut linkage: LinkageBuf<DOF>,
    layout: &BvhParameterLayout,
    joint_index: usize,
) -> LinkageBuf<DOF> {
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

fn apply_fixed_axis<const DOF: usize>(
    mut linkage: LinkageBuf<DOF>,
    axis_order: Option<&str>,
    axis: [f32; 3],
) -> LinkageBuf<DOF> {
    for axis_name in axis_order.unwrap_or("XYZ").chars() {
        linkage = apply_fixed_axis_rotation(linkage, axis_name, axis);
    }

    linkage
}

fn apply_inverse_fixed_axis<const DOF: usize>(
    mut linkage: LinkageBuf<DOF>,
    axis_order: Option<&str>,
    axis: [f32; 3],
) -> LinkageBuf<DOF> {
    for axis_name in axis_order.unwrap_or("XYZ").chars().rev() {
        linkage = apply_fixed_axis_rotation(linkage, axis_name, [-axis[0], -axis[1], -axis[2]]);
    }

    linkage
}

fn apply_fixed_axis_rotation<const DOF: usize>(
    linkage: LinkageBuf<DOF>,
    axis_name: char,
    axis: [f32; 3],
) -> LinkageBuf<DOF> {
    match axis_name {
        'X' | 'x' if !is_nearly_zero_degrees(axis[0]) => linkage.roll(axis[0]),
        'Y' | 'y' if !is_nearly_zero_degrees(axis[1]) => linkage.pitch(axis[1]),
        'Z' | 'z' if !is_nearly_zero_degrees(axis[2]) => linkage.yaw(axis[2]),
        _ => linkage,
    }
}

fn parameter_defaults_from_frame(
    layout: &AsfParameterLayout,
    frame: &AmcFrame,
) -> Result<Vec<f32>, MocapParseError> {
    let mut defaults = Vec::with_capacity(layout.len());

    for (parameter_index, parameter) in layout.parameters.iter().enumerate() {
        let joint_value_index = layout.parameters[..parameter_index]
            .iter()
            .filter(|candidate| candidate.joint_name == parameter.joint_name)
            .count();
        let joint_frame = frame
            .joints
            .iter()
            .find(|joint_frame| joint_frame.name == parameter.joint_name)
            .ok_or_else(|| {
                MocapParseError::new(format!(
                    "AMC frame {} missing joint `{}`",
                    frame.index, parameter.joint_name
                ))
            })?;
        let value = joint_frame
            .values
            .get(joint_value_index)
            .copied()
            .ok_or_else(|| {
                MocapParseError::new(format!(
                    "AMC frame {} joint `{}` missing value {}",
                    frame.index, parameter.joint_name, joint_value_index
                ))
            })?;

        defaults.push(normalize_parameter_default(parameter, value)?);
    }

    Ok(defaults)
}

fn normalize_parameter_default(
    parameter: &AsfParameter,
    value: f32,
) -> Result<f32, MocapParseError> {
    let (low, high) = parameter_range(parameter.dof);
    let default = (value - low) / (high - low);

    if !(0.0..=1.0).contains(&default) {
        return Err(MocapParseError::new(format!(
            "AMC value {value} for {} {:?} is outside [{low}, {high}]",
            parameter.joint_name, parameter.dof
        )));
    }

    Ok(default)
}

fn parameter_range(dof: Dof) -> (f32, f32) {
    match dof {
        Dof::Rx | Dof::Ry | Dof::Rz => (-180.0, 180.0),
        Dof::Tx | Dof::Ty | Dof::Tz => (-100.0, 100.0),
        Dof::L => (0.0, 100.0),
    }
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
    let default = (value - low) / (high - low);

    if !(0.0..=1.0).contains(&default) {
        return Err(MocapParseError::new(format!(
            "BVH value {value} for channel {:?} is outside [{low}, {high}]",
            parameter.channel
        )));
    }

    Ok(default)
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

fn append_bvh_joint<const DOF: usize>(
    mut linkage: LinkageBuf<DOF>,
    clip: &BvhClip,
    layout: &BvhParameterLayout,
    children: &[Vec<usize>],
    joint_index: usize,
) -> Result<LinkageBuf<DOF>, MocapParseError> {
    linkage = apply_bvh_joint_parameters(linkage, layout, joint_index);

    if children[joint_index].is_empty() {
        return Ok(linkage);
    }

    let mark_name = static_mark_name(joint_index)
        .ok_or_else(|| MocapParseError::new(format!("BVH joint {joint_index} cannot be marked")))?;
    linkage = linkage.mark(mark_name);

    for &child_index in &children[joint_index] {
        linkage = linkage.restore(mark_name);
        linkage = append_offset_segment(linkage, clip.joints[child_index].offset);
        linkage = append_bvh_joint(linkage, clip, layout, children, child_index)?;
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

fn append_bone_segment<const DOF: usize>(
    mut linkage: LinkageBuf<DOF>,
    bone: &AsfBone,
    mark_name: &'static str,
) -> LinkageBuf<DOF> {
    let [asf_x, asf_y, asf_z] = bone.direction;
    let direction_x = asf_z;
    let direction_y = asf_x;
    let direction_z = asf_y;
    let horizontal_length = direction_x.hypot(direction_y);
    let yaw_degrees = direction_y.atan2(direction_x).to_degrees();
    let pitch_degrees = -direction_z.atan2(horizontal_length).to_degrees();

    if !is_nearly_zero_degrees(yaw_degrees) {
        linkage = linkage.yaw(yaw_degrees);
    }
    if !is_nearly_zero_degrees(pitch_degrees) {
        linkage = linkage.pitch(pitch_degrees);
    }

    linkage = linkage.pen_down().forward(bone.length).pen_up();

    if !is_nearly_zero_degrees(pitch_degrees) {
        linkage = linkage.pitch(-pitch_degrees);
    }
    if !is_nearly_zero_degrees(yaw_degrees) {
        linkage = linkage.yaw(-yaw_degrees);
    }

    linkage.mark(mark_name)
}

fn append_offset_segment<const DOF: usize>(
    mut linkage: LinkageBuf<DOF>,
    offset: [f32; 3],
) -> LinkageBuf<DOF> {
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

    linkage = linkage.pen_width(1.5).pen_down().forward(length).pen_up();

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

fn parse_root<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    skeleton: &mut AsfSkeleton,
) -> Result<(), MocapParseError>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    while let Some((line_index, line)) = lines.peek().copied() {
        let line = clean_line(line);
        if line.is_empty() {
            lines.next();
            continue;
        }
        if line.starts_with(':') {
            return Ok(());
        }
        lines.next();

        let mut parts = line.split_whitespace();
        let key = parts
            .next()
            .ok_or_else(|| MocapParseError::at(line_index + 1, "missing root key"))?;
        match key {
            "order" => {
                skeleton.root.order.clear();
                for value in parts {
                    skeleton.root.order.push(value.parse()?);
                }
            }
            "axis" => skeleton.root.axis_order = parts.next().map(str::to_string),
            "position" => {
                skeleton.root.position = parse_vec3(line_index + 1, &mut parts, "position")?;
            }
            "orientation" => {
                skeleton.root.orientation = parse_vec3(line_index + 1, &mut parts, "orientation")?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_bonedata<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    skeleton: &mut AsfSkeleton,
) -> Result<(), MocapParseError>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    while let Some((line_index, line)) = lines.peek().copied() {
        let line = clean_line(line);
        if line.is_empty() {
            lines.next();
            continue;
        }
        if line.starts_with(':') {
            return Ok(());
        }
        lines.next();
        if line != "begin" {
            return Err(MocapParseError::at(
                line_index + 1,
                format!("expected `begin`, got `{line}`"),
            ));
        }

        skeleton.bones.push(parse_bone(lines)?);
    }
    Ok(())
}

fn parse_bone<'a, I>(lines: &mut std::iter::Peekable<I>) -> Result<AsfBone, MocapParseError>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    let mut bone = AsfBone::default();

    for (line_index, line) in lines.by_ref() {
        let line = clean_line(line);
        if line.is_empty() {
            continue;
        }
        if line == "end" {
            if bone.name.is_empty() {
                return Err(MocapParseError::at(line_index + 1, "bone missing name"));
            }
            return Ok(bone);
        }

        let mut parts = line.split_whitespace();
        let key = parts
            .next()
            .ok_or_else(|| MocapParseError::at(line_index + 1, "missing bone key"))?;

        match key {
            "id" => {
                let value = require_part(line_index + 1, &mut parts, "bone id")?;
                bone.id = Some(parse_u32(line_index + 1, value)?);
            }
            "name" => {
                bone.name = require_part(line_index + 1, &mut parts, "bone name")?.to_string();
            }
            "direction" => {
                bone.direction = parse_vec3(line_index + 1, &mut parts, "direction")?;
            }
            "length" => {
                let value = require_part(line_index + 1, &mut parts, "bone length")?;
                bone.length = parse_f32(line_index + 1, value)?;
            }
            "axis" => {
                bone.axis = parse_vec3(line_index + 1, &mut parts, "axis")?;
                bone.axis_order = parts.next().map(str::to_string);
            }
            "dof" => {
                bone.dof.clear();
                for value in parts {
                    bone.dof.push(value.parse()?);
                }
            }
            "limits" => {}
            _ => {}
        }
    }

    Err(MocapParseError::new("unterminated ASF bone"))
}

fn parse_hierarchy<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    skeleton: &mut AsfSkeleton,
) -> Result<(), MocapParseError>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    let mut inside = false;
    for (line_index, line) in lines.by_ref() {
        let line = clean_line(line);
        if line.is_empty() {
            continue;
        }
        if line == "begin" {
            inside = true;
            continue;
        }
        if line == "end" {
            return Ok(());
        }
        if !inside {
            return Err(MocapParseError::at(
                line_index + 1,
                format!("expected hierarchy `begin`, got `{line}`"),
            ));
        }

        let mut parts = line.split_whitespace();
        let parent = require_part(line_index + 1, &mut parts, "hierarchy parent")?;
        for child in parts {
            skeleton.hierarchy.push(HierarchyEdge {
                parent: parent.to_string(),
                child: child.to_string(),
            });
        }
    }

    Err(MocapParseError::new("unterminated ASF hierarchy"))
}

fn skip_asf_section<'a, I>(lines: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = (usize, &'a str)>,
{
    while let Some((_, line)) = lines.peek() {
        let line = clean_line(line);
        if line.starts_with(':') {
            return;
        }
        lines.next();
    }
}

fn clean_line(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}

fn parse_vec3<'a>(
    line_number: usize,
    parts: &mut impl Iterator<Item = &'a str>,
    field_name: &str,
) -> Result<[f32; 3], MocapParseError> {
    Ok([
        parse_f32(line_number, require_part(line_number, parts, field_name)?)?,
        parse_f32(line_number, require_part(line_number, parts, field_name)?)?,
        parse_f32(line_number, require_part(line_number, parts, field_name)?)?,
    ])
}

fn require_part<'a>(
    line_number: usize,
    parts: &mut impl Iterator<Item = &'a str>,
    field_name: &str,
) -> Result<&'a str, MocapParseError> {
    parts
        .next()
        .ok_or_else(|| MocapParseError::at(line_number, format!("missing {field_name}")))
}

fn parse_f32(line_number: usize, value: &str) -> Result<f32, MocapParseError> {
    value
        .parse::<f32>()
        .map_err(|_| MocapParseError::at(line_number, format!("expected f32 value, got `{value}`")))
}

fn parse_u32(line_number: usize, value: &str) -> Result<u32, MocapParseError> {
    value
        .parse::<u32>()
        .map_err(|_| MocapParseError::at(line_number, format!("expected u32 value, got `{value}`")))
}

const STATIC_PARAMETER_NAMES: [&str; 256] = [
    "mocap_000",
    "mocap_001",
    "mocap_002",
    "mocap_003",
    "mocap_004",
    "mocap_005",
    "mocap_006",
    "mocap_007",
    "mocap_008",
    "mocap_009",
    "mocap_010",
    "mocap_011",
    "mocap_012",
    "mocap_013",
    "mocap_014",
    "mocap_015",
    "mocap_016",
    "mocap_017",
    "mocap_018",
    "mocap_019",
    "mocap_020",
    "mocap_021",
    "mocap_022",
    "mocap_023",
    "mocap_024",
    "mocap_025",
    "mocap_026",
    "mocap_027",
    "mocap_028",
    "mocap_029",
    "mocap_030",
    "mocap_031",
    "mocap_032",
    "mocap_033",
    "mocap_034",
    "mocap_035",
    "mocap_036",
    "mocap_037",
    "mocap_038",
    "mocap_039",
    "mocap_040",
    "mocap_041",
    "mocap_042",
    "mocap_043",
    "mocap_044",
    "mocap_045",
    "mocap_046",
    "mocap_047",
    "mocap_048",
    "mocap_049",
    "mocap_050",
    "mocap_051",
    "mocap_052",
    "mocap_053",
    "mocap_054",
    "mocap_055",
    "mocap_056",
    "mocap_057",
    "mocap_058",
    "mocap_059",
    "mocap_060",
    "mocap_061",
    "mocap_062",
    "mocap_063",
    "mocap_064",
    "mocap_065",
    "mocap_066",
    "mocap_067",
    "mocap_068",
    "mocap_069",
    "mocap_070",
    "mocap_071",
    "mocap_072",
    "mocap_073",
    "mocap_074",
    "mocap_075",
    "mocap_076",
    "mocap_077",
    "mocap_078",
    "mocap_079",
    "mocap_080",
    "mocap_081",
    "mocap_082",
    "mocap_083",
    "mocap_084",
    "mocap_085",
    "mocap_086",
    "mocap_087",
    "mocap_088",
    "mocap_089",
    "mocap_090",
    "mocap_091",
    "mocap_092",
    "mocap_093",
    "mocap_094",
    "mocap_095",
    "mocap_096",
    "mocap_097",
    "mocap_098",
    "mocap_099",
    "mocap_100",
    "mocap_101",
    "mocap_102",
    "mocap_103",
    "mocap_104",
    "mocap_105",
    "mocap_106",
    "mocap_107",
    "mocap_108",
    "mocap_109",
    "mocap_110",
    "mocap_111",
    "mocap_112",
    "mocap_113",
    "mocap_114",
    "mocap_115",
    "mocap_116",
    "mocap_117",
    "mocap_118",
    "mocap_119",
    "mocap_120",
    "mocap_121",
    "mocap_122",
    "mocap_123",
    "mocap_124",
    "mocap_125",
    "mocap_126",
    "mocap_127",
    "mocap_128",
    "mocap_129",
    "mocap_130",
    "mocap_131",
    "mocap_132",
    "mocap_133",
    "mocap_134",
    "mocap_135",
    "mocap_136",
    "mocap_137",
    "mocap_138",
    "mocap_139",
    "mocap_140",
    "mocap_141",
    "mocap_142",
    "mocap_143",
    "mocap_144",
    "mocap_145",
    "mocap_146",
    "mocap_147",
    "mocap_148",
    "mocap_149",
    "mocap_150",
    "mocap_151",
    "mocap_152",
    "mocap_153",
    "mocap_154",
    "mocap_155",
    "mocap_156",
    "mocap_157",
    "mocap_158",
    "mocap_159",
    "mocap_160",
    "mocap_161",
    "mocap_162",
    "mocap_163",
    "mocap_164",
    "mocap_165",
    "mocap_166",
    "mocap_167",
    "mocap_168",
    "mocap_169",
    "mocap_170",
    "mocap_171",
    "mocap_172",
    "mocap_173",
    "mocap_174",
    "mocap_175",
    "mocap_176",
    "mocap_177",
    "mocap_178",
    "mocap_179",
    "mocap_180",
    "mocap_181",
    "mocap_182",
    "mocap_183",
    "mocap_184",
    "mocap_185",
    "mocap_186",
    "mocap_187",
    "mocap_188",
    "mocap_189",
    "mocap_190",
    "mocap_191",
    "mocap_192",
    "mocap_193",
    "mocap_194",
    "mocap_195",
    "mocap_196",
    "mocap_197",
    "mocap_198",
    "mocap_199",
    "mocap_200",
    "mocap_201",
    "mocap_202",
    "mocap_203",
    "mocap_204",
    "mocap_205",
    "mocap_206",
    "mocap_207",
    "mocap_208",
    "mocap_209",
    "mocap_210",
    "mocap_211",
    "mocap_212",
    "mocap_213",
    "mocap_214",
    "mocap_215",
    "mocap_216",
    "mocap_217",
    "mocap_218",
    "mocap_219",
    "mocap_220",
    "mocap_221",
    "mocap_222",
    "mocap_223",
    "mocap_224",
    "mocap_225",
    "mocap_226",
    "mocap_227",
    "mocap_228",
    "mocap_229",
    "mocap_230",
    "mocap_231",
    "mocap_232",
    "mocap_233",
    "mocap_234",
    "mocap_235",
    "mocap_236",
    "mocap_237",
    "mocap_238",
    "mocap_239",
    "mocap_240",
    "mocap_241",
    "mocap_242",
    "mocap_243",
    "mocap_244",
    "mocap_245",
    "mocap_246",
    "mocap_247",
    "mocap_248",
    "mocap_249",
    "mocap_250",
    "mocap_251",
    "mocap_252",
    "mocap_253",
    "mocap_254",
    "mocap_255",
];

fn static_parameter_name(index: usize) -> Option<&'static str> {
    STATIC_PARAMETER_NAMES.get(index).copied()
}

fn static_mark_name(index: usize) -> Option<&'static str> {
    const MARK_NAMES: [&str; 64] = [
        "bone_00", "bone_01", "bone_02", "bone_03", "bone_04", "bone_05", "bone_06", "bone_07",
        "bone_08", "bone_09", "bone_10", "bone_11", "bone_12", "bone_13", "bone_14", "bone_15",
        "bone_16", "bone_17", "bone_18", "bone_19", "bone_20", "bone_21", "bone_22", "bone_23",
        "bone_24", "bone_25", "bone_26", "bone_27", "bone_28", "bone_29", "bone_30", "bone_31",
        "bone_32", "bone_33", "bone_34", "bone_35", "bone_36", "bone_37", "bone_38", "bone_39",
        "bone_40", "bone_41", "bone_42", "bone_43", "bone_44", "bone_45", "bone_46", "bone_47",
        "bone_48", "bone_49", "bone_50", "bone_51", "bone_52", "bone_53", "bone_54", "bone_55",
        "bone_56", "bone_57", "bone_58", "bone_59", "bone_60", "bone_61", "bone_62", "bone_63",
    ];
    MARK_NAMES.get(index).copied()
}

/// ASF/AMC parser error.
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

    fn at(line_number: usize, message: impl Into<String>) -> Self {
        Self {
            line_number: Some(line_number),
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

    const ASF: &str = r#"
:version 1.10
:name tiny
:bonedata
begin
id 1
name lowerback
direction 0 0 1
length 2.5
axis 0 0 0 XYZ
dof rx ry rz
end
begin
id 2
name upperback
direction 1 0 0
length 3
axis 0 0 0 XYZ
dof rx ry rz
end
:hierarchy
begin
root lowerback
lowerback upperback
end
"#;

    const AMC: &str = r#"
:FULLY-SPECIFIED
:DEGREES
1
root 0 0 0 0 0 0
lowerback 1 2 3
upperback 4 5 6
2
root 0 0 0 10 20 30
lowerback 7 8 9
upperback 10 11 12
"#;

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
    fn parses_asf_bones_and_hierarchy() {
        let skeleton = parse_asf(ASF).expect("ASF should parse");

        assert_eq!(skeleton.version.as_deref(), Some("1.10"));
        assert_eq!(skeleton.name.as_deref(), Some("tiny"));
        assert_eq!(skeleton.bones.len(), 2);
        assert_eq!(skeleton.bone("lowerback").unwrap().length, 2.5);
        assert_eq!(
            skeleton.bone("upperback").unwrap().dof,
            [Dof::Rx, Dof::Ry, Dof::Rz]
        );
        assert_eq!(
            skeleton.hierarchy,
            [
                HierarchyEdge {
                    parent: "root".to_string(),
                    child: "lowerback".to_string(),
                },
                HierarchyEdge {
                    parent: "lowerback".to_string(),
                    child: "upperback".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parses_amc_frames() {
        let motion = parse_amc(AMC).expect("AMC should parse");

        assert_eq!(motion.frames.len(), 2);
        assert_eq!(motion.frames[0].index, 1);
        assert_eq!(motion.frames[0].joints[1].name, "lowerback");
        assert_eq!(motion.frames[0].joints[1].values, [1.0, 2.0, 3.0]);
        assert_eq!(
            motion.frames[1].joints[0].values,
            [0.0, 0.0, 0.0, 10.0, 20.0, 30.0]
        );
    }

    #[test]
    fn builds_static_linkage_from_asf() {
        let skeleton = parse_asf(ASF).expect("ASF should parse");
        let linkage = skeleton.static_linkage();

        assert!(linkage.view().poses(&[]).count() > 1);
    }

    #[test]
    fn discovers_parameters_then_builds_linkage_from_second_asf_pass() {
        let layout = discover_asf_parameters(ASF).expect("layout should parse");

        assert_eq!(layout.len(), 6);
        assert_eq!(layout.parameters[0].joint_name, "lowerback");
        assert_eq!(layout.parameters[0].dof, Dof::Rx);
        assert_eq!(layout.parameters[0].linkage_name, "mocap_000");

        let linkage = build_asf_linkage_buf::<6>(ASF, &layout).expect("linkage should build");
        assert!(linkage.view().poses(&[0.5; 6]).count() > 1);
    }

    #[test]
    fn converts_asf_to_lb_rs_source() {
        let source = asf_to_lb_rs::<6>(ASF).expect("ASF should serialize");

        assert!(source.starts_with("linkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(source.contains(".define_param(\"mocap_000\", 0.5)"));
        assert!(source.contains(".roll_param(\"mocap_000\", -180.0, 180.0)"));
    }

    #[test]
    fn converts_asf_and_amc_to_lb_rs_source_with_frame_defaults() {
        let source = asf_and_amc_to_lb_rs::<6>(ASF, AMC, 1).expect("ASF/AMC should serialize");

        assert!(source.starts_with("linkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(source.contains(".define_param(\"mocap_000\", 0.50277776)"));
        assert!(source.contains(".define_param(\"mocap_003\", 0.511111"));
    }

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
            build_bvh_linkage_buf::<32>(&clip, &layout).expect("BVH linkage should build");
        let params = bvh_frame_params::<32>(&layout, &clip.frames[1]).expect("params should build");

        assert_eq!(layout.len(), 15);
        assert!(params[0] > 0.5);
        assert!(linkage.view().draw_items(&params).count() >= 5);
    }

    #[test]
    fn bvh_rotation_axes_are_remapped_to_linkage_axes() {
        let clip = parse_bvh(BVH_X_ROTATION).expect("BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("BVH layout should parse");
        let linkage = build_bvh_linkage_buf::<1>(&clip, &layout).expect("BVH linkage should build");
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
    fn parses_real_cmu_subject_01_trial_01_when_present() {
        let Ok(asf) = std::fs::read_to_string("samples/cmu_01.asf") else {
            return;
        };
        let Ok(amc) = std::fs::read_to_string("samples/cmu_01_01.amc") else {
            return;
        };

        let skeleton = parse_asf(&asf).expect("real CMU ASF should parse");
        let motion = parse_amc(&amc).expect("real CMU AMC should parse");

        assert!(skeleton.bones.len() > 20);
        assert!(skeleton.bone("lowerback").is_some());
        assert!(motion.frames.len() > 100);
        assert_eq!(motion.frames[0].index, 1);
    }

    #[test]
    fn builds_real_cmu_parameterized_linkage_when_present() {
        let Ok(asf) = std::fs::read_to_string("samples/cmu_01.asf") else {
            return;
        };

        let layout = discover_asf_parameters(&asf).expect("real CMU layout should parse");
        let linkage =
            build_asf_linkage_buf::<128>(&asf, &layout).expect("real CMU linkage should build");

        assert!(layout.len() > 50);
        assert!(linkage.view().poses(&[0.5; 128]).count() > 20);
    }

    #[test]
    fn converts_real_cmu_asf_to_lb_rs_when_present() {
        let Ok(asf) = std::fs::read_to_string("samples/cmu_01.asf") else {
            return;
        };

        let source = asf_to_lb_rs::<128>(&asf).expect("real CMU ASF should serialize");

        assert!(source.starts_with("linkage![\n"));
        assert!(source.trim_end().ends_with(']'));
        assert!(source.contains(".define_param(\"mocap_000\", 0.5)"));
    }

    #[test]
    fn builds_real_bvh_linkage_when_present() {
        let Ok(bvh) = std::fs::read_to_string("samples/pirouette.bvh") else {
            return;
        };

        let clip = parse_bvh(&bvh).expect("real BVH should parse");
        let layout = discover_bvh_parameters(&clip).expect("real BVH layout should parse");
        let linkage =
            build_bvh_linkage_buf::<256>(&clip, &layout).expect("real BVH linkage should build");
        let params =
            bvh_frame_params::<256>(&layout, &clip.frames[0]).expect("real params should build");

        assert!(clip.joints.len() > 40);
        assert!(clip.frames.len() > 500);
        assert!(layout.len() > 120);
        assert!(linkage.view().draw_items(&params).count() > 40);
    }
}
