use embedded_graphics::prelude::Point;
use linkage_blaze_core::{
    DrawItem, DrawItemIter, LinkageFixed, Pose, Rgb888, WebColors, linkage, linkage_fixed,
};

// todo000 this should be hard coded in the reader and then read a as const after that. It should not be here.
use crate::ballet_frames::BALLET_DOF;

// todo000 these should be read from the cyd object, not be here.
pub const SCREEN_WIDTH: usize = 240;
pub const SCREEN_HEIGHT: usize = 320;

// todo000 every numeric color should have a comment telling what it is. (and named colors are better)
pub const BG: Rgb888 = Rgb888::new(10, 28, 36);
pub const FIGURE_COLOR: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
pub const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;

// todo000 these could be OK, but there are a lot of them. Can't some be done via math?
pub const STATUS_BAND_HEIGHT: i32 = 20;
pub const BALLET_TOP_LEFT: Point = Point::new(-68, -180);
pub const BALLET_WIDTH: usize = 375;
pub const BALLET_HEIGHT: usize = 500;
// todo0000 I thought we got rid of tiling in this app.
pub const BALLET_TILE_COLUMNS: usize = 3;
pub const BALLET_TILE_ROWS: usize = 4;
pub const BALLET_TILE_WIDTH: usize = 125;
pub const BALLET_TILE_HEIGHT: usize = 125;
pub const BALLET_TILE_PIXELS: usize = BALLET_TILE_WIDTH * BALLET_TILE_HEIGHT;
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

// todo0000 review this
pub trait BalletTileSink {
    fn draw_tile(&mut self, tile_flush: TileFlush, params: &[f32; BALLET_DOF]);
}

// todo0000 review this
#[derive(Clone, Copy, Debug)]
pub struct TileFlush {
    pub top_left: Point,
    pub origin: Point,
    pub width: usize,
    pub height: usize,
}

// todo0000 review this
impl TileFlush {
    pub fn new(tile_origin: Point, tile_width: usize, tile_height: usize) -> Option<Self> {
        let tile_top_left = Point::new(
            BALLET_TOP_LEFT.x + tile_origin.x,
            BALLET_TOP_LEFT.y + tile_origin.y,
        );
        let tile_bottom_right = Point::new(
            tile_top_left.x + tile_width as i32,
            tile_top_left.y + tile_height as i32,
        );
        let visible_left = tile_top_left.x.max(0);
        let visible_top = tile_top_left.y.max(STATUS_BAND_HEIGHT);
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

// todo0000 review this
pub fn draw_tiles<S: BalletTileSink>(params: &[f32; BALLET_DOF], sink: &mut S) {
    let mut tile_row = 0;
    while tile_row < BALLET_TILE_ROWS {
        let mut tile_column = 0;
        while tile_column < BALLET_TILE_COLUMNS {
            let tile_x = tile_column * BALLET_TILE_WIDTH;
            let tile_y = tile_row * BALLET_TILE_HEIGHT;
            let tile_origin = Point::new(tile_x as i32, tile_y as i32);
            if let Some(tile_flush) =
                TileFlush::new(tile_origin, BALLET_TILE_WIDTH, BALLET_TILE_HEIGHT)
            {
                sink.draw_tile(tile_flush, params);
            }
            tile_column += 1;
        }
        tile_row += 1;
    }
}

// todo0000 review this
pub fn render_tile<T: PixelTarget>(target: &mut T, params: &[f32; BALLET_DOF], tile_origin: Point) {
    let ballet_view = BALLET.view();
    let mut iter: DrawItemIter<BALLET_DOF, 6> = ballet_view.draw_items(params);
    for draw_item in &mut iter {
        match draw_item {
            DrawItem::Stroke(stroke) => {
                draw_segment(
                    target,
                    pose_to_point(stroke.start()),
                    pose_to_point(stroke.end()),
                    tile_origin,
                    FIGURE_COLOR,
                    FIGURE_STROKE_PX,
                );
            }
            DrawItem::Disk(disk) => {
                draw_filled_circle(
                    target,
                    pose_to_point(disk.pose()),
                    disk.radius(),
                    tile_origin,
                    FIGURE_COLOR,
                );
            }
            DrawItem::Ring(ring) => {
                draw_ring(
                    target,
                    pose_to_point(ring.pose()),
                    ring.radius(),
                    ring.width(),
                    tile_origin,
                    FIGURE_COLOR,
                );
            }
            DrawItem::Sphere(sphere) => {
                draw_filled_circle(
                    target,
                    pose_to_point(sphere.pose()),
                    sphere.radius(),
                    tile_origin,
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
    tile_origin: Point,
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
                put_pixel(target, current_x + dx, current_y + dy, tile_origin, color);
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

fn draw_filled_circle<T: PixelTarget>(
    target: &mut T,
    center: Point,
    radius: f32,
    tile_origin: Point,
    color: Rgb888,
) {
    let radius = round_to_i32(radius * BALLET_SCALE).max(1);
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
