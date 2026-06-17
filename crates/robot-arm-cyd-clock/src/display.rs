use core::fmt;

use cyd_esp32::{Cyd, CydError, DrawPrimitive, Ellipse, LineSegment, RectWorkspace, SCREEN_WIDTH};
use embedded_graphics::{
    Drawable,
    mono_font::{
        MonoFont, MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::{Rgb565, WebColors},
    prelude::Point,
    primitives::Rectangle,
    text::{Baseline, Text},
};
use esp_hal::time::Instant;
use robot_arm_core::{DiskItem, DrawItem, Linkage, Pose, Rgb888, RingItem, SphereItem, Vec3};
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
const CLOCK_BUFFER_WIDTH: usize = 140;
const CLOCK_BUFFER_HEIGHT: usize = 140;
const GLYPH_WORKSPACE_PIXELS: usize = GLYPH_WORKSPACE_WIDTH * GLYPH_WORKSPACE_HEIGHT;
const CLOCK_TOP_LEFT: Point = Point::new(90, 85);
const CLOCK_CENTER_X: i32 = 70;
const CLOCK_CENTER_Y: i32 = 70;
const HAND_SCALE: f32 = 1.0;
const BLACK: Rgb888 = Rgb888::CSS_BLACK;
const BG: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
const TEXT_DIM: Rgb888 = Rgb888::CSS_NAVY;
const TEXT_MAIN: Rgb888 = Rgb888::CSS_NAVY;
const TEXT_OK: Rgb888 = Rgb888::CSS_NAVY;
const CLOCK_PRIMITIVE_COUNT: usize = 9; // 2 disks + 7 strokes
const CLOCK_BOUNDS: Rectangle = Rectangle::new(
    CLOCK_TOP_LEFT,
    embedded_graphics::prelude::Size::new(CLOCK_BUFFER_WIDTH as u32, CLOCK_BUFFER_HEIGHT as u32),
);
const CLOCK_HANDS: Linkage<1, 60> = Linkage::start()
    .define_param("hour", 0.0)
    // Face disk
    .pen_color(Rgb888::new(33, 79, 155))
    .disk(66.0)
    // 12 o'clock tick (+X is already up, no yaw needed)
    .restart()
    .pen_color(Rgb888::new(255, 245, 216))
    .pen_width(4.5)
    .pen_up()
    .forward(48.0)
    .pen_down()
    .forward(14.0)
    // 3 o'clock tick
    .restart()
    .pen_color(Rgb888::new(255, 245, 216))
    .pen_width(4.5)
    .yaw(-90.0)
    .pen_up()
    .forward(51.0)
    .pen_down()
    .forward(11.0)
    // 6 o'clock tick
    .restart()
    .pen_color(Rgb888::new(255, 245, 216))
    .pen_width(4.5)
    .yaw(180.0)
    .pen_up()
    .forward(51.0)
    .pen_down()
    .forward(11.0)
    // 9 o'clock tick
    .restart()
    .pen_color(Rgb888::new(255, 245, 216))
    .pen_width(4.5)
    .yaw(90.0)
    .pen_up()
    .forward(51.0)
    .pen_down()
    .forward(11.0)
    // Hour hand
    .restart()
    .pen_color(Rgb888::CSS_ANTIQUE_WHITE)
    .pen_width(16.0)
    .yaw_param("hour", 360.0, 0.0)
    .forward(40.0)
    // Minute hand
    .restart()
    .pen_color(Rgb888::new(69, 215, 255))
    .pen_width(7.5)
    .yaw_param("hour", 4320.0, 0.0)
    .forward(52.0)
    // Second hand
    .restart()
    .pen_color(Rgb888::new(255, 89, 72))
    .pen_width(2.0)
    .yaw_param("hour", 259_200.0, 0.0)
    .forward(60.0)
    // Hub
    .restart()
    .up(8.0)
    .pen_color(Rgb888::CSS_RED)
    .disk(8.0);

type GlyphWorkspace = RectWorkspace<GLYPH_WORKSPACE_PIXELS>;

fn rgb565(color: Rgb888) -> Rgb565 {
    Rgb565::from(color)
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
    title_drawn: bool,
    last_time_text: heapless::String<16>,
    last_wifi_text: heapless::String<32>,
}

impl CydClockDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static GLYPH_WORKSPACE: StaticCell<GlyphWorkspace> = StaticCell::new();

        Self {
            cyd,
            glyph_workspace: GlyphWorkspace::init_static(&GLYPH_WORKSPACE),
            background_cleared: false,
            title_drawn: false,
            last_time_text: heapless::String::new(),
            last_wifi_text: heapless::String::new(),
        }
    }

    pub fn show(
        &mut self,
        wifi_mode: &str,
        clock_time: Option<&ClockTime>,
    ) -> Result<(), CydClockDisplayError> {
        if !self.background_cleared {
            self.cyd.clear_now(rgb565(BG))?;
            self.background_cleared = true;
        }

        let time_text = clock_time.map_or("--:--", ClockTime::as_str);

        if !self.title_drawn {
            self.show_small_text_line("CYD Clock", TEXT_DIM, Point::new(14, 8), 96)?;
            self.title_drawn = true;
        }

        let mut wifi_text = heapless::String::<32>::new();
        fmt::Write::write_fmt(
            &mut wifi_text,
            format_args!("WiFi {}", wifi_label(wifi_mode)),
        )
        .ok();
        if wifi_text.as_str() != self.last_wifi_text.as_str() {
            self.show_small_text_line(wifi_text.as_str(), TEXT_OK, Point::new(240, 8), 70)?;
            self.last_wifi_text.clear();
            self.last_wifi_text.push_str(wifi_text.as_str()).ok();
        }

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
        color: Rgb888,
    ) -> Result<(), CydClockDisplayError> {
        let flush_width = glyph_width * scale;
        let flush_height = glyph_height * scale;
        let mut glyph_left = top_left.x;

        for character in text.chars() {
            let mut character_text = heapless::String::<4>::new();
            fmt::Write::write_char(&mut character_text, character).ok();
            let mut glyph_buffer = self.glyph_workspace.view_mut(flush_width, flush_height);
            glyph_buffer.clear(rgb565(BG));
            Text::with_baseline(
                character_text.as_str(),
                Point::new(0, 0),
                MonoTextStyle::new(font, rgb565(color)),
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
        color: Rgb888,
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
        color: Rgb888,
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
            rgb565(BG),
        )?;
        Ok(())
    }

    fn show_clock(&mut self, clock_time: Option<&ClockTime>) -> Result<(), CydClockDisplayError> {
        let mut primitives = [empty_primitive(); CLOCK_PRIMITIVE_COUNT];
        let mut primitive_count = 0;
        let params = clock_time.map_or([0.0; 1], |t| t.params());
        draw_clock_hands(&params, &mut primitives, &mut primitive_count);
        let t0 = Instant::now();
        self.cyd
            .draw_primitives_now(CLOCK_BOUNDS, rgb565(BG), &primitives[..primitive_count])?;
        let elapsed_ms = (Instant::now() - t0).as_millis();
        esp_println::println!("draw_primitives_now ms = {}", elapsed_ms);
        Ok(())
    }
}

