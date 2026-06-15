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
.pitch_param("lower hand", 90.0, -90.0)
.forward(1.0)
.roll_param("spin hand", 360.0, -360.0)
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
        let Some(colon) = pair.find(':') else { continue };
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
    // Strip optional "Rgb888::" prefix so both Rgb888::CSS_ORANGE and CSS_ORANGE work
    let key = value.strip_prefix("Rgb888::").unwrap_or(value);
    if key.starts_with('#') && key.len() == 7 {
        return parse_hex_color(line_number, key);
    }
    css_color(key)
        .ok_or_else(|| format!("line {line_number}: unknown color `{value}`"))
}

fn number_constant(name: &str) -> Option<f32> {
    match name {
        "ARM_WIDTH" => Some(3.0),
        "AXIS_WIDTH" => Some(1.0),
        _ => None,
    }
}

fn rgb888_color(red: u8, green: u8, blue: u8) -> Color {
    Color::new(red as f32 / 255.0, green as f32 / 255.0, blue as f32 / 255.0)
}

fn css_color(name: &str) -> Option<Color> {
    match name {
        "CSS_ALICE_BLUE" => Some(rgb888_color(240, 248, 255)),
        "CSS_ANTIQUE_WHITE" => Some(rgb888_color(250, 235, 215)),
        "CSS_AQUA" => Some(rgb888_color(0, 255, 255)),
        "CSS_AQUAMARINE" => Some(rgb888_color(127, 255, 212)),
        "CSS_AZURE" => Some(rgb888_color(240, 255, 255)),
        "CSS_BEIGE" => Some(rgb888_color(245, 245, 220)),
        "CSS_BISQUE" => Some(rgb888_color(255, 228, 196)),
        "CSS_BLACK" => Some(rgb888_color(0, 0, 0)),
        "CSS_BLANCHED_ALMOND" => Some(rgb888_color(255, 235, 205)),
        "CSS_BLUE" => Some(rgb888_color(0, 0, 255)),
        "CSS_BLUE_VIOLET" => Some(rgb888_color(138, 43, 226)),
        "CSS_BROWN" => Some(rgb888_color(165, 42, 42)),
        "CSS_BURLY_WOOD" => Some(rgb888_color(222, 184, 135)),
        "CSS_CADET_BLUE" => Some(rgb888_color(95, 158, 160)),
        "CSS_CHARTREUSE" => Some(rgb888_color(127, 255, 0)),
        "CSS_CHOCOLATE" => Some(rgb888_color(210, 105, 30)),
        "CSS_CORAL" => Some(rgb888_color(255, 127, 80)),
        "CSS_CORNFLOWER_BLUE" => Some(rgb888_color(100, 149, 237)),
        "CSS_CORNSILK" => Some(rgb888_color(255, 248, 220)),
        "CSS_CRIMSON" => Some(rgb888_color(220, 20, 60)),
        "CSS_CYAN" => Some(rgb888_color(0, 255, 255)),
        "CSS_DARK_BLUE" => Some(rgb888_color(0, 0, 139)),
        "CSS_DARK_CYAN" => Some(rgb888_color(0, 139, 139)),
        "CSS_DARK_GOLDENROD" => Some(rgb888_color(184, 134, 11)),
        "CSS_DARK_GRAY" => Some(rgb888_color(169, 169, 169)),
        "CSS_DARK_GREEN" => Some(rgb888_color(0, 100, 0)),
        "CSS_DARK_KHAKI" => Some(rgb888_color(189, 183, 107)),
        "CSS_DARK_MAGENTA" => Some(rgb888_color(139, 0, 139)),
        "CSS_DARK_OLIVE_GREEN" => Some(rgb888_color(85, 107, 47)),
        "CSS_DARK_ORANGE" => Some(rgb888_color(255, 140, 0)),
        "CSS_DARK_ORCHID" => Some(rgb888_color(153, 50, 204)),
        "CSS_DARK_RED" => Some(rgb888_color(139, 0, 0)),
        "CSS_DARK_SALMON" => Some(rgb888_color(233, 150, 122)),
        "CSS_DARK_SEA_GREEN" => Some(rgb888_color(143, 188, 143)),
        "CSS_DARK_SLATE_BLUE" => Some(rgb888_color(72, 61, 139)),
        "CSS_DARK_SLATE_GRAY" => Some(rgb888_color(47, 79, 79)),
        "CSS_DARK_TURQUOISE" => Some(rgb888_color(0, 206, 209)),
        "CSS_DARK_VIOLET" => Some(rgb888_color(148, 0, 211)),
        "CSS_DEEP_PINK" => Some(rgb888_color(255, 20, 147)),
        "CSS_DEEP_SKY_BLUE" => Some(rgb888_color(0, 191, 255)),
        "CSS_DIM_GRAY" => Some(rgb888_color(105, 105, 105)),
        "CSS_DODGER_BLUE" => Some(rgb888_color(30, 144, 255)),
        "CSS_FIRE_BRICK" => Some(rgb888_color(178, 34, 34)),
        "CSS_FLORAL_WHITE" => Some(rgb888_color(255, 250, 240)),
        "CSS_FOREST_GREEN" => Some(rgb888_color(34, 139, 34)),
        "CSS_FUCHSIA" => Some(rgb888_color(255, 0, 255)),
        "CSS_GAINSBORO" => Some(rgb888_color(220, 220, 220)),
        "CSS_GHOST_WHITE" => Some(rgb888_color(248, 248, 255)),
        "CSS_GOLD" => Some(rgb888_color(255, 215, 0)),
        "CSS_GOLDENROD" => Some(rgb888_color(218, 165, 32)),
        "CSS_GRAY" => Some(rgb888_color(128, 128, 128)),
        "CSS_GREEN" => Some(rgb888_color(0, 128, 0)),
        "CSS_GREEN_YELLOW" => Some(rgb888_color(173, 255, 47)),
        "CSS_HONEYDEW" => Some(rgb888_color(240, 255, 240)),
        "CSS_HOT_PINK" => Some(rgb888_color(255, 105, 180)),
        "CSS_INDIAN_RED" => Some(rgb888_color(205, 92, 92)),
        "CSS_INDIGO" => Some(rgb888_color(75, 0, 130)),
        "CSS_IVORY" => Some(rgb888_color(255, 255, 240)),
        "CSS_KHAKI" => Some(rgb888_color(240, 230, 140)),
        "CSS_LAVENDER" => Some(rgb888_color(230, 230, 250)),
        "CSS_LAVENDER_BLUSH" => Some(rgb888_color(255, 240, 245)),
        "CSS_LAWN_GREEN" => Some(rgb888_color(124, 252, 0)),
        "CSS_LEMON_CHIFFON" => Some(rgb888_color(255, 250, 205)),
        "CSS_LIGHT_BLUE" => Some(rgb888_color(173, 216, 230)),
        "CSS_LIGHT_CORAL" => Some(rgb888_color(240, 128, 128)),
        "CSS_LIGHT_CYAN" => Some(rgb888_color(224, 255, 255)),
        "CSS_LIGHT_GOLDENROD_YELLOW" => Some(rgb888_color(250, 250, 210)),
        "CSS_LIGHT_GRAY" => Some(rgb888_color(211, 211, 211)),
        "CSS_LIGHT_GREEN" => Some(rgb888_color(144, 238, 144)),
        "CSS_LIGHT_PINK" => Some(rgb888_color(255, 182, 193)),
        "CSS_LIGHT_SALMON" => Some(rgb888_color(255, 160, 122)),
        "CSS_LIGHT_SEA_GREEN" => Some(rgb888_color(32, 178, 170)),
        "CSS_LIGHT_SKY_BLUE" => Some(rgb888_color(135, 206, 250)),
        "CSS_LIGHT_SLATE_GRAY" => Some(rgb888_color(119, 136, 153)),
        "CSS_LIGHT_STEEL_BLUE" => Some(rgb888_color(176, 196, 222)),
        "CSS_LIGHT_YELLOW" => Some(rgb888_color(255, 255, 224)),
        "CSS_LIME" => Some(rgb888_color(0, 255, 0)),
        "CSS_LIME_GREEN" => Some(rgb888_color(50, 205, 50)),
        "CSS_LINEN" => Some(rgb888_color(250, 240, 230)),
        "CSS_MAGENTA" => Some(rgb888_color(255, 0, 255)),
        "CSS_MAROON" => Some(rgb888_color(128, 0, 0)),
        "CSS_MEDIUM_AQUAMARINE" => Some(rgb888_color(102, 205, 170)),
        "CSS_MEDIUM_BLUE" => Some(rgb888_color(0, 0, 205)),
        "CSS_MEDIUM_ORCHID" => Some(rgb888_color(186, 85, 211)),
        "CSS_MEDIUM_PURPLE" => Some(rgb888_color(147, 112, 219)),
        "CSS_MEDIUM_SEA_GREEN" => Some(rgb888_color(60, 179, 113)),
        "CSS_MEDIUM_SLATE_BLUE" => Some(rgb888_color(123, 104, 238)),
        "CSS_MEDIUM_SPRING_GREEN" => Some(rgb888_color(0, 250, 154)),
        "CSS_MEDIUM_TURQUOISE" => Some(rgb888_color(72, 209, 204)),
        "CSS_MEDIUM_VIOLET_RED" => Some(rgb888_color(199, 21, 133)),
        "CSS_MIDNIGHT_BLUE" => Some(rgb888_color(25, 25, 112)),
        "CSS_MINT_CREAM" => Some(rgb888_color(245, 255, 250)),
        "CSS_MISTY_ROSE" => Some(rgb888_color(255, 228, 225)),
        "CSS_MOCCASIN" => Some(rgb888_color(255, 228, 181)),
        "CSS_NAVAJO_WHITE" => Some(rgb888_color(255, 222, 173)),
        "CSS_NAVY" => Some(rgb888_color(0, 0, 128)),
        "CSS_OLD_LACE" => Some(rgb888_color(253, 245, 230)),
        "CSS_OLIVE" => Some(rgb888_color(128, 128, 0)),
        "CSS_OLIVE_DRAB" => Some(rgb888_color(107, 142, 35)),
        "CSS_ORANGE" => Some(rgb888_color(255, 165, 0)),
        "CSS_ORANGE_RED" => Some(rgb888_color(255, 69, 0)),
        "CSS_ORCHID" => Some(rgb888_color(218, 112, 214)),
        "CSS_PALE_GOLDENROD" => Some(rgb888_color(238, 232, 170)),
        "CSS_PALE_GREEN" => Some(rgb888_color(152, 251, 152)),
        "CSS_PALE_TURQUOISE" => Some(rgb888_color(175, 238, 238)),
        "CSS_PALE_VIOLET_RED" => Some(rgb888_color(219, 112, 147)),
        "CSS_PAPAYA_WHIP" => Some(rgb888_color(255, 239, 213)),
        "CSS_PEACH_PUFF" => Some(rgb888_color(255, 218, 185)),
        "CSS_PERU" => Some(rgb888_color(205, 133, 63)),
        "CSS_PINK" => Some(rgb888_color(255, 192, 203)),
        "CSS_PLUM" => Some(rgb888_color(221, 160, 221)),
        "CSS_POWDER_BLUE" => Some(rgb888_color(176, 224, 230)),
        "CSS_PURPLE" => Some(rgb888_color(128, 0, 128)),
        "CSS_REBECCAPURPLE" => Some(rgb888_color(102, 51, 153)),
        "CSS_RED" => Some(rgb888_color(255, 0, 0)),
        "CSS_ROSY_BROWN" => Some(rgb888_color(188, 143, 143)),
        "CSS_ROYAL_BLUE" => Some(rgb888_color(65, 105, 225)),
        "CSS_SADDLE_BROWN" => Some(rgb888_color(139, 69, 19)),
        "CSS_SALMON" => Some(rgb888_color(250, 128, 114)),
        "CSS_SANDY_BROWN" => Some(rgb888_color(244, 164, 96)),
        "CSS_SEA_GREEN" => Some(rgb888_color(46, 139, 87)),
        "CSS_SEASHELL" => Some(rgb888_color(255, 245, 238)),
        "CSS_SIENNA" => Some(rgb888_color(160, 82, 45)),
        "CSS_SILVER" => Some(rgb888_color(192, 192, 192)),
        "CSS_SKY_BLUE" => Some(rgb888_color(135, 206, 235)),
        "CSS_SLATE_BLUE" => Some(rgb888_color(106, 90, 205)),
        "CSS_SLATE_GRAY" => Some(rgb888_color(112, 128, 144)),
        "CSS_SNOW" => Some(rgb888_color(255, 250, 250)),
        "CSS_SPRING_GREEN" => Some(rgb888_color(0, 255, 127)),
        "CSS_STEEL_BLUE" => Some(rgb888_color(70, 130, 180)),
        "CSS_TAN" => Some(rgb888_color(210, 180, 140)),
        "CSS_TEAL" => Some(rgb888_color(0, 128, 128)),
        "CSS_THISTLE" => Some(rgb888_color(216, 191, 216)),
        "CSS_TOMATO" => Some(rgb888_color(255, 99, 71)),
        "CSS_TURQUOISE" => Some(rgb888_color(64, 224, 208)),
        "CSS_VIOLET" => Some(rgb888_color(238, 130, 238)),
        "CSS_WHEAT" => Some(rgb888_color(245, 222, 179)),
        "CSS_WHITE" => Some(rgb888_color(255, 255, 255)),
        "CSS_WHITE_SMOKE" => Some(rgb888_color(245, 245, 245)),
        "CSS_YELLOW" => Some(rgb888_color(255, 255, 0)),
        "CSS_YELLOW_GREEN" => Some(rgb888_color(154, 205, 50)),
        _ => None,
    }
}

fn parse_hex_color(line_number: usize, value: &str) -> Result<Color, String> {
    let red = parse_hex_byte(line_number, value, 1)?;
    let green = parse_hex_byte(line_number, value, 3)?;
    let blue = parse_hex_byte(line_number, value, 5)?;
    Ok(rgb888_color(red, green, blue))
}

fn parse_hex_byte(line_number: usize, value: &str, start: usize) -> Result<u8, String> {
    u8::from_str_radix(&value[start..start + 2], 16)
        .map_err(|_| format!("line {line_number}: invalid hex color `{value}`"))
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
