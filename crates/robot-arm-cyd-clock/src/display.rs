use core::fmt;

use cyd_esp32::{Cyd, CydError, DrawPrimitive, Ellipse, LineSegment, RectWorkspace, SCREEN_WIDTH};
use embedded_graphics::{
    Drawable,
    mono_font::{
        MonoFont, MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::{Rgb565, RgbColor, raw::RawU16},
    prelude::Point,
    primitives::Rectangle,
    text::{Baseline, Text},
};
use robot_arm_core::{DiskItem, DrawItem, Linkage, Pose, RingItem};
use static_cell::StaticCell;

const SMALL_GLYPH_WIDTH: usize = 6;
const SMALL_GLYPH_HEIGHT: usize = 10;
const MAIN_GLYPH_WIDTH: usize = 10;
const MAIN_GLYPH_HEIGHT: usize = 20;
const MAIN_GLYPH_SCALE: usize = 2;
const MAX_TIME_CHARS: usize = 8; // "12:59 PM"
const MAX_TIME_DISPLAY_WIDTH: usize = MAX_TIME_CHARS * MAIN_GLYPH_WIDTH * MAIN_GLYPH_SCALE;
const TIME_TEXT_Y: i32 = 34;
const GLYPH_WORKSPACE_WIDTH: usize = MAIN_GLYPH_WIDTH * MAIN_GLYPH_SCALE;
const GLYPH_WORKSPACE_HEIGHT: usize = MAIN_GLYPH_HEIGHT * MAIN_GLYPH_SCALE;
const CLOCK_BUFFER_WIDTH: usize = 180;
const CLOCK_BUFFER_HEIGHT: usize = 158;
const GLYPH_WORKSPACE_PIXELS: usize = GLYPH_WORKSPACE_WIDTH * GLYPH_WORKSPACE_HEIGHT;
const CLOCK_TOP_LEFT: Point = Point::new(70, 82);
const CLOCK_CENTER_X: i32 = 90;
const CLOCK_CENTER_Y: i32 = 79;
const HAND_SCALE: f32 = 1.0;
const HOUR_PARAM: usize = 0;
const MINUTE_PARAM: usize = 1;
const SECOND_PARAM: usize = 2;
const BG: Rgb565 = Rgb565::new(31, 59, 27);
const FACE_FILL: u32 = rgb565_raw(2, 10, 24);
const TICK_MAJOR_COLOR: u32 = rgb565_raw(31, 62, 30);
const TICK_WIDTH: u16 = 3;
const TICK_INNER_RADIUS: f32 = 58.0;
const TICK_LENGTH: f32 = 10.0;
const TEXT_DIM: Rgb565 = Rgb565::new(1, 8, 16);
const TEXT_MAIN: Rgb565 = Rgb565::new(1, 8, 16);
const TEXT_OK: Rgb565 = Rgb565::new(1, 8, 16);
const HUB: Rgb565 = Rgb565::new(31, 62, 30);
const HOUR_HAND_COLOR: u32 = rgb565_raw(31, 62, 30);
const MINUTE_HAND_COLOR: u32 = rgb565_raw(12, 50, 31);
const SECOND_HAND_COLOR: u32 = rgb565_raw(31, 10, 6);
const HOUR_LENGTH: f32 = 38.0;
const MINUTE_LENGTH: f32 = 58.0;
const SECOND_LENGTH: f32 = 66.0;
const HOUR_WIDTH: u16 = 8;
const MINUTE_WIDTH: u16 = 5;
const SECOND_WIDTH: u16 = 2;
const FACE_FILL_RADIUS: u16 = 72;
const HUB_RADIUS: u16 = 6;
const HAND_ITEM_COUNT: usize = 8; // 1 face disk + 3 clock hands + 4 tick marks
const HUB_COUNT: usize = 1;
const CLOCK_PRIMITIVE_COUNT: usize = HAND_ITEM_COUNT + HUB_COUNT;
const CLOCK_BOUNDS: Rectangle = Rectangle::new(
    CLOCK_TOP_LEFT,
    embedded_graphics::prelude::Size::new(CLOCK_BUFFER_WIDTH as u32, CLOCK_BUFFER_HEIGHT as u32),
);
const CLOCK_HANDS: Linkage<3, 60> = Linkage::start()
    .pen_color(FACE_FILL)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .disk(FACE_FILL_RADIUS as f32)
    .restart()
    .pen_color(HOUR_HAND_COLOR)
    .pen_width(HOUR_WIDTH)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .yaw_param(HOUR_PARAM, -90.0, 270.0)
    .forward(HOUR_LENGTH)
    .restart()
    .pen_color(MINUTE_HAND_COLOR)
    .pen_width(MINUTE_WIDTH)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .yaw_param(MINUTE_PARAM, -90.0, 270.0)
    .forward(MINUTE_LENGTH)
    .restart()
    .pen_color(SECOND_HAND_COLOR)
    .pen_width(SECOND_WIDTH)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .yaw_param(SECOND_PARAM, -90.0, 270.0)
    .forward(SECOND_LENGTH)
    .restart()
    .pen_color(TICK_MAJOR_COLOR)
    .pen_width(0)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .yaw(-90.0)
    .forward(TICK_INNER_RADIUS)
    .pen_width(TICK_WIDTH)
    .forward(TICK_LENGTH)
    .restart()
    .pen_color(TICK_MAJOR_COLOR)
    .pen_width(0)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .yaw(0.0)
    .forward(TICK_INNER_RADIUS)
    .pen_width(TICK_WIDTH)
    .forward(TICK_LENGTH)
    .restart()
    .pen_color(TICK_MAJOR_COLOR)
    .pen_width(0)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .yaw(90.0)
    .forward(TICK_INNER_RADIUS)
    .pen_width(TICK_WIDTH)
    .forward(TICK_LENGTH)
    .restart()
    .pen_color(TICK_MAJOR_COLOR)
    .pen_width(0)
    .roll_param(SECOND_PARAM, 0.0, 360.0)
    .yaw(180.0)
    .forward(TICK_INNER_RADIUS)
    .pen_width(TICK_WIDTH)
    .forward(TICK_LENGTH);

type GlyphWorkspace = RectWorkspace<GLYPH_WORKSPACE_PIXELS>;

const fn rgb565_raw(red: u32, green: u32, blue: u32) -> u32 {
    (red << 11) | (green << 5) | blue
}

pub enum CydClockDisplayError {
    Cyd(CydError),
}

impl fmt::Debug for CydClockDisplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CydClockDisplayError::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
        }
    }
}

