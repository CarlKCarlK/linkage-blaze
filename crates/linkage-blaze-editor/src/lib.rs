#![forbid(unsafe_code)]
//todo000000 need to update the editor to work with linkage![...], or switch to a simpler pattern of just including the .lb.rs file after LinkageFixed::start() --- IGNORE --- (may no longer apply)

use linkage_blaze_core::{DrawItem3d, LinkageBuf, RgbColor};
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
