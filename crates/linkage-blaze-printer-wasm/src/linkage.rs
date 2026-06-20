extern crate alloc;
use alloc::vec::Vec;

use embedded_graphics_core::pixelcolor::RgbColor;
use linkage_blaze_core::{DrawItem, Rgb888, linkage, linkage_fixed, LinkageFixed, Pose, Vec3};

// Cartesian printer kinematic chain: Z (gantry rise) → X (carriage) → Y (bed)
// Steps: PenColor, PenWidth, PenDown, Up(z), Mark, Move(220), Up(-z), Restore, PenUp, Move(x),
//        PenColor, Sphere, PenColor, PenWidth, PenDown, Left(y), PenUp, PenColor, Sphere
// N = 19 steps + Start = 20
const PRINTER: LinkageFixed<3, 20> = linkage_fixed!("linkages/printer.lb.rs");

/// Returns printer draw items encoded as flat `[type, x0,y0,z0, x1,y1,z1, r,g,b, size1, size2, ...]`.
///
/// Each item is 12 floats:
/// - type: 0 = Stroke, 1 = Sphere, 2 = Disk, 3 = Ring
/// - x0,y0,z0: position or stroke start
/// - x1,y1,z1: stroke end (0,0,0 for non-strokes)
/// - r,g,b: color (0–255)
/// - size1: stroke width or sphere/disk/ring radius
/// - size2: ring inner radius (0 for others)
pub fn draw_items_from(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    // Clamp to [0,1]: validate_params panics if any value is out of range.
    let params = [
        (x_mm / 220.0).clamp(0.0, 1.0),
        (y_mm / 220.0).clamp(0.0, 1.0),
        (z_mm / 250.0).clamp(0.0, 1.0),
    ];
    PRINTER
        .view()
        .draw_items(&params)
        .flat_map(|item| {
            let mut record = [0f32; 12];
            match item {
                DrawItem::Stroke(s) => {
                    record[0] = 0.0;
                    let [x, y, z] = s.start().position().into_array();
                    record[1] = x; record[2] = y; record[3] = z;
                    let [x, y, z] = s.end().position().into_array();
                    record[4] = x; record[5] = y; record[6] = z;
                    let c = s.color();
                    record[7] = c.r() as f32; record[8] = c.g() as f32; record[9] = c.b() as f32;
                    record[10] = s.width();
                }
                DrawItem::Sphere(s) => {
                    record[0] = 1.0;
                    let [x, y, z] = s.pose().position().into_array();
                    record[1] = x; record[2] = y; record[3] = z;
                    let c = s.color();
                    record[7] = c.r() as f32; record[8] = c.g() as f32; record[9] = c.b() as f32;
                    record[10] = s.radius();
                }
                DrawItem::Disk(d) => {
                    record[0] = 2.0;
                    let [x, y, z] = d.pose().position().into_array();
                    record[1] = x; record[2] = y; record[3] = z;
                    let c = d.color();
                    record[7] = c.r() as f32; record[8] = c.g() as f32; record[9] = c.b() as f32;
                    record[10] = d.radius();
                }
                DrawItem::Ring(r_item) => {
                    record[0] = 3.0;
                    let [x, y, z] = r_item.pose().position().into_array();
                    record[1] = x; record[2] = y; record[3] = z;
                    let c = r_item.color();
                    record[7] = c.r() as f32; record[8] = c.g() as f32; record[9] = c.b() as f32;
                    record[10] = r_item.radius();
                    record[11] = r_item.width();
                }
            }
            record
        })
        .collect()
}

/// Returns 4 poses as a flat `[x,y,z, ...]` array (12 floats total).
///
/// - `pts[0..3]`   pose 0 — frame origin `(0, 0, 0)`
/// - `pts[3..6]`   pose 1 — gantry height `(0, 0, z_mm)`
/// - `pts[6..9]`   pose 2 — X carriage `(x_mm, 0, z_mm)`
/// - `pts[9..12]`  pose 3 — nozzle tip `(x_mm, y_mm, z_mm)`
pub fn printer_points_from(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    let params = [x_mm / 220.0, y_mm / 220.0, z_mm / 250.0];
    PRINTER
        .view()
        .poses(&params)
        .map(Pose::position)
        .flat_map(Vec3::into_array)
        .collect()
}
