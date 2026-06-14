use core::fmt;

use cyd_esp32::{Circle, Cyd, CydError, DrawPrimitive, LineSegment, RectWorkspace};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::{Rgb565, RgbColor, raw::RawU16},
    prelude::Point,
    primitives::Rectangle,
    text::{Baseline, Text},
};
use robot_arm_core::{Linkage, Pose};
use static_cell::StaticCell;

const TEXT_LINE_WIDTH: usize = 240;
const TEXT_LINE_HEIGHT: usize = 20;
const CLOCK_BUFFER_WIDTH: usize = 160;
const CLOCK_BUFFER_HEIGHT: usize = 160;
const TEXT_LINE_PIXELS: usize = TEXT_LINE_WIDTH * TEXT_LINE_HEIGHT;
const CLOCK_TOP_LEFT: Point = Point::new(80, 80);
const CLOCK_CENTER_X: i32 = 80;
const CLOCK_CENTER_Y: i32 = 80;
const HAND_SCALE: f32 = 1.0;
const HOUR_PARAM: usize = 0;
const MINUTE_PARAM: usize = 1;
const SECOND_PARAM: usize = 2;
const HOUR_HAND_COLOR: u32 = 0x07E0;
const MINUTE_HAND_COLOR: u32 = 0xFFE0;
const SECOND_HAND_COLOR: u32 = 0xF800;
const FACE_RADIUS: u16 = 74;
const FACE_STROKE_WIDTH: u16 = 2;
const FACE_PRIMITIVE_COUNT: usize = 1;
const HAND_SEGMENT_COUNT: usize = 3;
const CLOCK_PRIMITIVE_COUNT: usize = FACE_PRIMITIVE_COUNT + HAND_SEGMENT_COUNT;
const CLOCK_BOUNDS: Rectangle = Rectangle::new(
    CLOCK_TOP_LEFT,
    embedded_graphics::prelude::Size::new(CLOCK_BUFFER_WIDTH as u32, CLOCK_BUFFER_HEIGHT as u32),
);
const CLOCK_HANDS: Linkage<3, 15> = Linkage::start()
    .pen_color(HOUR_HAND_COLOR)
    .pen_width(8)
    .yaw_param(HOUR_PARAM, -90.0, 270.0)
    .forward(42.0)
    .restart()
    .pen_color(MINUTE_HAND_COLOR)
    .pen_width(4)
    .yaw_param(MINUTE_PARAM, -90.0, 270.0)
    .forward(64.0)
    .restart()
    .pen_color(SECOND_HAND_COLOR)
    .pen_width(2)
    .yaw_param(SECOND_PARAM, -90.0, 270.0)
    .forward(72.0);

type TextWorkspace = RectWorkspace<TEXT_LINE_PIXELS>;

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
    text_workspace: &'static mut TextWorkspace,
}

impl CydClockDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static TEXT_WORKSPACE: StaticCell<TextWorkspace> = StaticCell::new();

        Self {
            cyd,
            text_workspace: TextWorkspace::init_static(&TEXT_WORKSPACE),
        }
    }

    pub fn show(
        &mut self,
        wifi_mode: &str,
        clock_time: Option<&ClockTime>,
    ) -> Result<(), CydClockDisplayError> {
        let time_text = clock_time.map_or("--:--:--", ClockTime::as_str);

        self.show_text_line("CYD Clock", Rgb565::CYAN, Point::new(16, 6))?;

        let mut wifi_text = heapless::String::<32>::new();
        fmt::Write::write_fmt(&mut wifi_text, format_args!("WiFi: {wifi_mode}")).ok();
        self.show_text_line(wifi_text.as_str(), Rgb565::WHITE, Point::new(16, 32))?;

        let mut time_text_line = heapless::String::<32>::new();
        fmt::Write::write_fmt(&mut time_text_line, format_args!("Time: {time_text}")).ok();
        self.show_text_line(time_text_line.as_str(), Rgb565::YELLOW, Point::new(16, 58))?;

        self.show_clock(clock_time)?;

        Ok(())
    }

    fn show_text_line(
        &mut self,
        text: &str,
        color: Rgb565,
        top_left: Point,
    ) -> Result<(), CydClockDisplayError> {
        let mut text_line_buffer = self
            .text_workspace
            .view_mut(TEXT_LINE_WIDTH, TEXT_LINE_HEIGHT);
        text_line_buffer.clear(Rgb565::BLACK);
        Text::with_baseline(
            text,
            Point::new(0, 0),
            MonoTextStyle::new(&FONT_10X20, color),
            Baseline::Top,
        )
        .draw(&mut text_line_buffer)
        .ok();
        self.cyd.flush(&text_line_buffer, top_left)?;
        Ok(())
    }

    fn show_clock(&mut self, clock_time: Option<&ClockTime>) -> Result<(), CydClockDisplayError> {
        let mut primitives = [empty_primitive(); CLOCK_PRIMITIVE_COUNT];
        let mut primitive_count = draw_clock_face(&mut primitives);
        if let Some(clock_time) = clock_time {
            draw_clock_hands(clock_time, &mut primitives, &mut primitive_count);
        }
        self.cyd.draw_primitives_now(
            CLOCK_BOUNDS,
            Rgb565::BLACK,
            &primitives[..primitive_count],
        )?;
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

fn draw_clock_face(primitives: &mut [DrawPrimitive; CLOCK_PRIMITIVE_COUNT]) -> usize {
    primitives[0] = DrawPrimitive::Circle(Circle {
        center: clock_point(Point::new(CLOCK_CENTER_X, CLOCK_CENTER_Y)),
        radius: FACE_RADIUS,
        stroke_width: FACE_STROKE_WIDTH,
        color: Rgb565::BLUE,
        filled: false,
    });
    FACE_PRIMITIVE_COUNT
}

fn draw_clock_hands(
    clock_time: &ClockTime,
    primitives: &mut [DrawPrimitive; CLOCK_PRIMITIVE_COUNT],
    primitive_count: &mut usize,
) {
    let params = clock_time.params();
    for stroke_segment in CLOCK_HANDS.stroke_segments(&params) {
        let start = pose_to_point(stroke_segment.start());
        let end = pose_to_point(stroke_segment.end());
        if start != end {
            primitives[*primitive_count] = DrawPrimitive::LineSegment(LineSegment {
                start,
                end,
                width: stroke_segment.width(),
                color: Rgb565::from(RawU16::new(stroke_segment.color() as u16)),
            });
            *primitive_count += 1;
        }
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
            format_args!("{}:{:02}:{:02} {}", hours12, minutes, seconds, meridiem),
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