impl From<CydError> for CydClockDisplayError {
    fn from(error: CydError) -> Self {
        Self::Cyd(error)
    }
}

pub struct CydClockDisplay {
    cyd: Cyd,
    glyph_workspace: &'static mut GlyphWorkspace,
    background_cleared: bool,
    last_time_text: heapless::String<16>,
}

impl CydClockDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static GLYPH_WORKSPACE: StaticCell<GlyphWorkspace> = StaticCell::new();

        Self {
            cyd,
            glyph_workspace: GlyphWorkspace::init_static(&GLYPH_WORKSPACE),
            background_cleared: false,
            last_time_text: heapless::String::new(),
        }
    }

    pub fn show(
        &mut self,
        wifi_mode: &str,
        clock_time: Option<&ClockTime>,
    ) -> Result<(), CydClockDisplayError> {
        if !self.background_cleared {
            self.cyd.clear_now(BG)?;
            self.background_cleared = true;
        }

        let time_text = clock_time.map_or("--:--", ClockTime::as_str);

        let mut wifi_text = heapless::String::<32>::new();
        fmt::Write::write_fmt(
            &mut wifi_text,
            format_args!("WiFi {}", wifi_label(wifi_mode)),
        )
        .ok();
        self.show_small_text_line("CYD Clock", TEXT_DIM, Point::new(14, 8), 96)?;
        self.show_small_text_line(wifi_text.as_str(), TEXT_OK, Point::new(240, 8), 70)?;
        if time_text != self.last_time_text.as_str() {
            self.show_main_text_line(time_text, TEXT_MAIN)?;
            self.last_time_text.clear();
            self.last_time_text.push_str(time_text).ok();
        }

        self.show_clock(clock_time)?;

        Ok(())
    }

    fn show_text_line(
        &mut self,
        text: &str,
        top_left: Point,
        font: &'static MonoFont<'static>,
        glyph_width: usize,
        glyph_height: usize,
        scale: usize,
        color: Rgb565,
    ) -> Result<(), CydClockDisplayError> {
        let flush_width = glyph_width * scale;
        let flush_height = glyph_height * scale;
        let mut glyph_left = top_left.x;

        for character in text.chars() {
            let mut character_text = heapless::String::<4>::new();
            fmt::Write::write_char(&mut character_text, character).ok();
            let mut glyph_buffer = self.glyph_workspace.view_mut(flush_width, flush_height);
            glyph_buffer.clear(BG);
            Text::with_baseline(
                character_text.as_str(),
                Point::new(0, 0),
                MonoTextStyle::new(font, color),
                Baseline::Top,
            )
            .draw(&mut glyph_buffer)
            .ok();
            if scale > 1 {
                scale_glyph_in_place(&mut glyph_buffer, glyph_width, glyph_height, scale);
            }
            self.cyd
                .flush(&glyph_buffer, Point::new(glyph_left, top_left.y))?;
            glyph_left += flush_width as i32;
        }

        Ok(())
    }

    fn show_small_text_line(
        &mut self,
        text: &str,
        color: Rgb565,
        top_left: Point,
        width: usize,
    ) -> Result<(), CydClockDisplayError> {
        self.clear_text_rect(top_left, width, SMALL_GLYPH_HEIGHT)?;
        self.show_text_line(
            text,
            top_left,
            &FONT_6X10,
            SMALL_GLYPH_WIDTH,
            SMALL_GLYPH_HEIGHT,
            1,
            color,
        )
    }

    fn show_main_text_line(
        &mut self,
        text: &str,
        color: Rgb565,
    ) -> Result<(), CydClockDisplayError> {
        let mut padded = heapless::String::<16>::new();
        padded.push_str(text).ok();
        while padded.chars().count() < MAX_TIME_CHARS {
            padded.push(' ').ok();
        }
        let x = (SCREEN_WIDTH as i32 - MAX_TIME_DISPLAY_WIDTH as i32) / 2;
        self.show_text_line(
            padded.as_str(),
            Point::new(x, TIME_TEXT_Y),
            &FONT_10X20,
            MAIN_GLYPH_WIDTH,
            MAIN_GLYPH_HEIGHT,
            MAIN_GLYPH_SCALE,
            color,
        )
    }

    fn clear_text_rect(
        &mut self,
        top_left: Point,
        width: usize,
        height: usize,
    ) -> Result<(), CydClockDisplayError> {
        self.cyd.fill_rect_now(
            Rectangle::new(
                top_left,
                embedded_graphics::prelude::Size::new(width as u32, height as u32),
            ),
            BG,
        )?;
        Ok(())
    }

    fn show_clock(&mut self, clock_time: Option<&ClockTime>) -> Result<(), CydClockDisplayError> {
        let mut primitives = [empty_primitive(); CLOCK_PRIMITIVE_COUNT];
        let mut primitive_count = 0;
        let params = clock_time.map_or([0.0; 3], |t| t.params());
        draw_clock_hands(&params, &mut primitives, &mut primitive_count);
        draw_clock_hub(&mut primitives, &mut primitive_count);
        self.cyd
            .draw_primitives_now(CLOCK_BOUNDS, BG, &primitives[..primitive_count])?;
        Ok(())
    }
}

