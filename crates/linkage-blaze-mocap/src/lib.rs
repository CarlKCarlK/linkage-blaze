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
            linkage = apply_joint_parameters(linkage, layout, "root");
        } else if let Some(parent_index) = skeleton.bone_index(&edge.parent) {
            let Some(parent_mark_name) = static_mark_name(parent_index) else {
                continue;
            };
            linkage = linkage.restore(parent_mark_name);
            linkage = apply_joint_parameters(linkage, layout, &edge.parent);
        }

        if let Some(child_bone) = skeleton.bone(&edge.child) {
            let Some(child_index) = skeleton.bone_index(&edge.child) else {
                continue;
            };
            let Some(child_mark_name) = static_mark_name(child_index) else {
                continue;
            };
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

const STATIC_PARAMETER_NAMES: [&str; 128] = [
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
}
