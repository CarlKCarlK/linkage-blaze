#![forbid(unsafe_code)]

use core::f32::consts::PI;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

const DEFAULT_PROGRAM: &str = r#"Linkage::start()
.define_param("x/y view", 0.5833333)
.define_param("z", 0.8333333)
.define_param("lower hand", 0.5)
.define_param("bend elbow", 0.5)
.define_param("close hand", 0.0)
.define_param("lower arm", 0.5)
.define_param("spin whole", 0.5)
.define_param("spin hand", 0.5)
.pen_color(ARM_COLOR) // default pen is down, white, width 1
.pen_width(ARM_WIDTH)
.yaw(90.0)
.pitch(-90.0)
.yaw_param("x/y view", 90.0, -90.0)
.pitch_param("z", -45.0, 45.0)
.yaw_param("spin whole", 360.0, -360.0)
.pitch(90.0)
.forward(2.5)
.pitch(-90.0)
.pitch_param("lower arm", 30.0, 0.0)
.forward(3.0)
.yaw_param("bend elbow", 90.0, -90.0)
.forward(3.0)
.pitch_param("lower hand", 90.0, -90.0)
.forward(1.0)
.roll_param("spin hand", 360.0, -360.0)
.forward(0.5)
.yaw(90.0)
.move_param("close hand", 0.5, 0.0)
.yaw(-90.0)
.forward(1.0)
.yaw(180.0)
.forward(1.0)
.yaw(90.0)
.move_param("close hand", 1.0, 0.0)
.yaw(90.0)
.forward(1.0)
.restart()
.pen_color(TARGET_COLOR)
.disk(0.05)
"#;

#[wasm_bindgen]
pub fn default_program() -> String {
    DEFAULT_PROGRAM.into()
}

#[wasm_bindgen]
pub fn render_program_json(source: &str) -> Result<String, JsValue> {
    render_program(source).map_err(|error| JsValue::from_str(&error))
}

fn render_program(source: &str) -> Result<String, String> {
    let mut turtle = Turtle::new();
    let mut params = Vec::new();
    let mut primitives = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let Some(method_call) = parse_method_call(line_number, line)? else {
            continue;
        };

        apply_method(
            line_number,
            &method_call,
            &mut turtle,
            &mut params,
            &mut primitives,
        )?;
    }

    Ok(primitives_json(&primitives))
}

