use embedded_graphics::prelude::Point;
use linkage_blaze_core::{
    DrawItem, DrawItemIter, LinkageFixed, Pose, Rgb888, WebColors, linkage, linkage_fixed,
};

pub const SCREEN_WIDTH: usize = 240;
pub const SCREEN_HEIGHT: usize = 320;

// Palette --------------------------------------------------------------------
// Deep blue/teal night background, a single warm "bone" color for the whole
// figure, dark-teal placards, and a muted cool color for the secondary top text.
pub const BACKGROUND: Rgb888 = Rgb888::CSS_MIDNIGHT_BLUE; // deep night blue (25, 25, 112)
pub const FIGURE_COLOR: Rgb888 = Rgb888::CSS_WHEAT; // warm pale bone-like tan (245, 222, 179)
pub const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE; // muted cool text (176, 196, 222)
const PLACARD_FILL: Rgb888 = Rgb888::new(25, 60, 70); // dark teal sign face

pub const TEXT_BAND_HEIGHT: i32 = 34;
pub const DANCE_TOP_LEFT: Point = Point::new(-68, -170);
pub const DANCE_WIDTH: usize = 375;
pub const DANCE_HEIGHT: usize = 500;
pub const DANCE_TILE_COLUMNS: usize = 3;
pub const DANCE_TILE_ROWS: usize = 4;
pub const DANCE_TILE_WIDTH: usize = 125;
pub const DANCE_TILE_HEIGHT: usize = 125;
pub const DANCE_TILE_PIXELS: usize = DANCE_TILE_WIDTH * DANCE_TILE_HEIGHT;
// Figure placement / size.  Centered horizontally on the 240px-wide screen and
// scaled up to fill most of the height below the top text band.  These three
// are the knobs to tune on-device if the figure clips an edge.
pub const DANCE_CENTER_X: i32 = 207;
pub const DANCE_BASELINE_Y: i32 = 480;
pub const DANCE_SCALE: f32 = 1.35;

// Figure stroke is a single fixed pixel width (reads less like debug line art).
const FIGURE_STROKE_PX: i32 = 3;
pub const SMALL_GLYPH_WIDTH: usize = 6;
pub const SMALL_GLYPH_HEIGHT: usize = 10;
// 12-hour clock text is right-justified to a fixed 11-char field ("12:04:32 PM")
// near the screen's right edge; single-digit hours get a leading space.
pub const TIME_TEXT_TOP_LEFT: Point = Point::new(166, 12);
pub const TIME_TEXT_WIDTH: usize = 72;
pub const WIFI_TEXT_TOP_LEFT: Point = Point::new(8, 12);

/// Format a 12-hour clock string with AM/PM, right-justified to 11 characters
/// (e.g. " 5:04:32 PM" or "12:04:32 PM").  No alloc.
pub fn format_clock_12h(hours: u8, minutes: u8, seconds: u8) -> heapless::String<16> {
    let hour12 = match hours % 12 {
        0 => 12,
        other => other,
    };
    let suffix = if hours % 24 < 12 { "AM" } else { "PM" };
    let mut text = heapless::String::new();
    let _ = core::fmt::write(
        &mut text,
        format_args!("{hour12:>2}:{minutes:02}:{seconds:02} {suffix}"),
    );
    text
}
pub const DANCE: LinkageFixed<3, 6, 400> = {
    const WITH_PEN: LinkageFixed<132, 6, 600> = LinkageFixed::<0, 0, 3>::start()
        .pen_width(3.5)
        .pen_color(Rgb888::CSS_IVORY)
        .combine(linkage_fixed!(
            "../../linkage-blaze-mocap/samples/pirouette.lb.rs",
            132,
            6,
            600
        ));
    WITH_PEN
        .freeze_param_name::<131>("l_shin_yrotation", 57.6)
        .freeze_param_name_at_default::<130>("abdomen_xrotation")
        .retain_param_names(&["head_yrotation", "l_shldr_zrotation", "r_shldr_zrotation"])
        .strip_fixed_noops::<400>()
        .merge_adjacent_fixed::<400>()
        .strip_fixed_noops::<400>()
};
const CLOCK_HAND_PARAM_TURN: f32 = 0.25;
const EYES_FORWARD_PARAM: f32 = 0.5;
const RIGHT_ARM_12_PARAM: f32 = 0.4375;
const LEFT_ARM_12_PARAM: f32 = 0.5625;

