#![forbid(unsafe_code)]
//todo000000 need to update the editor to work with linkage![...], or switch to a simpler pattern of just including the .lb.rs file after LinkageFixed::start() --- IGNORE --- (may no longer apply)

use core::f32::consts::PI;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
pub fn default_program() -> String {
    include_str!("../../linkage-blaze-armatron-core/src/armatron1.lb.rs").into()
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

    let args = split_args(line_number, &line[open + 1..close])?;

    Ok(Some(MethodCall {
        name: name.to_owned(),
        args,
    }))
}

fn is_linkage_macro_wrapper(line: &str) -> bool {
    let line = line.trim_end_matches(';').trim();
    matches!(line, "linkage![" | "linkage! [" | "]")
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
            translate_turtle(
                turtle,
                primitives,
                turtle.pose.orientation.forward(),
                distance,
            );
        }
        "forward_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            let distance = parse_param_arg(line_number, method_call, params)?;
            translate_turtle(
                turtle,
                primitives,
                turtle.pose.orientation.forward(),
                distance,
            );
        }
        "left" => {
            expect_arg_count(line_number, method_call, 1)?;
            let distance = parse_number_arg(line_number, method_call, 0)?;
            translate_turtle(turtle, primitives, turtle.pose.orientation.left(), distance);
        }
        "left_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            let distance = parse_param_arg(line_number, method_call, params)?;
            translate_turtle(turtle, primitives, turtle.pose.orientation.left(), distance);
        }
        "up" => {
            expect_arg_count(line_number, method_call, 1)?;
            let distance = parse_number_arg(line_number, method_call, 0)?;
            translate_turtle(turtle, primitives, turtle.pose.orientation.up(), distance);
        }
        "up_param" => {
            expect_arg_count(line_number, method_call, 3)?;
            let distance = parse_param_arg(line_number, method_call, params)?;
            translate_turtle(turtle, primitives, turtle.pose.orientation.up(), distance);
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
        "mark" => {
            expect_arg_count(line_number, method_call, 1)?;
            let name = parse_identifier(line_number, method_call, 0)?;
            turtle.mark(name);
        }
        "restore" => {
            expect_arg_count(line_number, method_call, 1)?;
            let name = parse_identifier(line_number, method_call, 0)?;
            turtle.restore(name);
        }
        "disk" => {
            expect_arg_count(line_number, method_call, 1)?;
            let radius = parse_radius(line_number, method_call, 0)?;
            primitives.push(Primitive::Disk {
                center: turtle.pose.position,
                normal: turtle.pose.orientation.up(),
                radius,
                width: turtle.width,
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
                width: turtle.width,
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

fn translate_turtle(
    turtle: &mut Turtle,
    primitives: &mut Vec<Primitive>,
    axis: Vec3,
    distance: f32,
) {
    let start = turtle.pose.position;
    turtle.pose.position = turtle.pose.position + axis * distance;
    if turtle.pen == Pen::Down {
        primitives.push(Primitive::Segment {
            start,
            end: turtle.pose.position,
            width: turtle.width,
            color: turtle.color,
        });
    }
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

fn parse_identifier(
    line_number: usize,
    method_call: &MethodCall,
    arg_index: usize,
) -> Result<&'static str, String> {
    let value = parse_string_arg(line_number, method_call, arg_index)?;
    Ok(Box::leak(value.to_string().into_boxed_str()))
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

    if let Ok(parsed) = numeric_value.parse::<f32>() {
        if looks_like_integer(&numeric_value) {
            return Err(format!(
                "line {line_number}: `{method_name}` argument `{value}` is an integer; use `{value}.0`"
            ));
        }
        return Ok(parsed);
    }
    number_constant(value).ok_or_else(|| {
        format!("line {line_number}: `{method_name}` argument `{value}` is not a number or known constant")
    })
}

fn looks_like_integer(s: &str) -> bool {
    let digits = s
        .strip_prefix('-')
        .or_else(|| s.strip_prefix('+'))
        .unwrap_or(s);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
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

//todo00000 CSS_COLORS duplicates embedded-graphics web_colors.rs; revisit deriving this at build time
const CSS_COLORS: &[(&str, u8, u8, u8)] = &[
    ("Rgb888::CSS_ALICE_BLUE", 240, 248, 255),
    ("Rgb888::CSS_ANTIQUE_WHITE", 250, 235, 215),
    ("Rgb888::CSS_AQUA", 0, 255, 255),
    ("Rgb888::CSS_AQUAMARINE", 127, 255, 212),
    ("Rgb888::CSS_AZURE", 240, 255, 255),
    ("Rgb888::CSS_BEIGE", 245, 245, 220),
    ("Rgb888::CSS_BISQUE", 255, 228, 196),
    ("Rgb888::CSS_BLACK", 0, 0, 0),
    ("Rgb888::CSS_BLANCHED_ALMOND", 255, 235, 205),
    ("Rgb888::CSS_BLUE", 0, 0, 255),
    ("Rgb888::CSS_BLUE_VIOLET", 138, 43, 226),
    ("Rgb888::CSS_BROWN", 165, 42, 42),
    ("Rgb888::CSS_BURLY_WOOD", 222, 184, 135),
    ("Rgb888::CSS_CADET_BLUE", 95, 158, 160),
    ("Rgb888::CSS_CHARTREUSE", 127, 255, 0),
    ("Rgb888::CSS_CHOCOLATE", 210, 105, 30),
    ("Rgb888::CSS_CORAL", 255, 127, 80),
    ("Rgb888::CSS_CORNFLOWER_BLUE", 100, 149, 237),
    ("Rgb888::CSS_CORNSILK", 255, 248, 220),
    ("Rgb888::CSS_CRIMSON", 220, 20, 60),
    ("Rgb888::CSS_CYAN", 0, 255, 255),
    ("Rgb888::CSS_DARK_BLUE", 0, 0, 139),
    ("Rgb888::CSS_DARK_CYAN", 0, 139, 139),
    ("Rgb888::CSS_DARK_GOLDENROD", 184, 134, 11),
    ("Rgb888::CSS_DARK_GRAY", 169, 169, 169),
    ("Rgb888::CSS_DARK_GREEN", 0, 100, 0),
    ("Rgb888::CSS_DARK_KHAKI", 189, 183, 107),
    ("Rgb888::CSS_DARK_MAGENTA", 139, 0, 139),
    ("Rgb888::CSS_DARK_OLIVE_GREEN", 85, 107, 47),
    ("Rgb888::CSS_DARK_ORANGE", 255, 140, 0),
    ("Rgb888::CSS_DARK_ORCHID", 153, 50, 204),
    ("Rgb888::CSS_DARK_RED", 139, 0, 0),
    ("Rgb888::CSS_DARK_SALMON", 233, 150, 122),
    ("Rgb888::CSS_DARK_SEA_GREEN", 143, 188, 143),
    ("Rgb888::CSS_DARK_SLATE_BLUE", 72, 61, 139),
    ("Rgb888::CSS_DARK_SLATE_GRAY", 47, 79, 79),
    ("Rgb888::CSS_DARK_TURQUOISE", 0, 206, 209),
    ("Rgb888::CSS_DARK_VIOLET", 148, 0, 211),
    ("Rgb888::CSS_DEEP_PINK", 255, 20, 147),
    ("Rgb888::CSS_DEEP_SKY_BLUE", 0, 191, 255),
    ("Rgb888::CSS_DIM_GRAY", 105, 105, 105),
    ("Rgb888::CSS_DODGER_BLUE", 30, 144, 255),
    ("Rgb888::CSS_FIRE_BRICK", 178, 34, 34),
    ("Rgb888::CSS_FLORAL_WHITE", 255, 250, 240),
    ("Rgb888::CSS_FOREST_GREEN", 34, 139, 34),
    ("Rgb888::CSS_FUCHSIA", 255, 0, 255),
    ("Rgb888::CSS_GAINSBORO", 220, 220, 220),
    ("Rgb888::CSS_GHOST_WHITE", 248, 248, 255),
    ("Rgb888::CSS_GOLD", 255, 215, 0),
    ("Rgb888::CSS_GOLDENROD", 218, 165, 32),
    ("Rgb888::CSS_GRAY", 128, 128, 128),
    ("Rgb888::CSS_GREEN", 0, 128, 0),
    ("Rgb888::CSS_GREEN_YELLOW", 173, 255, 47),
    ("Rgb888::CSS_HONEYDEW", 240, 255, 240),
    ("Rgb888::CSS_HOT_PINK", 255, 105, 180),
    ("Rgb888::CSS_INDIAN_RED", 205, 92, 92),
    ("Rgb888::CSS_INDIGO", 75, 0, 130),
    ("Rgb888::CSS_IVORY", 255, 255, 240),
    ("Rgb888::CSS_KHAKI", 240, 230, 140),
    ("Rgb888::CSS_LAVENDER", 230, 230, 250),
    ("Rgb888::CSS_LAVENDER_BLUSH", 255, 240, 245),
    ("Rgb888::CSS_LAWN_GREEN", 124, 252, 0),
    ("Rgb888::CSS_LEMON_CHIFFON", 255, 250, 205),
    ("Rgb888::CSS_LIGHT_BLUE", 173, 216, 230),
    ("Rgb888::CSS_LIGHT_CORAL", 240, 128, 128),
    ("Rgb888::CSS_LIGHT_CYAN", 224, 255, 255),
    ("Rgb888::CSS_LIGHT_GOLDENROD_YELLOW", 250, 250, 210),
    ("Rgb888::CSS_LIGHT_GRAY", 211, 211, 211),
    ("Rgb888::CSS_LIGHT_GREEN", 144, 238, 144),
    ("Rgb888::CSS_LIGHT_PINK", 255, 182, 193),
    ("Rgb888::CSS_LIGHT_SALMON", 255, 160, 122),
    ("Rgb888::CSS_LIGHT_SEA_GREEN", 32, 178, 170),
    ("Rgb888::CSS_LIGHT_SKY_BLUE", 135, 206, 250),
    ("Rgb888::CSS_LIGHT_SLATE_GRAY", 119, 136, 153),
    ("Rgb888::CSS_LIGHT_STEEL_BLUE", 176, 196, 222),
    ("Rgb888::CSS_LIGHT_YELLOW", 255, 255, 224),
    ("Rgb888::CSS_LIME", 0, 255, 0),
    ("Rgb888::CSS_LIME_GREEN", 50, 205, 50),
    ("Rgb888::CSS_LINEN", 250, 240, 230),
    ("Rgb888::CSS_MAGENTA", 255, 0, 255),
    ("Rgb888::CSS_MAROON", 128, 0, 0),
    ("Rgb888::CSS_MEDIUM_AQUAMARINE", 102, 205, 170),
    ("Rgb888::CSS_MEDIUM_BLUE", 0, 0, 205),
    ("Rgb888::CSS_MEDIUM_ORCHID", 186, 85, 211),
    ("Rgb888::CSS_MEDIUM_PURPLE", 147, 112, 219),
    ("Rgb888::CSS_MEDIUM_SEA_GREEN", 60, 179, 113),
    ("Rgb888::CSS_MEDIUM_SLATE_BLUE", 123, 104, 238),
    ("Rgb888::CSS_MEDIUM_SPRING_GREEN", 0, 250, 154),
    ("Rgb888::CSS_MEDIUM_TURQUOISE", 72, 209, 204),
    ("Rgb888::CSS_MEDIUM_VIOLET_RED", 199, 21, 133),
    ("Rgb888::CSS_MIDNIGHT_BLUE", 25, 25, 112),
    ("Rgb888::CSS_MINT_CREAM", 245, 255, 250),
    ("Rgb888::CSS_MISTY_ROSE", 255, 228, 225),
    ("Rgb888::CSS_MOCCASIN", 255, 228, 181),
    ("Rgb888::CSS_NAVAJO_WHITE", 255, 222, 173),
    ("Rgb888::CSS_NAVY", 0, 0, 128),
    ("Rgb888::CSS_OLD_LACE", 253, 245, 230),
    ("Rgb888::CSS_OLIVE", 128, 128, 0),
    ("Rgb888::CSS_OLIVE_DRAB", 107, 142, 35),
    ("Rgb888::CSS_ORANGE", 255, 165, 0),
    ("Rgb888::CSS_ORANGE_RED", 255, 69, 0),
    ("Rgb888::CSS_ORCHID", 218, 112, 214),
    ("Rgb888::CSS_PALE_GOLDENROD", 238, 232, 170),
    ("Rgb888::CSS_PALE_GREEN", 152, 251, 152),
    ("Rgb888::CSS_PALE_TURQUOISE", 175, 238, 238),
    ("Rgb888::CSS_PALE_VIOLET_RED", 219, 112, 147),
    ("Rgb888::CSS_PAPAYA_WHIP", 255, 239, 213),
    ("Rgb888::CSS_PEACH_PUFF", 255, 218, 185),
    ("Rgb888::CSS_PERU", 205, 133, 63),
    ("Rgb888::CSS_PINK", 255, 192, 203),
    ("Rgb888::CSS_PLUM", 221, 160, 221),
    ("Rgb888::CSS_POWDER_BLUE", 176, 224, 230),
    ("Rgb888::CSS_PURPLE", 128, 0, 128),
    ("Rgb888::CSS_REBECCAPURPLE", 102, 51, 153),
    ("Rgb888::CSS_RED", 255, 0, 0),
    ("Rgb888::CSS_ROSY_BROWN", 188, 143, 143),
    ("Rgb888::CSS_ROYAL_BLUE", 65, 105, 225),
    ("Rgb888::CSS_SADDLE_BROWN", 139, 69, 19),
    ("Rgb888::CSS_SALMON", 250, 128, 114),
    ("Rgb888::CSS_SANDY_BROWN", 244, 164, 96),
    ("Rgb888::CSS_SEA_GREEN", 46, 139, 87),
    ("Rgb888::CSS_SEASHELL", 255, 245, 238),
    ("Rgb888::CSS_SIENNA", 160, 82, 45),
    ("Rgb888::CSS_SILVER", 192, 192, 192),
    ("Rgb888::CSS_SKY_BLUE", 135, 206, 235),
    ("Rgb888::CSS_SLATE_BLUE", 106, 90, 205),
    ("Rgb888::CSS_SLATE_GRAY", 112, 128, 144),
    ("Rgb888::CSS_SNOW", 255, 250, 250),
    ("Rgb888::CSS_SPRING_GREEN", 0, 255, 127),
    ("Rgb888::CSS_STEEL_BLUE", 70, 130, 180),
    ("Rgb888::CSS_TAN", 210, 180, 140),
    ("Rgb888::CSS_TEAL", 0, 128, 128),
    ("Rgb888::CSS_THISTLE", 216, 191, 216),
    ("Rgb888::CSS_TOMATO", 255, 99, 71),
    ("Rgb888::CSS_TURQUOISE", 64, 224, 208),
    ("Rgb888::CSS_VIOLET", 238, 130, 238),
    ("Rgb888::CSS_WHEAT", 245, 222, 179),
    ("Rgb888::CSS_WHITE", 255, 255, 255),
    ("Rgb888::CSS_WHITE_SMOKE", 245, 245, 245),
    ("Rgb888::CSS_YELLOW", 255, 255, 0),
    ("Rgb888::CSS_YELLOW_GREEN", 154, 205, 50),
];

#[wasm_bindgen]
pub fn known_colors() -> String {
    let mut out = String::from("[");
    for (i, &(name, _, _, _)) in CSS_COLORS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(name);
        out.push('"');
    }
    out.push(']');
    out
}

fn parse_color_arg(line_number: usize, method_call: &MethodCall) -> Result<Color, String> {
    let value = method_call.args[0].as_str();

    if let Some(color_args) = value
        .strip_prefix("Rgb888::new(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_rgb888_new_color(line_number, color_args);
    }

    for &(name, r, g, b) in CSS_COLORS {
        if name == value {
            return Ok(rgb888_color(r, g, b));
        }
    }

    Err(format!(
        "line {line_number}: unknown color `{value}`; use `Rgb888::CSS_*` or `Rgb888::new(r, g, b)`"
    ))
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
struct TurtleState {
    pose: Pose,
    pen: Pen,
    color: Color,
    width: f32,
}

struct Turtle {
    pose: Pose,
    pen: Pen,
    color: Color,
    width: f32,
    remembered: [(Option<&'static str>, TurtleState); 16],
    remembered_len: usize,
}

impl Turtle {
    fn new() -> Self {
        Self {
            pose: Pose::start(),
            pen: Pen::Down,
            color: Color::new(1.0, 1.0, 1.0),
            width: 0.1,
            remembered: [(
                None,
                TurtleState {
                    pose: Pose::start(),
                    pen: Pen::Down,
                    color: Color::new(1.0, 1.0, 1.0),
                    width: 0.1,
                },
            ); 16],
            remembered_len: 0,
        }
    }

    fn mark(&mut self, name: &'static str) {
        if self.remembered_len < 16 {
            self.remembered[self.remembered_len] = (
                Some(name),
                TurtleState {
                    pose: self.pose,
                    pen: self.pen,
                    color: self.color,
                    width: self.width,
                },
            );
            self.remembered_len += 1;
        }
    }

    fn restore(&mut self, name: &'static str) {
        let mut i = 0;
        while i < self.remembered_len {
            if let (Some(n), state) = self.remembered[i] {
                if n == name {
                    self.pose = state.pose;
                    self.pen = state.pen;
                    self.color = state.color;
                    self.width = state.width;
                    return;
                }
            }
            i += 1;
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
        width: f32,
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
                width,
                color,
            } => {
                json.push_str("{\"type\":\"disk\",\"center\":");
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

    fn left(self) -> Vec3 {
        Vec3::new(self.rows[0][1], self.rows[1][1], self.rows[2][1])
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
                r#"LinkageFixed::start()
.pen_color(Rgb888::new(245, 238, 210))
.disk(1.0)
"#,
                &[],
            )
            .is_ok()
        );
    }

    #[test]
    fn accepts_linkage_macro_wrapper() {
        let result = render_program(
            r#"linkage! [
    .define_param("x", 0.5)
    .forward_param("x", 0.0, 10.0)
    .pen_color(Rgb888::new(245, 238, 210))
    .disk(1.0)
]
"#,
            &[],
        );
        assert!(
            result.is_ok(),
            "linkage macro wrapper should be accepted: {result:?}"
        );
    }

    #[test]
    fn accepts_compact_linkage_macro_wrapper() {
        let result = render_program(
            r#"linkage![
    .forward(1.0)
]
"#,
            &[],
        );
        assert!(
            result.is_ok(),
            "compact linkage macro wrapper should be accepted: {result:?}"
        );
    }

    #[test]
    fn rejects_integer_args() {
        for bad in [
            ".forward(1)",
            ".yaw(90)",
            ".up(2)",
            ".define_param(\"x\", 1)",
        ] {
            let program = format!("LinkageFixed::start()\n{bad}\n");
            let result = render_program(&program, &[]);
            assert!(result.is_err(), "`{bad}` should be rejected as integer");
        }
    }

    #[test]
    fn accepts_float_args() {
        let result = render_program(
            "LinkageFixed::start()\n.forward(1.0)\n.yaw(90.0)\n.up(2.0)\n.define_param(\"x\", 1.0)\n",
            &[],
        );
        assert!(result.is_ok(), "floats should be accepted: {:?}", result);
    }

    #[test]
    fn rejects_non_rust_color_forms() {
        for color in ["white", "CSS_RED", "#ff0000"] {
            let program = format!(
                r#"LinkageFixed::start()
.pen_color({color})
.disk(1.0)
"#
            );
            assert!(render_program(&program, &[]).is_err());
        }
    }
}