fn parse_method_call(line_number: usize, line: &str) -> Result<Option<MethodCall>, String> {
    let line = strip_rust_comment(line).trim();
    if line.is_empty() || line == "Linkage::start()" {
        return Ok(None);
    }

    if line.contains("Linkage::start()") {
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

    let args = line[open + 1..close]
        .split(',')
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(str::to_owned)
        .collect();

    Ok(Some(MethodCall {
        name: name.to_owned(),
        args,
    }))
}

fn apply_method(
    line_number: usize,
    method_call: &MethodCall,
    turtle: &mut Turtle,
    params: &mut Vec<EditorParam>,
    primitives: &mut Vec<Primitive>,
) -> Result<(), String> {
    match method_call.name.as_str() {
        "define_param" => {
            expect_arg_count(line_number, method_call, 2)?;
            let name = parse_string_arg(line_number, method_call, 0)?;
            let default = parse_number_arg(line_number, method_call, 1)?;
            if !(0.0..=1.0).contains(&default) {
                return Err(format!(
                    "line {line_number}: define_param default must be between 0.0 and 1.0"
                ));
            }
            if params.iter().any(|param| param.name == name) {
                return Err(format!(
                    "line {line_number}: parameter `{name}` is already defined"
                ));
            }
            params.push(EditorParam {
                name: name.to_owned(),
                value: default,
            });
        }
        "forward" | "move" => {
            expect_arg_count(line_number, method_call, 1)?;
            let distance = parse_number_arg(line_number, method_call, 0)?;
            let start = turtle.pose.position;
            turtle.pose.position =
                turtle.pose.position + turtle.pose.orientation.forward() * distance;
            if turtle.pen == Pen::Down {
                primitives.push(Primitive::Segment {
                    start,
                    end: turtle.pose.position,
                    width: turtle.width,
                    color: turtle.color,
                });
            }
        }
        "move_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            let distance = parse_param_arg(line_number, method_call, params)?;
            let start = turtle.pose.position;
            turtle.pose.position =
                turtle.pose.position + turtle.pose.orientation.forward() * distance;
            if turtle.pen == Pen::Down {
                primitives.push(Primitive::Segment {
                    start,
                    end: turtle.pose.position,
                    width: turtle.width,
                    color: turtle.color,
                });
            }
        }
        "yaw" => {
            expect_arg_count(line_number, method_call, 1)?;
            turtle.pose.orientation = turtle.pose.orientation
                * Mat3::yaw(degrees_to_radians(parse_number_arg(
                    line_number,
                    method_call,
                    0,
                )?));
        }
        "pitch" => {
            expect_arg_count(line_number, method_call, 1)?;
            turtle.pose.orientation = turtle.pose.orientation
                * Mat3::pitch(degrees_to_radians(parse_number_arg(
                    line_number,
                    method_call,
                    0,
                )?));
        }
        "roll" => {
            expect_arg_count(line_number, method_call, 1)?;
            turtle.pose.orientation = turtle.pose.orientation
                * Mat3::roll(degrees_to_radians(parse_number_arg(
                    line_number,
                    method_call,
                    0,
                )?));
        }
        "yaw_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            turtle.pose.orientation = turtle.pose.orientation
                * Mat3::yaw(degrees_to_radians(parse_param_arg(
                    line_number,
                    method_call,
                    params,
                )?));
        }
        "pitch_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            turtle.pose.orientation = turtle.pose.orientation
                * Mat3::pitch(degrees_to_radians(parse_param_arg(
                    line_number,
                    method_call,
                    params,
                )?));
        }
        "roll_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            turtle.pose.orientation = turtle.pose.orientation
                * Mat3::roll(degrees_to_radians(parse_param_arg(
                    line_number,
                    method_call,
                    params,
                )?));
        }
        "pen_up" => {
            expect_arg_count(line_number, method_call, 0)?;
            turtle.pen = Pen::Up;
        }
        "pen_down" => {
            expect_arg_count(line_number, method_call, 0)?;
            turtle.pen = Pen::Down;
        }
        "pen_color" => {
            expect_arg_count(line_number, method_call, 1)?;
            turtle.color = parse_color_arg(line_number, method_call)?;
        }
        "pen_width" => {
            expect_arg_count(line_number, method_call, 1)?;
            turtle.width = parse_number_arg(line_number, method_call, 0)?;
            if turtle.width < 0.0 {
                return Err(format!(
                    "line {line_number}: pen_width must be non-negative"
                ));
            }
        }
        "restart" => {
            expect_arg_count(line_number, method_call, 0)?;
            turtle.pose = Pose::start();
        }
        "disk" => {
            expect_arg_count(line_number, method_call, 1)?;
            let radius = parse_radius(line_number, method_call, 0)?;
            primitives.push(Primitive::Disk {
                center: turtle.pose.position,
                normal: turtle.pose.orientation.up(),
                radius,
                color: turtle.color,
            });
        }
        "sphere" => {
            expect_arg_count(line_number, method_call, 1)?;
            let radius = parse_radius(line_number, method_call, 0)?;
            primitives.push(Primitive::Sphere {
                center: turtle.pose.position,
                radius,
                color: turtle.color,
            });
        }
        "disk_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            let radius = parse_param_arg(line_number, method_call, params)?;
            if radius < 0.0 {
                return Err(format!(
                    "line {line_number}: disk_param radius resolved below zero"
                ));
            }
            primitives.push(Primitive::Disk {
                center: turtle.pose.position,
                normal: turtle.pose.orientation.up(),
                radius,
                color: turtle.color,
            });
        }
        "ring" => {
            expect_arg_count(line_number, method_call, 1)?;
            let radius = parse_radius(line_number, method_call, 0)?;
            primitives.push(Primitive::Ring {
                center: turtle.pose.position,
                normal: turtle.pose.orientation.up(),
                radius,
                width: turtle.width,
                color: turtle.color,
            });
        }
        _ => {
            return Err(format!(
                "line {line_number}: unknown method `{}`",
                method_call.name
            ));
        }
    }

    Ok(())
}

fn strip_rust_comment(line: &str) -> &str {
    line.split_once("//")
        .map_or(line, |(before_comment, _)| before_comment)
}

