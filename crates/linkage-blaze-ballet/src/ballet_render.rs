use embedded_graphics::prelude::Point;
use linkage_blaze_core::{
    DrawItem, DrawItemIter, LinkageFixed, Pose, Rgb888, WebColors, linkage, linkage_fixed,
};

#[cfg(target_os = "none")]
use embedded_graphics::pixelcolor::IntoStorage;
#[cfg(target_os = "none")]
use linkage_blaze_cyd::{Cyd, CydFrame};

// todo000 this should be hard coded in the reader and then read a as const after that. It should not be here.
use crate::ballet_frames::BALLET_DOF;

// todo00 audit the existing numeric color backlog and add approximate color-name comments.
// todo000 every numeric color should have a comment telling what it is. (and named colors are better)
pub const BACKGROUND: Rgb888 = Rgb888::new(10, 28, 36); // very dark blue-green
pub const FIGURE_COLOR: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
pub const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;

// todo000 these could be OK, but there are a lot of them. Can't some be done via math?
pub const STATUS_BAND_HEIGHT: i32 = 20;
pub const BALLET_CENTER_X: i32 = 207;
pub const BALLET_BASELINE_Y: i32 = 480;
pub const BALLET_SCALE: f32 = 1.35;

// todo000 is this used anywhere? if so, why?
const FIGURE_STROKE_PX: i32 = 5;

pub const BALLET: LinkageFixed<BALLET_DOF, 6, 538> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");

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

// todo0000 (may no longer apply) review the old tile renderer replacement.
pub fn render_frame<T: PixelTarget>(target: &mut T, params: &[f32; BALLET_DOF]) {
    let ballet_view = BALLET.view();
    let mut iter: DrawItemIter<BALLET_DOF, 6> = ballet_view.draw_items(params);
    for draw_item in &mut iter {
        match draw_item {
            DrawItem::Stroke(stroke) => {
                draw_segment(
                    target,
                    pose_to_point(stroke.start()),
                    pose_to_point(stroke.end()),
                    FIGURE_COLOR,
                    FIGURE_STROKE_PX,
                );
            }
            DrawItem::Disk(disk) => {
                draw_filled_circle(
                    target,
                    pose_to_point(disk.pose()),
                    disk.radius(),
                    FIGURE_COLOR,
                );
            }
            DrawItem::Ring(ring) => {
                draw_ring(
                    target,
                    pose_to_point(ring.pose()),
                    ring.radius(),
                    ring.width(),
                    FIGURE_COLOR,
                );
            }
            DrawItem::Sphere(sphere) => {
                draw_filled_circle(
                    target,
                    pose_to_point(sphere.pose()),
                    sphere.radius(),
                    FIGURE_COLOR,
                );
            }
        }
    }
}

fn draw_segment<T: PixelTarget>(
    target: &mut T,
    start: Point,
    end: Point,
    color: Rgb888,
    thickness: i32,
) {
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

fn draw_filled_circle<T: PixelTarget>(target: &mut T, center: Point, radius: f32, color: Rgb888) {
    let radius = round_to_i32(radius * BALLET_SCALE).max(1);
    for local_y in -radius..=radius {
        for local_x in -radius..=radius {
            if local_x * local_x + local_y * local_y <= radius * radius {
                put_pixel(target, center.x + local_x, center.y + local_y, color);
            }
        }
    }
}

fn draw_ring<T: PixelTarget>(
    target: &mut T,
    center: Point,
    radius: f32,
    width: f32,
    color: Rgb888,
) {
    let radius = (radius * BALLET_SCALE).max(1.0);
    let width = (width * BALLET_SCALE).max(1.0);
    let outer = round_to_i32(radius + width * 0.5).max(1);
    let inner = round_to_i32((radius - width * 0.5).max(0.0));
    let outer_squared = outer * outer;
    let inner_squared = inner * inner;
    for local_y in -outer..=outer {
        for local_x in -outer..=outer {
            let distance_squared = local_x * local_x + local_y * local_y;
            if distance_squared <= outer_squared && distance_squared >= inner_squared {
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

fn pose_to_point(pose: Pose) -> Point {
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
