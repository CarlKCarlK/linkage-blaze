use embedded_graphics::prelude::Point;
use linkage_blaze_core::{DrawItem, LinkageFixed, Pose, Rgb888, WebColors, linkage, linkage_fixed};

pub const SCREEN_WIDTH: usize = 240;
pub const SCREEN_HEIGHT: usize = 320;
pub const BG: Rgb888 = Rgb888::CSS_BLACK;
pub const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;
pub const DANCE_TOP_LEFT: Point = Point::new(-68, -170);
pub const DANCE_WIDTH: usize = 375;
pub const DANCE_HEIGHT: usize = 500;
pub const DANCE_TILE_COLUMNS: usize = 3;
pub const DANCE_TILE_ROWS: usize = 4;
pub const DANCE_TILE_WIDTH: usize = 125;
pub const DANCE_TILE_HEIGHT: usize = 125;
pub const DANCE_TILE_PIXELS: usize = DANCE_TILE_WIDTH * DANCE_TILE_HEIGHT;
pub const DANCE_CENTER_X: i32 = 188;
pub const DANCE_BASELINE_Y: i32 = 440;
pub const DANCE_SCALE: f32 = 1.05;
pub const SMALL_GLYPH_WIDTH: usize = 6;
pub const SMALL_GLYPH_HEIGHT: usize = 10;
pub const TIME_TEXT_TOP_LEFT: Point = Point::new(88, 12);
pub const WIFI_TEXT_TOP_LEFT: Point = Point::new(8, 12);
pub const DANCE: LinkageFixed<3, 4, 377> = linkage_fixed!("dance.lb.rs");
const PARAM_TURN: f32 = 0.25;
const MINUTE_PARAM_TURN: f32 = 0.15;
const EYES_FORWARD_PARAM: f32 = 0.5;
const RIGHT_ARM_12_PARAM: f32 = 0.4375;
const LEFT_ARM_12_PARAM: f32 = 0.5625;

pub trait PixelTarget {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888);
}

#[derive(Clone, Copy, Debug)]
pub struct TileFlush {
    pub top_left: Point,
    pub origin: Point,
    pub width: usize,
    pub height: usize,
}

impl TileFlush {
    pub fn new(tile_origin: Point, tile_width: usize, tile_height: usize) -> Option<Self> {
        let tile_top_left = Point::new(
            DANCE_TOP_LEFT.x + tile_origin.x,
            DANCE_TOP_LEFT.y + tile_origin.y,
        );
        let tile_bottom_right = Point::new(
            tile_top_left.x + tile_width as i32,
            tile_top_left.y + tile_height as i32,
        );
        let visible_left = tile_top_left.x.max(0);
        let visible_top = tile_top_left.y.max(0);
        let visible_right = tile_bottom_right.x.min(SCREEN_WIDTH as i32);
        let visible_bottom = tile_bottom_right.y.min(SCREEN_HEIGHT as i32);

        if visible_left >= visible_right || visible_top >= visible_bottom {
            return None;
        }

        Some(Self {
            top_left: Point::new(visible_left, visible_top),
            origin: Point::new(
                tile_origin.x + visible_left - tile_top_left.x,
                tile_origin.y + visible_top - tile_top_left.y,
            ),
            width: (visible_right - visible_left) as usize,
            height: (visible_bottom - visible_top) as usize,
        })
    }
}

#[must_use]
pub fn dance_params(hours: u8, minutes: u8, seconds: u8) -> [f32; 3] {
    let second_phase = seconds as f32 / 60.0;
    let minute_phase = (minutes as f32 + second_phase) / 60.0;
    let hour_phase = ((hours % 12) as f32 + minute_phase) / 12.0;
    let signed_hour_phase = signed_phase_from_twelve(hour_phase);
    [
        wrap_param(EYES_FORWARD_PARAM + second_phase * PARAM_TURN),
        wrap_param(RIGHT_ARM_12_PARAM + minute_phase * MINUTE_PARAM_TURN),
        wrap_param(LEFT_ARM_12_PARAM + signed_hour_phase * PARAM_TURN),
    ]
}