pub trait PixelTarget {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888);
}

pub trait DanceTileSink {
    fn draw_tile(&mut self, tile_flush: TileFlush, params: &[f32; 3], hours: u8, minutes: u8);
}

#[derive(Clone, Copy, Debug)]
pub struct DanceClock {
    params: [f32; 3],
    hours: u8,
    minutes: u8,
}

impl DanceClock {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            params: [0.5; 3],
            hours: 0,
            minutes: 0,
        }
    }

    #[must_use]
    pub fn from_time(hours: u8, minutes: u8, seconds: u8) -> Self {
        Self {
            params: dance_params(hours, minutes, seconds),
            hours,
            minutes,
        }
    }

    #[must_use]
    pub fn from_params(params: [f32; 3]) -> Self {
        Self {
            params,
            hours: 0,
            minutes: 0,
        }
    }

    #[must_use]
    pub const fn params(&self) -> &[f32; 3] {
        &self.params
    }

    pub fn draw_tiles<S: DanceTileSink>(&self, sink: &mut S) {
        let mut tile_row = 0;
        while tile_row < DANCE_TILE_ROWS {
            let mut tile_column = 0;
            while tile_column < DANCE_TILE_COLUMNS {
                let tile_x = tile_column * DANCE_TILE_WIDTH;
                let tile_y = tile_row * DANCE_TILE_HEIGHT;
                let tile_origin = Point::new(tile_x as i32, tile_y as i32);
                if let Some(tile_flush) =
                    TileFlush::new(tile_origin, DANCE_TILE_WIDTH, DANCE_TILE_HEIGHT)
                {
                    sink.draw_tile(tile_flush, &self.params, self.hours, self.minutes);
                }
                tile_column += 1;
            }
            tile_row += 1;
        }
    }
}