fn empty_primitive() -> DrawPrimitive {
    DrawPrimitive::LineSegment(LineSegment {
        start: Point::new(0, 0),
        end: Point::new(0, 0),
        width: 0,
        color: rgb565(BLACK),
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
    params: &[f32; 1],
    primitives: &mut [DrawPrimitive; CLOCK_PRIMITIVE_COUNT],
    primitive_count: &mut usize,
) {
    for draw_item in CLOCK_HANDS.draw_items(params) {
        match draw_item {
            DrawItem::Stroke(stroke) => {
                let start = pose_to_point(stroke.start());
                let end = pose_to_point(stroke.end());
                if start != end {
                    primitives[*primitive_count] = DrawPrimitive::LineSegment(LineSegment {
                        start,
                        end,
                        width: clock_width_pixels(stroke.width()),
                        color: Rgb565::from(stroke.color()),
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
            DrawItem::Sphere(sphere) => {
                primitives[*primitive_count] = DrawPrimitive::Ellipse(sphere_to_ellipse(sphere));
                *primitive_count += 1;
            }
        }
    }
}

// Project model coordinates onto the clock face screen plane.
// Convention: model +X=up, model +Y=left.
// screen_x = -model_y  (left → negative screen X)
// screen_y = -model_x  (up   → negative screen Y, since embedded-graphics Y goes down)
fn project_x(pos: Vec3, scale: f32) -> i32 {
    -(pos[1] * scale) as i32
}
fn project_y(pos: Vec3, scale: f32) -> i32 {
    -(pos[0] * scale) as i32
}

fn disk_to_ellipse(disk: DiskItem) -> Ellipse {
    let pos = disk.pose().position();
    let center = clock_point(Point::new(
        CLOCK_CENTER_X + project_x(pos, 1.0),
        CLOCK_CENTER_Y + project_y(pos, 1.0),
    ));
    let orient = disk.pose().orientation();
    let r = disk.radius();
    Ellipse {
        center,
        // project orientation axes: screen_x=-model_y, screen_y=-model_x
        axis_a: (-orient[1][0] * r, -orient[0][0] * r),
        axis_b: (-orient[1][1] * r, -orient[0][1] * r),
        radius: r,
        stroke_width: 0,
        color: Rgb565::from(disk.color()),
        filled: true,
    }
}

fn ring_to_ellipse(ring: RingItem) -> Ellipse {
    let pos = ring.pose().position();
    let center = clock_point(Point::new(
        CLOCK_CENTER_X + project_x(pos, 1.0),
        CLOCK_CENTER_Y + project_y(pos, 1.0),
    ));
    let orient = ring.pose().orientation();
    let r = ring.radius();
    Ellipse {
        center,
        axis_a: (-orient[1][0] * r, -orient[0][0] * r),
        axis_b: (-orient[1][1] * r, -orient[0][1] * r),
        radius: r,
        stroke_width: clock_width_pixels(ring.width()),
        color: Rgb565::from(ring.color()),
        filled: false,
    }
}

fn sphere_to_ellipse(sphere: SphereItem) -> Ellipse {
    let pos = sphere.pose().position();
    let center = clock_point(Point::new(
        CLOCK_CENTER_X + project_x(pos, 1.0),
        CLOCK_CENTER_Y + project_y(pos, 1.0),
    ));
    let r = sphere.radius();
    Ellipse {
        center,
        axis_a: (r, 0.0),
        axis_b: (0.0, r),
        radius: r,
        stroke_width: 0,
        color: Rgb565::from(sphere.color()),
        filled: true,
    }
}

fn clock_width_pixels(width: f32) -> u16 {
    round_to_u16(width * HAND_SCALE).max(1)
}

fn pose_to_point(pose: Pose) -> Point {
    let position = pose.position();
    clock_point(Point::new(
        CLOCK_CENTER_X + project_x(position, HAND_SCALE),
        CLOCK_CENTER_Y + project_y(position, HAND_SCALE),
    ))
}

fn round_to_u16(value: f32) -> u16 {
    (value + 0.5) as u16
}

fn clock_point(point: Point) -> Point {
    Point::new(CLOCK_TOP_LEFT.x + point.x, CLOCK_TOP_LEFT.y + point.y)
}

pub struct ClockTime {
    text: heapless::String<16>,
    hours: u8,
    minutes: u8,
    seconds: f32,
}

impl ClockTime {
    pub fn new(hours: u8, minutes: u8, seconds: f32) -> Result<Self, fmt::Error> {
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
        let seconds = seconds.clamp(0.0, 60.0);
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

    fn params(&self) -> [f32; 1] {
        let second = self.seconds / 60.0;
        let minute = (self.minutes as f32 + second) / 60.0;
        let hour = ((self.hours % 12) as f32 + minute) / 12.0;
        [hour]
    }
}
