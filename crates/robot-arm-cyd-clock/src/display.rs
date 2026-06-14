use core::fmt;

use cyd_esp32::{Cyd, CydError, RectView, RectWorkspace};
use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::{Rgb565, RgbColor},
    prelude::{DrawTarget, Point},
    primitives::{Circle, Primitive, PrimitiveStyle},
    text::{Baseline, Text},
};
use robot_arm_core::{Linkage, Pose};
use static_cell::StaticCell;

const TEXT_LINE_WIDTH: usize = 240;
const TEXT_LINE_HEIGHT: usize = 20;
const CLOCK_BUFFER_WIDTH: usize = 112;
const CLOCK_BUFFER_HEIGHT: usize = 112;
const CLOCK_BUFFER_PIXELS: usize = CLOCK_BUFFER_WIDTH * CLOCK_BUFFER_HEIGHT;
const CLOCK_TOP_LEFT: Point = Point::new(104, 120);
const CLOCK_CENTER_X: i32 = 56;
const CLOCK_CENTER_Y: i32 = 56;
const CLOCK_RADIUS: i32 = 50;
const HAND_SCALE: f32 = 1.0;
const HOUR_PARAM: usize = 0;
const MINUTE_PARAM: usize = 1;
const SECOND_PARAM: usize = 2;
const CLOCK_HANDS: Linkage<3, 9> = Linkage::start()
    .yaw_param(HOUR_PARAM, -90.0, 270.0)
    .forward(28.0)
    .restart()
    .yaw_param(MINUTE_PARAM, -90.0, 270.0)
    .forward(42.0)
    .restart()
    .yaw_param(SECOND_PARAM, -90.0, 270.0)
    .forward(48.0);

type ClockWorkspace = RectWorkspace<CLOCK_BUFFER_PIXELS>;

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
    clock_workspace: &'static mut ClockWorkspace,
}

impl CydClockDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static CLOCK_WORKSPACE: StaticCell<ClockWorkspace> = StaticCell::new();

        Self {
            cyd,
            clock_workspace: ClockWorkspace::init_static(&CLOCK_WORKSPACE),
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
            .clock_workspace
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
        let mut clock_buffer = self
            .clock_workspace
            .view_mut(CLOCK_BUFFER_WIDTH, CLOCK_BUFFER_HEIGHT);
        clock_buffer.clear(Rgb565::BLACK);
        draw_clock_face(&mut clock_buffer);
        if let Some(clock_time) = clock_time {
            draw_clock_hands(&mut clock_buffer, clock_time);
        }
        self.cyd.flush(&clock_buffer, CLOCK_TOP_LEFT)?;
        Ok(())
    }
}

fn draw_clock_face(target: &mut RectView<'_>) {
    Circle::with_center(
        Point::new(CLOCK_CENTER_X, CLOCK_CENTER_Y),
        (CLOCK_RADIUS * 2) as u32,
    )
    .into_styled(PrimitiveStyle::with_stroke(Rgb565::BLUE, 1))
    .draw(target)
    .ok();
}

fn draw_clock_hands(target: &mut RectView<'_>, clock_time: &ClockTime) {
    let params = clock_time.params();
    let mut previous_pose = None;
    let mut hand_index = 0;
    for pose in CLOCK_HANDS.poses(&params) {
        if is_origin_pose(pose) {
            previous_pose = Some(pose);
            continue;
        }

        if let Some(previous_pose) = previous_pose {
            let radius = match hand_index {
                0 => 4,
                1 => 2,
                _ => 1,
            };
            draw_pose_line(target, previous_pose, pose, radius, Rgb565::GREEN);
            hand_index += 1;
        }
        previous_pose = Some(pose);
    }
}

fn is_origin_pose(pose: Pose) -> bool {
    let position = pose.position();
    position[0].abs() < 0.001 && position[1].abs() < 0.001 && position[2].abs() < 0.001
}

fn draw_pose_line(target: &mut RectView<'_>, start: Pose, end: Pose, radius: i32, color: Rgb565) {
    let start = pose_to_point(start);
    let end = pose_to_point(end);
    draw_wide_line(target, start, end, radius, color);
}

fn pose_to_point(pose: Pose) -> Point {
    let position = pose.position();
    Point::new(
        CLOCK_CENTER_X + (position[0] * HAND_SCALE) as i32,
        CLOCK_CENTER_Y + (position[1] * HAND_SCALE) as i32,
    )
}

fn draw_wide_line(target: &mut RectView<'_>, start: Point, end: Point, radius: i32, color: Rgb565) {
    let mut position_x = start.x;
    let mut position_y = start.y;
    let end_x = end.x;
    let end_y = end.y;
    let delta_x = (end_x - position_x).abs();
    let step_x = if position_x < end_x { 1 } else { -1 };
    let delta_y = -(end_y - position_y).abs();
    let step_y = if position_y < end_y { 1 } else { -1 };
    let mut error = delta_x + delta_y;

    loop {
        draw_wide_pixel(target, position_x, position_y, radius, color);
        if position_x == end_x && position_y == end_y {
            break;
        }
        let error2 = 2 * error;
        if error2 >= delta_y {
            error += delta_y;
            position_x += step_x;
        }
        if error2 <= delta_x {
            error += delta_x;
            position_y += step_y;
        }
    }
}

fn draw_wide_pixel(target: &mut RectView<'_>, x: i32, y: i32, radius: i32, color: Rgb565) {
    let radius_squared = radius * radius;
    for offset_y in -radius..=radius {
        for offset_x in -radius..=radius {
            if offset_x * offset_x + offset_y * offset_y <= radius_squared {
                draw_pixel(target, Point::new(x + offset_x, y + offset_y), color);
            }
        }
    }
}

fn draw_pixel(target: &mut RectView<'_>, point: Point, color: Rgb565) {
    match target.draw_iter(core::iter::once(Pixel(point, color))) {
        Ok(()) => {}
        Err(infallible) => match infallible {},
    }
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
