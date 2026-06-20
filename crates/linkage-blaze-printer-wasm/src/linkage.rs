use linkage_blaze_core::{linkage, linkage_buf, LinkageBuf, Pose, Vec3};

fn printer_linkage() -> LinkageBuf<3> {
    linkage_buf!("linkages/printer.lb.rs", 3)
}

/// Returns the flat (x,y,z, x,y,z, ...) point array for a given toolhead position.
///
/// `x_mm`, `y_mm`, `z_mm` are absolute G-code coordinates (millimetres).
/// The printer build volume is 220 × 220 × 250 mm.
pub fn toolhead_points(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    let linkage = printer_linkage();
    let params = [x_mm / 220.0, y_mm / 220.0, z_mm / 250.0];
    linkage
        .view()
        .poses(&params)
        .map(Pose::position)
        .flat_map(Vec3::into_array)
        .collect()
}