fn signed_phase_from_twelve(phase: f32) -> f32 {
    if phase > 0.5 { phase - 1.0 } else { phase }
}

fn wrap_param(value: f32) -> f32 {
    let mut value = value;
    while value >= 1.0 {
        value -= 1.0;
    }
    while value < 0.0 {
        value += 1.0;
    }
    value
}

pub fn render_tile<T: PixelTarget>(target: &mut T, params: &[f32; 3], tile_origin: Point) {
    for draw_item in DANCE.view().draw_items(params) {
        match draw_item {
            DrawItem::Stroke(stroke) => {
                draw_segment(
                    target,
                    pose_to_point(stroke.start()),
                    pose_to_point(stroke.end()),
                    tile_origin,
                    stroke.color(),
                );
            }
            DrawItem::Disk(disk) => {
                draw_filled_circle(
                    target,
                    pose_to_point(disk.pose()),
                    disk.radius(),
                    tile_origin,
                    disk.color(),
                );
            }
            DrawItem::Ring(ring) => {
                draw_ring(
                    target,
                    pose_to_point(ring.pose()),
                    ring.radius(),
                    ring.width(),
                    tile_origin,
                    ring.color(),
                );
            }
            DrawItem::Sphere(sphere) => {
                draw_filled_circle(
                    target,
                    pose_to_point(sphere.pose()),
                    sphere.radius(),
                    tile_origin,
                    sphere.color(),
                );
            }
        }
    }
}

fn draw_segment<T: PixelTarget>(
    target: &mut T,
    start: Point,
    end: Point,
    tile_origin: Point,
    color: Rgb888,
) {
    let mut current_x = start.x;
    let mut current_y = start.y;
    let delta_x = (end.x - start.x).abs();
    let delta_y = -(end.y - start.y).abs();
    let step_x = if start.x < end.x { 1 } else { -1 };
    let step_y = if start.y < end.y { 1 } else { -1 };
    let mut error = delta_x + delta_y;

    loop {
        put_pixel(target, current_x, current_y, tile_origin, color);
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

fn draw_filled_circle<T: PixelTarget>(
    target: &mut T,
    center: Point,
    radius: f32,
    tile_origin: Point,
    color: Rgb888,
) {
    let radius = round_to_i32(radius * DANCE_SCALE).max(1);
    for local_y in -radius..=radius {
        for local_x in -radius..=radius {
            if local_x * local_x + local_y * local_y <= radius * radius {
                put_pixel(
                    target,
                    center.x + local_x,
                    center.y + local_y,
                    tile_origin,
                    color,
                );
            }
        }
    }
}

fn draw_ring<T: PixelTarget>(
    target: &mut T,
    center: Point,
    radius: f32,
    width: f32,
    tile_origin: Point,
    color: Rgb888,
) {
    let radius = (radius * DANCE_SCALE).max(1.0);
    let width = (width * DANCE_SCALE).max(1.0);
    let outer = round_to_i32(radius + width * 0.5).max(1);
    let inner = round_to_i32((radius - width * 0.5).max(0.0));
    let outer_squared = outer * outer;
    let inner_squared = inner * inner;
    for local_y in -outer..=outer {
        for local_x in -outer..=outer {
            let distance_squared = local_x * local_x + local_y * local_y;
            if distance_squared <= outer_squared && distance_squared >= inner_squared {
                put_pixel(
                    target,
                    center.x + local_x,
                    center.y + local_y,
                    tile_origin,
                    color,
                );
            }
        }
    }
}

fn put_pixel<T: PixelTarget>(target: &mut T, x: i32, y: i32, tile_origin: Point, color: Rgb888) {
    let x = x - tile_origin.x;
    let y = y - tile_origin.y;
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
        DANCE_CENTER_X - round_to_i32(position[1] * DANCE_SCALE),
        DANCE_BASELINE_Y - round_to_i32(position[2] * DANCE_SCALE),
    )
}

fn round_to_i32(value: f32) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}
