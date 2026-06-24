extern crate alloc;
use alloc::vec::Vec;

use embedded_graphics_core::pixelcolor::RgbColor;
use linkage_blaze_core::{
    DrawItem, LinkageFixed, Pose, Rgb888, Vec3, WebColors, linkage, linkage_fixed,
};

const BUILD_X_MM: f32 = 220.0;
const BUILD_Y_MM: f32 = 240.0;
const BUILD_Z_MM: f32 = 250.0;

// Bed-slinger printer kinematic chain: Z (gantry rise) -> X (carriage) -> Y (bed).
// Fixed capacity for the printer linkage steps plus the implicit Start step.
const PRINTER: LinkageFixed<3, 5, 180> = linkage_fixed!("linkages/printer.lb.rs");

/// Returns printer draw items encoded as flat `[type, x0,y0,z0, x1,y1,z1, r,g,b, size1, size2, ...]`.
///
/// Each item is 12 floats:
/// - type: 0 = Stroke, 1 = Sphere, 2 = Disk
/// - x0,y0,z0: position or stroke start
/// - x1,y1,z1: stroke end (0,0,0 for non-strokes)
/// - r,g,b: color (0–255)
/// - size1: stroke width or sphere/disk radius
/// - size2: unused (0)
pub fn draw_items_from(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    let params = [x_mm / BUILD_X_MM, y_mm / BUILD_Y_MM, z_mm / BUILD_Z_MM];
    PRINTER
        .view()
        .draw_items(&params)
        .flat_map(|item| {
            let mut record = [0f32; 12];
            match item {
                DrawItem::Stroke(s) => {
                    record[0] = 0.0;
                    let [x, y, z] = s.start().position().into_array();
                    record[1] = x;
                    record[2] = y;
                    record[3] = z;
                    let [x, y, z] = s.end().position().into_array();
                    record[4] = x;
                    record[5] = y;
                    record[6] = z;
                    let c = s.color();
                    record[7] = c.r() as f32;
                    record[8] = c.g() as f32;
                    record[9] = c.b() as f32;
                    record[10] = s.width();
                }
                DrawItem::Sphere(s) => {
                    record[0] = 1.0;
                    let [x, y, z] = s.pose().position().into_array();
                    record[1] = x;
                    record[2] = y;
                    record[3] = z;
                    let c = s.color();
                    record[7] = c.r() as f32;
                    record[8] = c.g() as f32;
                    record[9] = c.b() as f32;
                    record[10] = s.radius();
                }
                DrawItem::Disk(d) => {
                    record[0] = 2.0;
                    let [x, y, z] = d.pose().position().into_array();
                    record[1] = x;
                    record[2] = y;
                    record[3] = z;
                    let c = d.color();
                    record[7] = c.r() as f32;
                    record[8] = c.g() as f32;
                    record[9] = c.b() as f32;
                    record[10] = d.radius();
                }
            }
            record
        })
        .collect()
}

/// Returns all printer linkage poses as a flat `[x,y,z, ...]` array.
///
/// In this bed-slinger model, X and Z move the nozzle while Y moves the bed.
pub fn printer_points_from(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    let params = [x_mm / BUILD_X_MM, y_mm / BUILD_Y_MM, z_mm / BUILD_Z_MM];
    PRINTER
        .view()
        .poses(&params)
        .map(Pose::position)
        .flat_map(Vec3::into_array)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mini_boat_extents_are_inside_printer_model_range() {
        let items = draw_items_from(158.0, 215.28, 65.0);
        assert!(!items.is_empty());
    }
}