fn empty_primitive() -> DrawPrimitive {
    DrawPrimitive::LineSegment(LineSegment {
        start: Point::new(0, 0),
        end: Point::new(0, 0),
        width: 0,
        color: Rgb565::BLACK,
    })
}

fn wifi_label(wifi_mode: &str) -> &str {
    match wifi_mode {
        "connected" => "OK",
        "connecting" => "...",
        "connect failed" => "fail",
        "setup CydClock" => "setup",
        _ => wifi_mode,
    }
}

fn scale_glyph_in_place(
    glyph_buffer: &mut cyd_esp32::RectView<'_>,
    glyph_width: usize,
    glyph_height: usize,
    scale: usize,
) {
    let scaled_width = glyph_width * scale;
    let pixels = glyph_buffer.raw_pixels_mut();

    for source_y in (0..glyph_height).rev() {
        for source_x in (0..glyph_width).rev() {
            let color = pixels[source_y * scaled_width + source_x];
            let scaled_x = source_x * scale;
            let scaled_y = source_y * scale;
            for offset_y in 0..scale {
                for offset_x in 0..scale {
                    pixels[(scaled_y + offset_y) * scaled_width + scaled_x + offset_x] = color;
                }
            }
        }
    }
}

fn draw_clock_hands(
    params: &[f32; 3],
    primitives: &mut [DrawPrimitive; CLOCK_PRIMITIVE_COUNT],
    primitive_count: &mut usize,
) {
    for draw_item in CLOCK_HANDS.draw_items(params) {
        match draw_item {
            DrawItem::Stroke(stroke) => {
                if stroke.width() == 0 {
                    continue;
                }
                let start = pose_to_point(stroke.start());
                let end = pose_to_point(stroke.end());
                if start != end {
                    primitives[*primitive_count] = DrawPrimitive::LineSegment(LineSegment {
                        start,
                        end,
                        width: stroke.width(),
                        color: Rgb565::from(RawU16::new(stroke.color() as u16)),
                    });
                    *primitive_count += 1;
                }
            }
            DrawItem::Disk(disk) => {
                primitives[*primitive_count] = DrawPrimitive::Ellipse(disk_to_ellipse(disk));
                *primitive_count += 1;
            }
            DrawItem::Ring(ring) => {
                primitives[*primitive_count] = DrawPrimitive::Ellipse(ring_to_ellipse(ring));
                *primitive_count += 1;
            }
        }
    }
}

