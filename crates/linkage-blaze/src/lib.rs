#![forbid(unsafe_code)]

use core::f32::consts::PI;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

const DEFAULT_PROGRAM: &str = r#"Linkage::start()
.define_param("x/y view", 0.5833333)
.define_param("z", 0.8333333)
.define_param("raise hand", 0.5)
.define_param("bend elbow", 0.5)
.define_param("close hand", 0.0)
.define_param("lower arm", 0.5)
.define_param("spin whole", 0.5)
.define_param("spin hand", 0.5)
.pen_color(Rgb888::CSS_DARK_GREEN)
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
.pitch_param("raise hand", 90.0, -90.0)
.forward(1.0)
.roll_param("spin hand", -360.0, 360.0)
.forward(0.5)
.yaw(90.0)
.forward_param("close hand", 0.5, 0.0)
.yaw(-90.0)
.forward(1.0)
.yaw(180.0)
.forward(1.0)
.yaw(90.0)
.forward_param("close hand", 1.0, 0.0)
.yaw(90.0)
.forward(1.0)
.restart()
.pen_color(Rgb888::CSS_RED)
.disk(0.05)
"#;

#[wasm_bindgen]
pub fn default_program() -> String {
    DEFAULT_PROGRAM.into()
}

#[wasm_bindgen]
pub fn render_program_json(source: &str) -> Result<String, JsValue> {
    render_program(source, &[]).map_err(|error| JsValue::from_str(&error))
}

/// Re-render the program using caller-supplied param values (by name).
///
/// `overrides_json` is a JSON object mapping param name to value, e.g.
/// `{"x/y view":0.583,"z":0.7}`. Unknown names are ignored; missing names
/// fall back to the `define_param` default.
#[wasm_bindgen]
pub fn render_program_with_params_json(
    source: &str,
    overrides_json: &str,
) -> Result<String, JsValue> {
    let overrides = parse_overrides(overrides_json);
    render_program(source, &overrides).map_err(|error| JsValue::from_str(&error))
}

fn render_program(source: &str, overrides: &[(String, f32)]) -> Result<String, String> {
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
            overrides,
            &mut primitives,
        )?;
    }

    Ok(result_json(&primitives, &params))
}

/// Parse `{"name":value,...}` into a vec of (name, value) pairs.
fn parse_overrides(json: &str) -> Vec<(String, f32)> {
    let mut result = Vec::new();
    let json = json.trim();
    if json.len() < 2 {
        return result;
    }
    let inner = &json[1..json.len() - 1];
    for pair in inner.split(',') {
        let pair = pair.trim();
        let Some(colon) = pair.find(':') else {
            continue;
        };
        let name = pair[..colon].trim().trim_matches('"');
        let value_str = pair[colon + 1..].trim();
        if let Ok(value) = value_str.parse::<f32>() {
            result.push((name.to_owned(), value));
        }
    }
    result
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

    let args = split_args(line_number, &line[open + 1..close])?;

    Ok(Some(MethodCall {
        name: name.to_owned(),
        args,
    }))
}

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

