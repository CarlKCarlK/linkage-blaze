#![forbid(unsafe_code)]
//todo000000 need to update the editor to work with linkage![...], or switch to a simpler pattern of just including the .lb.rs file after LinkageFixed::start() --- IGNORE --- (may no longer apply)

use linkage_blaze_core::{DrawItem3d, LinkageBuf, RgbColor};
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

#[wasm_bindgen]
pub fn default_program() -> String {
    include_str!("../../linkage-blaze-example-core/src/armatron/armatron1.lb.rs").into()
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
    let linkage = LinkageBuf::<256, 64>::from_lb_rs(source)?;
    let view = linkage.view();
    let mut params = [0.0; 256];
    let mut editor_params = Vec::new();

    for (param_index, param) in view.params().iter().enumerate() {
        if param.name().is_empty() {
            continue;
        }
        let value = overrides
            .iter()
            .find(|(name, _)| name == param.name())
            .map_or(param.default(), |(_, value)| value.clamp(0.0, 1.0));
        params[param_index] = value;
        editor_params.push(EditorParam {
            name: param.name().to_owned(),
            value,
        });
    }

    let mut primitives = Vec::new();
    for draw_item_3d in view.draw_items_3d(&params) {
        primitives.push(Primitive::from(draw_item_3d));
    }

    Ok(result_json(&primitives, &editor_params))
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

#[derive(Clone, Debug)]
struct EditorParam {
    name: String,
    value: f32,
}

#[derive(Clone, Copy, Debug)]
struct Color {
    red: f32,
    green: f32,
    blue: f32,
}

impl Color {
    fn from_rgb888(color: linkage_blaze_core::Rgb888) -> Self {
        Self {
            red: color.r() as f32 / 255.0,
            green: color.g() as f32 / 255.0,
            blue: color.b() as f32 / 255.0,
        }
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
    Sphere {
        center: Vec3,
        radius: f32,
        color: Color,
    },
}

impl From<DrawItem3d> for Primitive {
    fn from(draw_item_3d: DrawItem3d) -> Self {
        match draw_item_3d {
            DrawItem3d::Stroke(stroke) => Self::Segment {
                start: Vec3::from(stroke.start().position().into_array()),
                end: Vec3::from(stroke.end().position().into_array()),
                width: stroke.width(),
                color: Color::from_rgb888(stroke.color()),
            },
            DrawItem3d::Disk(disk) => Self::Disk {
                center: Vec3::from(disk.pose().position().into_array()),
                normal: Vec3::from(disk.pose().orientation().up().into_array()),
                radius: disk.radius(),
                width: 0.0,
                color: Color::from_rgb888(disk.color()),
            },
            DrawItem3d::Sphere(sphere) => Self::Sphere {
                center: Vec3::from(sphere.pose().position().into_array()),
                radius: sphere.radius(),
                color: Color::from_rgb888(sphere.color()),
            },
        }
    }
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

impl From<[f32; 3]> for Vec3 {
    fn from(value: [f32; 3]) -> Self {
        Self::new(value[0], value[1], value[2])
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