fn draw_clock_hub(
    primitives: &mut [DrawPrimitive; CLOCK_PRIMITIVE_COUNT],
    primitive_count: &mut usize,
) {
    let r = HUB_RADIUS as f32;
    primitives[*primitive_count] = DrawPrimitive::Ellipse(Ellipse {
        center: clock_point(Point::new(CLOCK_CENTER_X, CLOCK_CENTER_Y)),
        axis_a: (r, 0.0),
        axis_b: (0.0, r),
        radius: r,
        stroke_width: 0,
        color: HUB,
        filled: true,
    });
    *primitive_count += 1;
}

fn disk_to_ellipse(disk: DiskItem) -> Ellipse {
    let pos = disk.pose().position();
    let center = clock_point(Point::new(
        CLOCK_CENTER_X + pos[0] as i32,
        CLOCK_CENTER_Y + pos[1] as i32,
    ));
    let orient = disk.pose().orientation();
    let r = disk.radius();
    Ellipse {
        center,
        axis_a: (orient[0][0] * r, orient[1][0] * r),
        axis_b: (orient[0][1] * r, orient[1][1] * r),
        radius: r,
        stroke_width: 0,
        color: Rgb565::from(RawU16::new(disk.color() as u16)),
        filled: true,
    }
}

fn ring_to_ellipse(ring: RingItem) -> Ellipse {
    let pos = ring.pose().position();
    let center = clock_point(Point::new(
        CLOCK_CENTER_X + pos[0] as i32,
        CLOCK_CENTER_Y + pos[1] as i32,
    ));
    let orient = ring.pose().orientation();
    let r = ring.radius();
    Ellipse {
        center,
        axis_a: (orient[0][0] * r, orient[1][0] * r),
        axis_b: (orient[0][1] * r, orient[1][1] * r),
        radius: r,
        stroke_width: ring.width(),
        color: Rgb565::from(RawU16::new(ring.color() as u16)),
        filled: false,
    }
}

fn pose_to_point(pose: Pose) -> Point {
    let position = pose.position();
    clock_point(Point::new(
        CLOCK_CENTER_X + (position[0] * HAND_SCALE) as i32,
        CLOCK_CENTER_Y + (position[1] * HAND_SCALE) as i32,
    ))
}

fn clock_point(point: Point) -> Point {
    Point::new(CLOCK_TOP_LEFT.x + point.x, CLOCK_TOP_LEFT.y + point.y)
}

pub struct ClockTime {
    text: heapless::String<16>,
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl ClockTime {
    pub fn new(hours: u8, minutes: u8, seconds: u8) -> Result<Self, fmt::Error> {
        let mut text = heapless::String::<16>::new();
        let meridiem = if hours < 12 { "AM" } else { "PM" };
        let hours12 = match hours % 12 {
            0 => 12,
            hours12 => hours12,
        };
        fmt::Write::write_fmt(
            &mut text,
            format_args!("{}:{:02} {}", hours12, minutes, meridiem),
        )?;
        Ok(Self {
            text,
            hours,
            minutes,
            seconds,
        })
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn params(&self) -> [f32; 3] {
        let second = self.seconds as f32 / 60.0;
        let minute = (self.minutes as f32 + second) / 60.0;
        let hour = ((self.hours % 12) as f32 + minute) / 12.0;
        [hour, minute, second]
    }
}