impl Default for DanceClock {
    fn default() -> Self {
        Self::new()
    }
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
        let visible_top = tile_top_left.y.max(TEXT_BAND_HEIGHT);
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
        wrap_param(EYES_FORWARD_PARAM + second_phase * CLOCK_HAND_PARAM_TURN),
        wrap_param(RIGHT_ARM_12_PARAM + minute_phase * CLOCK_HAND_PARAM_TURN),
        wrap_param(LEFT_ARM_12_PARAM + signed_hour_phase * CLOCK_HAND_PARAM_TURN),
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

pub fn render_tile<T: PixelTarget>(
    target: &mut T,
    params: &[f32; 3],
    tile_origin: Point,
    hours: u8,
    minutes: u8,
) {
    // Faint dial marks first, so the figure and placards draw over them.
    draw_dial(target, tile_origin);

    let dance_view = DANCE.view();
    let mut iter: DrawItemIter<3, 6> = dance_view.draw_items(params);
    // The figure is a single color: ignore each item's model color and stroke
    // width, and draw everything in FIGURE_COLOR at a fixed stroke width.
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

    // Hanging placards use the hand marks only as anchor points; they hang
    // straight down in screen coordinates and do not inherit hand rotation.
    let hour_display = if hours % 12 == 0 { 12 } else { hours % 12 };
    let right_hand_pose = iter
        .marked_pose("rMid2")
        .expect("rMid2 mark missing from DANCE");
    let left_hand_pose = iter
        .marked_pose("lMid2")
        .expect("lMid2 mark missing from DANCE");
    draw_hanging_placard(
        target,
        pose_to_point(left_hand_pose),
        tile_origin,
        hour_display as u32,
    );
    draw_hanging_placard(
        target,
        pose_to_point(right_hand_pose),
        tile_origin,
        minutes as u32,
    );
}

// Placard ("hanging sign") styling.  Fill is a darker, richer color than the
// background; border and text share the figure color so the signs read as part
// of the figure's world.
const PLACARD_BORDER: Rgb888 = FIGURE_COLOR;
const PLACARD_TEXT: Rgb888 = FIGURE_COLOR;
const DIGIT_W: i32 = 3;
const DIGIT_H: i32 = 5;
const DIGIT_SCALE: i32 = 2; // 3x5 cells become 6x10 px glyphs
const DIGIT_GAP: i32 = 2; // gap between the two digits of a placard
// Both placards are the same fixed size and always show two digits, so the hour
// ("05") and minute ("28") signs match.
const PLACARD_W: i32 = 34; // a touch wider so "00"/"34" aren't cramped
const PLACARD_H: i32 = 20;
const PLACARD_BORDER_PX: i32 = 2; // sign frame thickness
const HANGER_PX: i32 = 2; // hanger line thickness
const HANGER_HOOK: i32 = 7; // short vertical hook straight down from the hand
const HANGER_TRIANGLE: i32 = 22; // height of the triangle from hook apex to sign top
// Faint clock-face marks (12/3/6/9) drawn behind the figure.  The center is in
// SCREEN coordinates and converted to dance space in draw_dial (see DANCE_TOP_LEFT).
const DIAL_COLOR: Rgb888 = Rgb888::CSS_DARK_SLATE_GRAY; // muted teal-gray (47, 79, 79)
const DIAL_SCALE: i32 = 2;
const DIAL_CENTER_SCREEN: Point = Point::new(120, 178);
const DIAL_RADIUS_X: i32 = 100;
const DIAL_RADIUS_Y: i32 = 118;

// Each digit is 3×5 pixels, encoded as 15 bits (row-major, top-to-bottom, left-to-right).
#[rustfmt::skip]
const DIGIT_BITMAPS: [u16; 10] = [
    0b111_101_101_101_111, // 0
    0b010_110_010_010_111, // 1
    0b111_001_111_100_111, // 2
    0b111_001_111_001_111, // 3
    0b101_101_111_001_001, // 4
    0b111_100_111_001_111, // 5
    0b111_100_111_101_111, // 6
    0b111_001_001_001_001, // 7
    0b111_101_111_101_111, // 8
    0b111_101_111_001_111, // 9
];

fn draw_digit<T: PixelTarget>(
    target: &mut T,
    digit: u32,
    origin: Point,
    tile_origin: Point,
    color: Rgb888,
    scale: i32,
) {
    let bits = DIGIT_BITMAPS[(digit % 10) as usize];
    for row in 0..DIGIT_H {
        for col in 0..DIGIT_W {
            let bit = 14 - (row * DIGIT_W + col);
            if (bits >> bit) & 1 == 1 {
                for scale_y in 0..scale {
                    for scale_x in 0..scale {
                        put_pixel(
                            target,
                            origin.x + col * scale + scale_x,
                            origin.y + row * scale + scale_y,
                            tile_origin,
                            color,
                        );
                    }
                }
            }
        }
    }
}

/// Draw a 1- or 2-digit number centered on `center`, at the given pixel scale.
fn draw_number_centered<T: PixelTarget>(
    target: &mut T,
    number: u32,
    center: Point,
    tile_origin: Point,
    color: Rgb888,
    scale: i32,
) {
    let glyph_w = DIGIT_W * scale;
    let glyph_h = DIGIT_H * scale;
    let digit_count = if number >= 10 { 2 } else { 1 };
    let total_w = digit_count * glyph_w + (digit_count - 1) * DIGIT_GAP;
    let left = center.x - total_w / 2;
    let top = center.y - glyph_h / 2;
    if digit_count == 2 {
        draw_digit(
            target,
            number / 10,
            Point::new(left, top),
            tile_origin,
            color,
            scale,
        );
        draw_digit(
            target,
            number % 10,
            Point::new(left + glyph_w + DIGIT_GAP, top),
            tile_origin,
            color,
            scale,
        );
    } else {
        draw_digit(
            target,
            number,
            Point::new(left, top),
            tile_origin,
            color,
            scale,
        );
    }
}

/// Draw a hanging number sign anchored at `anchor` (a hand mark in screen
/// coordinates).  The sign is a fixed size and always shows two digits.  It
/// hangs straight down via a short vertical hook from the hand, then a triangle
/// splays out to the sign's two top corners.  The sign never rotates with the
/// hand.  `number` is shown modulo 100 (00–99).
fn draw_hanging_placard<T: PixelTarget>(
    target: &mut T,
    anchor: Point,
    tile_origin: Point,
    number: u32,
) {
    let card_left = anchor.x - PLACARD_W / 2;
    let card_top = anchor.y + HANGER_HOOK + HANGER_TRIANGLE;
    let card_right = card_left + PLACARD_W;

    // Short vertical hook straight down from the hand, then a triangle out to
    // the sign's top corners.
    let apex = Point::new(anchor.x, anchor.y + HANGER_HOOK);
    draw_segment(target, anchor, apex, tile_origin, PLACARD_BORDER, HANGER_PX);
    draw_segment(
        target,
        apex,
        Point::new(card_left, card_top),
        tile_origin,
        PLACARD_BORDER,
        HANGER_PX,
    );
    draw_segment(
        target,
        apex,
        Point::new(card_right, card_top),
        tile_origin,
        PLACARD_BORDER,
        HANGER_PX,
    );

    // Sign face then frame.
    fill_rect(
        target,
        card_left,
        card_top,
        PLACARD_W,
        PLACARD_H,
        tile_origin,
        PLACARD_FILL,
    );
    draw_rect_border(
        target,
        card_left,
        card_top,
        PLACARD_W,
        PLACARD_H,
        PLACARD_BORDER_PX,
        tile_origin,
        PLACARD_BORDER,
    );

    // Centered two-digit number (always padded, e.g. "05").
    let glyph_w = DIGIT_W * DIGIT_SCALE;
    let glyph_h = DIGIT_H * DIGIT_SCALE;
    let total_w = 2 * glyph_w + DIGIT_GAP;
    let text_left = card_left + (PLACARD_W - total_w) / 2;
    let text_top = card_top + (PLACARD_H - glyph_h) / 2;
    let value = number % 100;
    draw_digit(
        target,
        value / 10,
        Point::new(text_left, text_top),
        tile_origin,
        PLACARD_TEXT,
        DIGIT_SCALE,
    );
    draw_digit(
        target,
        value % 10,
        Point::new(text_left + glyph_w + DIGIT_GAP, text_top),
        tile_origin,
        PLACARD_TEXT,
        DIGIT_SCALE,
    );
}

/// Draw the faint clock-face marks (12 at top, 3 right, 6 bottom, 9 left) behind
/// the figure.  Dim and subtle so they support rather than compete.
fn draw_dial<T: PixelTarget>(target: &mut T, tile_origin: Point) {
    // Convert the screen-space dial center into dance space (the space that
    // pose_to_point and put_pixel use), which is offset by DANCE_TOP_LEFT.
    let center_x = DIAL_CENTER_SCREEN.x - DANCE_TOP_LEFT.x;
    let center_y = DIAL_CENTER_SCREEN.y - DANCE_TOP_LEFT.y;
    let top = Point::new(center_x, center_y - DIAL_RADIUS_Y);
    let bottom = Point::new(center_x, center_y + DIAL_RADIUS_Y);
    let right = Point::new(center_x + DIAL_RADIUS_X, center_y);
    let left = Point::new(center_x - DIAL_RADIUS_X, center_y);
    draw_number_centered(target, 12, top, tile_origin, DIAL_COLOR, DIAL_SCALE);
    draw_number_centered(target, 3, right, tile_origin, DIAL_COLOR, DIAL_SCALE);
    draw_number_centered(target, 6, bottom, tile_origin, DIAL_COLOR, DIAL_SCALE);
    draw_number_centered(target, 9, left, tile_origin, DIAL_COLOR, DIAL_SCALE);
}

fn fill_rect<T: PixelTarget>(
    target: &mut T,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    tile_origin: Point,
    color: Rgb888,
) {
    for dy in 0..height {
        for dx in 0..width {
            put_pixel(target, left + dx, top + dy, tile_origin, color);
        }
    }
}

fn draw_rect_border<T: PixelTarget>(
    target: &mut T,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    thickness: i32,
    tile_origin: Point,
    color: Rgb888,
) {
    for dy in 0..height {
        for dx in 0..width {
            let on_border = dx < thickness
                || dx >= width - thickness
                || dy < thickness
                || dy >= height - thickness;
            if on_border {
                put_pixel(target, left + dx, top + dy, tile_origin, color);
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
    // Brush spans `thickness` pixels, supporting both even and odd widths.
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
        // Paint a filled square of `thickness` pixels at each line-walk point.
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
