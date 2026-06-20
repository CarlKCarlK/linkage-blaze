extern crate alloc;
use alloc::vec::Vec;

use crate::printer::Segment;

#[derive(Debug, Clone)]
pub struct GCodeState {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub e: f32,
    pub f: f32,
    pub xyz_absolute: bool,
    pub e_absolute: bool,
}

impl Default for GCodeState {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            e: 0.0,
            f: 3000.0,
            xyz_absolute: true,
            e_absolute: true,
        }
    }
}

fn strip_comment(line: &str) -> &str {
    if let Some(pos) = line.find(';') {
        &line[..pos]
    } else {
        line
    }
}

fn parse_word(token: &str, letter: char) -> Option<f32> {
    let first = token.chars().next()?;
    if first.to_ascii_uppercase() == letter {
        token[1..].parse::<f32>().ok()
    } else {
        None
    }
}

fn parse_command(token: &str) -> Option<(char, u32)> {
    let letter = token.chars().next()?.to_ascii_uppercase();
    let number_str = token[1..].split('.').next()?;
    let number = number_str.parse::<u32>().ok()?;
    Some((letter, number))
}

pub fn parse_gcode(text: &str) -> Vec<Segment> {
    let mut state = GCodeState::default();
    let mut layer: u32 = 0;
    // Z-based layer detection is used only when the file has no slicer layer markers.
    let mut layer_z = f32::NEG_INFINITY;
    let mut has_slicer_markers = false;
    let mut segments = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Slicer layer markers (checked before stripping `;` comments)
        if let Some(rest) = trimmed.strip_prefix(";LAYER:") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                layer = n;
                has_slicer_markers = true;
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("; layer ") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                layer = n;
                has_slicer_markers = true;
            }
        }

        let code_part = strip_comment(line).trim();
        if code_part.is_empty() {
            continue;
        }

        let mut tokens = code_part.split_whitespace();
        let Some(first) = tokens.next() else { continue };
        let Some((cmd_letter, cmd_number)) = parse_command(first) else {
            continue;
        };

        match (cmd_letter, cmd_number) {
            ('G', 0) | ('G', 1) => {
                let prev_x = state.x;
                let prev_y = state.y;
                let prev_z = state.z;
                let prev_e = state.e;

                let mut new_x = state.x;
                let mut new_y = state.y;
                let mut new_z = state.z;
                let mut new_e = state.e;
                let mut has_xyz = false;
                let mut has_e = false;

                for token in tokens {
                    if let Some(value) = parse_word(token, 'X') {
                        new_x = if state.xyz_absolute {
                            value
                        } else {
                            state.x + value
                        };
                        has_xyz = true;
                    } else if let Some(value) = parse_word(token, 'Y') {
                        new_y = if state.xyz_absolute {
                            value
                        } else {
                            state.y + value
                        };
                        has_xyz = true;
                    } else if let Some(value) = parse_word(token, 'Z') {
                        new_z = if state.xyz_absolute {
                            value
                        } else {
                            state.z + value
                        };
                        has_xyz = true;
                    } else if let Some(value) = parse_word(token, 'E') {
                        new_e = if state.e_absolute {
                            value
                        } else {
                            state.e + value
                        };
                        has_e = true;
                    } else if let Some(value) = parse_word(token, 'F') {
                        state.f = value;
                    }
                }

                // Z-based layer detection fires only when the file has no slicer markers.
                if !has_slicer_markers && new_z > layer_z + 1e-4 {
                    layer += 1;
                    layer_z = new_z;
                }

                let extruding = cmd_number == 1 && has_e && new_e > prev_e + 1e-6;

                let moved = (new_x - prev_x).abs() > 1e-6
                    || (new_y - prev_y).abs() > 1e-6
                    || (new_z - prev_z).abs() > 1e-6;

                if moved && (has_xyz || has_e) {
                    segments.push(Segment {
                        x0: prev_x,
                        y0: prev_y,
                        z0: prev_z,
                        x1: new_x,
                        y1: new_y,
                        z1: new_z,
                        extruding,
                        layer,
                    });
                }

                state.x = new_x;
                state.y = new_y;
                state.z = new_z;
                state.e = new_e;
            }
            ('G', 90) => state.xyz_absolute = true,
            ('G', 91) => state.xyz_absolute = false,
            ('M', 82) => state.e_absolute = true,
            ('M', 83) => state.e_absolute = false,
            _ => {}
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_absolute_moves() {
        let gcode = "G90\nG1 X10 Y20 Z0.2 E1.0 F1500\nG1 X30 Y40 E3.0";
        let segments = parse_gcode(gcode);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].x0, 0.0);
        assert_eq!(segments[0].x1, 10.0);
        assert_eq!(segments[0].y1, 20.0);
        assert!(segments[0].extruding);
        assert_eq!(segments[1].x0, 10.0);
        assert_eq!(segments[1].x1, 30.0);
    }

    #[test]
    fn parses_relative_mode() {
        let gcode = "G91\nG1 X5 Y5 E1.0\nG1 X5 Y0 E1.0";
        let segments = parse_gcode(gcode);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].x1, 5.0);
        assert_eq!(segments[1].x0, 5.0);
        assert_eq!(segments[1].x1, 10.0);
    }

    #[test]
    fn travel_moves_not_extruding() {
        let gcode = "G0 X10 Y10\nG1 X20 Y20 E2.0";
        let segments = parse_gcode(gcode);
        assert_eq!(segments.len(), 2);
        assert!(!segments[0].extruding);
        assert!(segments[1].extruding);
    }

    #[test]
    fn layer_increments_on_z_increase() {
        let gcode = "G1 X10 Y10 Z0.2 E1.0\nG1 X20 Y20 Z0.4 E2.0\nG1 X30 Y30 Z0.4 E3.0";
        let segments = parse_gcode(gcode);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].layer, 1);
        assert_eq!(segments[1].layer, 2);
        assert_eq!(segments[2].layer, 2);
    }

    #[test]
    fn counts_extrusion_and_travel_segments() {
        let gcode = "\
G90\n\
G0 X0 Y0 Z0.2\n\
G1 X10 Y0 E1.0\n\
G0 X0 Y10\n\
G1 X10 Y10 E2.0\n\
";
        let segments = parse_gcode(gcode);
        let extrusion_count = segments.iter().filter(|seg| seg.extruding).count();
        let travel_count = segments.iter().filter(|seg| !seg.extruding).count();
        assert_eq!(extrusion_count, 2);
        assert_eq!(travel_count, 2);
    }

    #[test]
    fn slicer_layer_marker_overrides_z_detection() {
        let gcode = ";LAYER:0\nG1 X10 Y0 Z0.2 E1.0\n;LAYER:1\nG1 X20 Y0 Z0.4 E2.0";
        let segments = parse_gcode(gcode);
        assert_eq!(segments[0].layer, 0);
        assert_eq!(segments[1].layer, 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn parses_mini_boat_sample() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/mini_boat.gcode");
        let gcode = std::fs::read_to_string(path).expect("mini_boat.gcode missing");
        let segments = parse_gcode(&gcode);
        assert!(
            segments.len() > 10_000,
            "expected many segments, got {}",
            segments.len()
        );
        // All segment coordinates should be finite (no NaN/inf from parse bugs)
        for seg in &segments {
            assert!(seg.x1.is_finite() && seg.y1.is_finite() && seg.z1.is_finite());
        }
    }
}