fn apply_method(
    line_number: usize,
    method_call: &MethodCall,
    turtle: &mut Turtle,
    params: &mut Vec<EditorParam>,
    overrides: &[(String, f32)],
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
            let value = overrides
                .iter()
                .find(|(n, _)| n == name)
                .map_or(default, |(_, v)| v.clamp(0.0, 1.0));
            params.push(EditorParam {
                name: name.to_owned(),
                value,
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
        "forward_param" => {
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
            turtle.restart();
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
            let radius = parse_param_radius(line_number, method_call, params)?;
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
        "ring_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            let radius = parse_param_radius(line_number, method_call, params)?;
            primitives.push(Primitive::Ring {
                center: turtle.pose.position,
                normal: turtle.pose.orientation.up(),
                radius,
                width: turtle.width,
                color: turtle.color,
            });
        }
        "sphere_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            let radius = parse_param_radius(line_number, method_call, params)?;
            primitives.push(Primitive::Sphere {
                center: turtle.pose.position,
                radius,
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

fn parse_param_radius(
    line_number: usize,
    method_call: &MethodCall,
    params: &[EditorParam],
) -> Result<f32, String> {
    let radius = parse_param_arg(line_number, method_call, params)?;
    if radius < 0.0 {
        Err(format!(
            "line {line_number}: `{}` radius resolved below zero",
            method_call.name
        ))
    } else {
        Ok(radius)
    }
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
    let numeric_value: String = value
        .chars()
        .filter(|character| *character != '_')
        .collect();

    if let Ok(value) = numeric_value.parse::<f32>() {
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

    if let Some(color_args) = value
        .strip_prefix("Rgb888::new(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_rgb888_new_color(line_number, color_args);
    }

    match value {
        "Rgb888::CSS_BLACK" => Ok(rgb888_color(0, 0, 0)),
        "Rgb888::CSS_WHITE" => Ok(rgb888_color(255, 255, 255)),
        "Rgb888::CSS_RED" => Ok(rgb888_color(255, 0, 0)),
        "Rgb888::CSS_GREEN" => Ok(rgb888_color(0, 128, 0)),
        "Rgb888::CSS_LIME" => Ok(rgb888_color(0, 255, 0)),
        "Rgb888::CSS_BLUE" => Ok(rgb888_color(0, 0, 255)),
        "Rgb888::CSS_CYAN" => Ok(rgb888_color(0, 255, 255)),
        "Rgb888::CSS_YELLOW" => Ok(rgb888_color(255, 255, 0)),
        "Rgb888::CSS_ORANGE" => Ok(rgb888_color(255, 165, 0)),
        "Rgb888::CSS_DARK_GREEN" => Ok(rgb888_color(0, 100, 0)),
        "Rgb888::CSS_LIGHT_SLATE_GRAY" => Ok(rgb888_color(119, 136, 153)),
        "Rgb888::CSS_ANTIQUE_WHITE" => Ok(rgb888_color(250, 235, 215)),
        "Rgb888::CSS_IVORY" => Ok(rgb888_color(255, 255, 240)),
        "Rgb888::CSS_NAVY" => Ok(rgb888_color(0, 0, 128)),
        "Rgb888::CSS_MEDIUM_BLUE" => Ok(rgb888_color(0, 0, 205)),
        "Rgb888::CSS_LIGHT_SKY_BLUE" => Ok(rgb888_color(135, 206, 250)),
        "Rgb888::CSS_TOMATO" => Ok(rgb888_color(255, 99, 71)),
        _ => Err(format!(
            "line {line_number}: unknown color `{value}`; use `Rgb888::CSS_*` or `Rgb888::new(r, g, b)`"
        )),
    }
}

fn parse_rgb888_new_color(line_number: usize, args: &str) -> Result<Color, String> {
    let args = split_args(line_number, args)?;
    if args.len() != 3 {
        return Err(format!(
            "line {line_number}: `Rgb888::new` expects 3 argument(s), got {}",
            args.len()
        ));
    }

    Ok(rgb888_color(
        parse_u8_arg(line_number, "Rgb888::new", &args[0])?,
        parse_u8_arg(line_number, "Rgb888::new", &args[1])?,
        parse_u8_arg(line_number, "Rgb888::new", &args[2])?,
    ))
}

fn parse_u8_arg(line_number: usize, method_name: &str, value: &str) -> Result<u8, String> {
    let numeric_value: String = value
        .chars()
        .filter(|character| *character != '_')
        .collect();
    numeric_value
        .parse::<u8>()
        .map_err(|_| format!("line {line_number}: `{method_name}` argument `{value}` is not a u8"))
}

fn number_constant(name: &str) -> Option<f32> {
    match name {
        "ARM_WIDTH" => Some(3.0),
        "AXIS_WIDTH" => Some(1.0),
        _ => None,
    }
}

fn rgb888_color(red: u8, green: u8, blue: u8) -> Color {
    Color::new(
        red as f32 / 255.0,
        green as f32 / 255.0,
        blue as f32 / 255.0,
    )
}

fn result_json(primitives: &[Primitive], params: &[EditorParam]) -> String {
    let mut json = String::from("{\"primitives\":[");
    for (i, primitive) in primitives.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        primitive.push_json(&mut json);
    }
    json.push_str("],\"params\":[");
    for (i, param) in params.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"name\":\"");
        json.push_str(&param.name);
        json.push_str("\",\"value\":");
        push_float(&mut json, param.value);
        json.push('}');
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
            width: 0.1,
        }
    }

    fn restart(&mut self) {
        *self = Self::new();
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

#[cfg(test)]
mod tests {
    use super::render_program;

    #[test]
    fn accepts_rust_rgb888_new_color() {
        assert!(
            render_program(
                r#"Linkage::start()
.pen_color(Rgb888::new(245, 238, 210))
.disk(1.0)
"#,
                &[],
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_non_rust_color_forms() {
        for color in ["white", "CSS_RED", "#ff0000"] {
            let program = format!(
                r#"Linkage::start()
.pen_color({color})
.disk(1.0)
"#
            );
            assert!(render_program(&program, &[]).is_err());
        }
    }
}