fn expect_arg_count(
    line_number: usize,
    method_call: &MethodCall,
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

fn parse_number_arg(
    line_number: usize,
    method_call: &MethodCall,
    arg_index: usize,
) -> Result<f32, String> {
    let value = &method_call.args[arg_index];
    parse_number_or_constant(line_number, method_call.name.as_str(), value)
}

fn parse_param_arg(
    line_number: usize,
    method_call: &MethodCall,
    params: &[EditorParam],
) -> Result<f32, String> {
    let param_name = parse_string_arg(line_number, method_call, 0)?;
    let param = params
        .iter()
        .find(|param| param.name == param_name)
        .ok_or_else(|| format!("line {line_number}: unknown param `{param_name}`"))?;
    let low = parse_number_arg(line_number, method_call, 1)?;
    let high = parse_number_arg(line_number, method_call, 2)?;
    Ok(low + param.value * (high - low))
}

fn parse_string_arg(
    line_number: usize,
    method_call: &MethodCall,
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

fn parse_number_or_constant(
    line_number: usize,
    method_name: &str,
    value: &str,
) -> Result<f32, String> {
    if let Ok(value) = value.parse::<f32>() {
        return Ok(value);
    }
    number_constant(value).ok_or_else(|| {
        format!("line {line_number}: `{method_name}` argument `{value}` is not a number or known constant")
    })
}

fn parse_radius(
    line_number: usize,
    method_call: &MethodCall,
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

fn parse_color_arg(line_number: usize, method_call: &MethodCall) -> Result<Color, String> {
    let value = method_call.args[0].as_str();
    match value {
        "white" => Ok(Color::new(1.0, 1.0, 1.0)),
        "black" => Ok(Color::new(0.0, 0.0, 0.0)),
        "red" => Ok(Color::new(1.0, 0.0, 0.0)),
        "green" => Ok(Color::new(0.0, 1.0, 0.0)),
        "blue" => Ok(Color::new(0.0, 0.0, 1.0)),
        "cyan" => Ok(Color::new(0.0, 1.0, 1.0)),
        "yellow" => Ok(Color::new(1.0, 1.0, 0.0)),
        "orange" => Ok(Color::new(1.0, 0.44, 0.25)),
        _ if value.starts_with('#') && value.len() == 7 => parse_hex_color(line_number, value),
        _ => color_constant(value)
            .ok_or_else(|| format!("line {line_number}: unknown color `{value}`")),
    }
}

fn number_constant(name: &str) -> Option<f32> {
    match name {
        "ARM_WIDTH" => Some(3.0),
        "AXIS_WIDTH" => Some(1.0),
        _ => None,
    }
}

fn color_constant(name: &str) -> Option<Color> {
    match name {
        "FLOOR_COLOR" => Some(rgb565_color(5, 19, 9)),
        "AXIS_COLOR" => Some(rgb565_color(10, 28, 14)),
        "ARM_COLOR" => Some(rgb565_color(0, 34, 17)),
        "TARGET_COLOR" => Some(rgb565_color(31, 0, 0)),
        _ => None,
    }
}

fn rgb565_color(red: u8, green: u8, blue: u8) -> Color {
    Color::new(red as f32 / 31.0, green as f32 / 63.0, blue as f32 / 31.0)
}

fn parse_hex_color(line_number: usize, value: &str) -> Result<Color, String> {
    let red = parse_hex_byte(line_number, value, 1)?;
    let green = parse_hex_byte(line_number, value, 3)?;
    let blue = parse_hex_byte(line_number, value, 5)?;
    Ok(Color::new(
        red as f32 / 255.0,
        green as f32 / 255.0,
        blue as f32 / 255.0,
    ))
}

fn parse_hex_byte(line_number: usize, value: &str, start: usize) -> Result<u8, String> {
    u8::from_str_radix(&value[start..start + 2], 16)
        .map_err(|_| format!("line {line_number}: invalid hex color `{value}`"))
}

fn primitives_json(primitives: &[Primitive]) -> String {
    let mut json = String::from("{\"primitives\":[");
    for (primitive_index, primitive) in primitives.iter().enumerate() {
        if primitive_index > 0 {
            json.push(',');
        }
        primitive.push_json(&mut json);
    }
    json.push_str("]}");
    json
}

fn degrees_to_radians(degrees: f32) -> f32 {
    degrees * (PI / 180.0)
}

#[derive(Debug)]
struct MethodCall {
    name: String,
    args: Vec<String>,
}

#[derive(Clone, Debug)]
struct EditorParam {
    name: String,
    value: f32,
}

#[derive(Clone, Copy, Debug)]
struct Turtle {
    pose: Pose,
    pen: Pen,
    color: Color,
    width: f32,
}

impl Turtle {
    fn new() -> Self {
        Self {
            pose: Pose::start(),
            pen: Pen::Down,
            color: Color::new(1.0, 1.0, 1.0),
            width: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Pose {
    orientation: Mat3,
    position: Vec3,
}

impl Pose {
    fn start() -> Self {
        Self {
            orientation: Mat3::IDENTITY,
            position: Vec3::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Pen {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug)]
struct Color {
    red: f32,
    green: f32,
    blue: f32,
}

impl Color {
    const fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }

    fn push_json(self, json: &mut String) {
        push_float(json, self.red);
        json.push(',');
        push_float(json, self.green);
        json.push(',');
        push_float(json, self.blue);
    }
}

#[derive(Clone, Copy, Debug)]
enum Primitive {
    Segment {
        start: Vec3,
        end: Vec3,
        width: f32,
        color: Color,
    },
    Disk {
        center: Vec3,
        normal: Vec3,
        radius: f32,
        color: Color,
    },
    Ring {
        center: Vec3,
        normal: Vec3,
        radius: f32,
        width: f32,
        color: Color,
    },
    Sphere {
        center: Vec3,
        radius: f32,
        color: Color,
    },
}

impl Primitive {
    fn push_json(self, json: &mut String) {
        match self {
            Self::Segment {
                start,
                end,
                width,
                color,
            } => {
                json.push_str("{\"type\":\"segment\",\"start\":");
                start.push_json(json);
                json.push_str(",\"end\":");
                end.push_json(json);
                json.push_str(",\"width\":");
                push_float(json, width);
                json.push_str(",\"color\":[");
                color.push_json(json);
                json.push_str("]}");
            }
            Self::Disk {
                center,
                normal,
                radius,
                color,
            } => {
                json.push_str("{\"type\":\"disk\",\"center\":");
                center.push_json(json);
                json.push_str(",\"normal\":");
                normal.push_json(json);
                json.push_str(",\"radius\":");
                push_float(json, radius);
                json.push_str(",\"color\":[");
                color.push_json(json);
                json.push_str("]}");
            }
            Self::Ring {
                center,
                normal,
                radius,
                width,
                color,
            } => {
                json.push_str("{\"type\":\"ring\",\"center\":");
                center.push_json(json);
                json.push_str(",\"normal\":");
                normal.push_json(json);
                json.push_str(",\"radius\":");
                push_float(json, radius);
                json.push_str(",\"width\":");
                push_float(json, width);
                json.push_str(",\"color\":[");
                color.push_json(json);
                json.push_str("]}");
            }
            Self::Sphere {
                center,
                radius,
                color,
            } => {
                json.push_str("{\"type\":\"sphere\",\"center\":");
                center.push_json(json);
                json.push_str(",\"radius\":");
                push_float(json, radius);
                json.push_str(",\"color\":[");
                color.push_json(json);
                json.push_str("]}");
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn push_json(self, json: &mut String) {
        json.push('[');
        push_float(json, self.x);
        json.push(',');
        push_float(json, self.y);
        json.push(',');
        push_float(json, self.z);
        json.push(']');
    }
}

impl core::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, right: Self) -> Self::Output {
        Self::new(self.x + right.x, self.y + right.y, self.z + right.z)
    }
}

impl core::ops::Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, right: f32) -> Self::Output {
        Self::new(self.x * right, self.y * right, self.z * right)
    }
}

#[derive(Clone, Copy, Debug)]
struct Mat3 {
    rows: [[f32; 3]; 3],
}

impl Mat3 {
    const IDENTITY: Self = Self {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    fn yaw(radians: f32) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self {
            rows: [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    fn pitch(radians: f32) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self {
            rows: [[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]],
        }
    }

    fn roll(radians: f32) -> Self {
        let cos = radians.cos();
        let sin = radians.sin();
        Self {
            rows: [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]],
        }
    }

    fn forward(self) -> Vec3 {
        Vec3::new(self.rows[0][0], self.rows[1][0], self.rows[2][0])
    }

    fn up(self) -> Vec3 {
        Vec3::new(self.rows[0][2], self.rows[1][2], self.rows[2][2])
    }
}

impl core::ops::Mul for Mat3 {
    type Output = Self;

    fn mul(self, right: Self) -> Self::Output {
        let mut rows = [[0.0f32; 3]; 3];
        for row_index in 0..3 {
            for column_index in 0..3 {
                for component_index in 0..3 {
                    rows[row_index][column_index] += self.rows[row_index][component_index]
                        * right.rows[component_index][column_index];
                }
            }
        }
        Self { rows }
    }
}

fn push_float(json: &mut String, value: f32) {
    if value.is_finite() {
        json.push_str(&format!("{value:.5}"));
    } else {
        json.push('0');
    }
}
