use embedded_graphics::prelude::Point;
use linkage_blaze_core::{LinkageFixed, Mat3, Pose, Rgb888, WebColors, linkage, linkage_fixed};

#[cfg(target_os = "none")]
use embedded_graphics::pixelcolor::IntoStorage;
#[cfg(target_os = "none")]
use linkage_blaze_cyd::{Cyd, CydFrame};

// todo000 this should be hard coded in the reader and then read a as const after that. It should not be here.
const BALLET_DOF: usize = 132;

// todo00 audit the existing numeric color backlog and add approximate color-name comments.
// todo000 every numeric color should have a comment telling what it is. (and named colors are better)
pub const BACKGROUND: Rgb888 = Rgb888::new(10, 28, 36); // very dark blue-green
const FIGURE_COLOR: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
pub const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;

// todo000 these could be OK, but there are a lot of them. Can't some be done via math?
pub const STATUS_BAND_HEIGHT: i32 = 20;
pub const BALLET_CENTER_X: i32 = 84;
pub const BALLET_BASELINE_Y: i32 = 300;
pub const BALLET_SCALE: f32 = 1.575;

// todo0000 interesting.
pub const BALLET: LinkageFixed<BALLET_DOF, 6, 540> = {
    const INNER: LinkageFixed<BALLET_DOF, 6, 538> =
        linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");
    LinkageFixed::<0, 0, 3>::start()
        .pen_color(FIGURE_COLOR)
        .pen_width(3.2)
        .combine(INNER)
};

// todo0000 review this
pub trait PixelTarget {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888);
}

#[cfg(target_os = "none")]
impl PixelTarget for CydFrame<'_> {
    fn width(&self) -> usize {
        self.width()
    }

    fn height(&self) -> usize {
        self.height()
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let stride = self.width();
        self.raw_pixels_mut()[y * stride + x] = Cyd::rgb565(color).into_storage();
    }
}

pub fn draw_segment<T: PixelTarget>(
    target: &mut T,
    start: Point,
    end: Point,
    color: Rgb888,
    width: f32,
) {
    let thickness = round_to_i32(width * BALLET_SCALE).max(1);
    let brush_low = -(thickness / 2);
    let brush_high = brush_low + thickness - 1;
    let mut current_x = start.x;
    let mut current_y = start.y;
    let delta_x = (end.x - start.x).abs();
    let delta_y = -(end.y - start.y).abs();
    let step_x = if start.x < end.x { 1 } else { -1 };
    let step_y = if start.y < end.y { 1 } else { -1 };
    let mut error = delta_x + delta_y;

    loop {
        let mut dy = brush_low;
        while dy <= brush_high {
            let mut dx = brush_low;
            while dx <= brush_high {
                put_pixel(target, current_x + dx, current_y + dy, color);
                dx += 1;
            }
            dy += 1;
        }
        if current_x == end.x && current_y == end.y {
            break;
        }
        let doubled_error = error * 2;
        if doubled_error >= delta_y {
            error += delta_y;
            current_x += step_x;
        }
        if doubled_error <= delta_x {
            error += delta_x;
            current_y += step_y;
        }
    }
}

pub fn draw_filled_circle<T: PixelTarget>(
    target: &mut T,
    center: Point,
    radius: f32,
    color: Rgb888,
) {
    let radius = round_to_i32(radius * BALLET_SCALE).max(1);
    for local_y in -radius..=radius {
        for local_x in -radius..=radius {
            if local_x * local_x + local_y * local_y <= radius * radius {
                put_pixel(target, center.x + local_x, center.y + local_y, color);
            }
        }
    }
}

/// Project a disk's orientation and radius into two screen-space half-axis vectors.
/// The ballet view looks along -X: screen_x ← -world_Y, screen_y ← -world_Z.
pub fn disk_screen_axes(orient: Mat3, radius: f32) -> ((f32, f32), (f32, f32)) {
    let axis_a = (-orient[1][0] * radius, -orient[2][0] * radius);
    let axis_b = (-orient[1][1] * radius, -orient[2][1] * radius);
    (axis_a, axis_b)
}

/// Fill an ellipse defined by two screen-space half-axis vectors from the center.
/// Skips degenerate (edge-on) disks whose projected area is zero.
pub fn draw_filled_ellipse<T: PixelTarget>(
    target: &mut T,
    center: Point,
    axis_a: (f32, f32),
    axis_b: (f32, f32),
    color: Rgb888,
) {
    let (ax, ay) = (axis_a.0 * BALLET_SCALE, axis_a.1 * BALLET_SCALE);
    let (bx, by) = (axis_b.0 * BALLET_SCALE, axis_b.1 * BALLET_SCALE);
    let det = ax * by - ay * bx;
    if det.abs() < 0.5 {
        return;
    }
    let inv_det = 1.0 / det;
    let bound_x = (ax.abs() + bx.abs()) as i32 + 1;
    let bound_y = (ay.abs() + by.abs()) as i32 + 1;
    for local_y in -bound_y..=bound_y {
        for local_x in -bound_x..=bound_x {
            let dx = local_x as f32;
            let dy = local_y as f32;
            let s = (by * dx - bx * dy) * inv_det;
            let t = (ax * dy - ay * dx) * inv_det;
            if s * s + t * t <= 1.0 {
                put_pixel(target, center.x + local_x, center.y + local_y, color);
            }
        }
    }
}

fn put_pixel<T: PixelTarget>(target: &mut T, x: i32, y: i32, color: Rgb888) {
    if x < 0 || y < 0 {
        return;
    }

    let x = x as usize;
    let y = y as usize;
    if x >= target.width() || y >= target.height() {
        return;
    }

    target.put_pixel(x, y, color);
}

pub fn pose_to_point(pose: Pose) -> Point {
    let position = pose.position();
    Point::new(
        BALLET_CENTER_X - round_to_i32(position[1] * BALLET_SCALE),
        BALLET_BASELINE_Y - round_to_i32(position[2] * BALLET_SCALE),
    )
}

fn round_to_i32(value: f32) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}
