use core::fmt;

use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{Rgb565, WebColors},
    prelude::{Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use esp_hal::time::Instant;
use linkage_blaze_core::{DrawItem, LinkageFixed, Pose, Rgb888, linkage, linkage_fixed};
use linkage_blaze_cyd::{Cyd, CydError, RectWorkspace};
use log::info;
use static_cell::StaticCell;

const BG: Rgb888 = Rgb888::CSS_BLACK;
const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;
const DANCE_BOUNDS: Rectangle = Rectangle::new(Point::new(16, 38), Size::new(288, 190));
const DANCE_CENTER_X: i32 = 160;
const DANCE_BASELINE_Y: i32 = 216;
const DANCE_SCALE: f32 = 0.82;
const STROKE_SCALE: f32 = 1.0;
const SMALL_GLYPH_WIDTH: usize = 6;
const SMALL_GLYPH_HEIGHT: usize = 10;
const TIME_TEXT_TOP_LEFT: Point = Point::new(136, 12);
const WIFI_TEXT_TOP_LEFT: Point = Point::new(8, 12);
const GLYPH_WORKSPACE_PIXELS: usize = SMALL_GLYPH_WIDTH * SMALL_GLYPH_HEIGHT;
const DANCE: LinkageFixed<3, 4, 377> = linkage_fixed!("dance.lb.rs");

type GlyphWorkspace = RectWorkspace<GLYPH_WORKSPACE_PIXELS>;

fn rgb565(color: Rgb888) -> Rgb565 {
    Rgb565::from(color)
}

pub enum CydDanceDisplayError {
    Cyd(CydError),
}

impl fmt::Debug for CydDanceDisplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CydDanceDisplayError::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
        }
    }
}

impl From<CydError> for CydDanceDisplayError {
    fn from(error: CydError) -> Self {
        Self::Cyd(error)
    }
}

pub struct CydDanceDisplay {
    cyd: Cyd,
    glyph_workspace: &'static mut GlyphWorkspace,
    background_cleared: bool,
    last_time_text: heapless::String<16>,
    last_wifi_text: heapless::String<32>,
}

impl CydDanceDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static GLYPH_WORKSPACE: StaticCell<GlyphWorkspace> = StaticCell::new();

        Self {
            cyd,
            glyph_workspace: GlyphWorkspace::init_static(&GLYPH_WORKSPACE),
            background_cleared: false,
            last_time_text: heapless::String::new(),
            last_wifi_text: heapless::String::new(),
        }
    }

    pub fn show(
        &mut self,
        wifi_mode: &str,
        dance_time: Option<&DanceTime>,
    ) -> Result<(), CydDanceDisplayError> {
        info!(
            "display show start: wifi_mode={wifi_mode}, has_time={}",
            dance_time.is_some()
        );
        if !self.background_cleared {
            info!("display clearing background");
            self.cyd.clear_now(rgb565(BG))?;
            self.background_cleared = true;
            info!("display background cleared");
        }

        let mut wifi_text = heapless::String::<32>::new();
        fmt::Write::write_fmt(
            &mut wifi_text,
            format_args!("WiFi {}", wifi_label(wifi_mode)),
        )
        .ok();
        if wifi_text.as_str() != self.last_wifi_text.as_str() {
            info!("display updating wifi text: {}", wifi_text.as_str());
            self.show_small_text_line(wifi_text.as_str(), WIFI_TEXT_TOP_LEFT, 90)?;
            self.last_wifi_text.clear();
            self.last_wifi_text.push_str(wifi_text.as_str()).ok();
        }

        let time_text = dance_time.map_or("--:--:--", DanceTime::as_str);
        if time_text != self.last_time_text.as_str() {
            info!("display updating time text: {time_text}");
            self.show_small_text_line(time_text, TIME_TEXT_TOP_LEFT, 72)?;
            self.last_time_text.clear();
            self.last_time_text.push_str(time_text).ok();
        }

        self.show_dance(dance_time)?;

        info!("display show complete");
        Ok(())
    }

    fn show_small_text_line(
        &mut self,
        text: &str,
        top_left: Point,
        width: usize,
    ) -> Result<(), CydDanceDisplayError> {
        self.cyd.fill_rect_now(
            Rectangle::new(top_left, Size::new(width as u32, SMALL_GLYPH_HEIGHT as u32)),
            rgb565(BG),
        )?;

        let mut glyph_left = top_left.x;
        for character in text.chars() {
            let mut character_text = heapless::String::<4>::new();
            fmt::Write::write_char(&mut character_text, character).ok();
            let mut glyph_buffer = self
                .glyph_workspace
                .view_mut(SMALL_GLYPH_WIDTH, SMALL_GLYPH_HEIGHT);
            glyph_buffer.clear(rgb565(BG));
            Text::with_baseline(
                character_text.as_str(),
                Point::new(0, 0),
                MonoTextStyle::new(&FONT_6X10, rgb565(TEXT)),
                Baseline::Top,
            )
            .draw(&mut glyph_buffer)
            .ok();
            self.cyd
                .flush(&glyph_buffer, Point::new(glyph_left, top_left.y))?;
            glyph_left += SMALL_GLYPH_WIDTH as i32;
        }

        Ok(())
    }

    fn show_dance(&mut self, dance_time: Option<&DanceTime>) -> Result<(), CydDanceDisplayError> {
        let params = dance_time.map_or([0.5; 3], DanceTime::params);
        info!(
            "dance draw start: params=({:.3}, {:.3}, {:.3}), bounds={}x{}",
            params[0], params[1], params[2], DANCE_BOUNDS.size.width, DANCE_BOUNDS.size.height
        );
        let t0 = Instant::now();
        self.cyd.fill_contiguous_now(
            DANCE_BOUNDS,
            DancePixels {
                index: 0,
                pixel_count: DANCE_BOUNDS.size.width as usize * DANCE_BOUNDS.size.height as usize,
                params,
            },
        )?;
        let elapsed_ms = (Instant::now() - t0).as_millis();
        info!("dance draw complete: {elapsed_ms} ms");
        Ok(())
    }
}

struct DancePixels {
    index: usize,
    pixel_count: usize,
    params: [f32; 3],
}

impl Iterator for DancePixels {
    type Item = Rgb565;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.pixel_count {
            return None;
        }

        let local_x = self.index % DANCE_BOUNDS.size.width as usize;
        let local_y = self.index / DANCE_BOUNDS.size.width as usize;
        self.index += 1;

        let point = Point::new(
            DANCE_BOUNDS.top_left.x + local_x as i32,
            DANCE_BOUNDS.top_left.y + local_y as i32,
        );

        Some(rgb565(dance_pixel_color(point, &self.params)))
    }
}

fn dance_pixel_color(point: Point, params: &[f32; 3]) -> Rgb888 {
    let mut color = BG;
    for draw_item in DANCE.view().draw_items(params) {
        match draw_item {
            DrawItem::Stroke(stroke) => {
                let start = pose_to_point(stroke.start());
                let end = pose_to_point(stroke.end());
                let radius = (stroke.width() * STROKE_SCALE * 0.5).max(0.75);
                if point_covers_segment(point, start, end, radius) {
                    color = stroke.color();
                }
            }
            DrawItem::Disk(disk) => {
                if point_covers_circle(point, pose_to_point(disk.pose()), disk.radius()) {
                    color = disk.color();
                }
            }
            DrawItem::Ring(ring) => {
                let radius = ring.radius() * DANCE_SCALE;
                let width = ring.width().max(1.0);
                if point_covers_ring(point, pose_to_point(ring.pose()), radius, width) {
                    color = ring.color();
                }
            }
            DrawItem::Sphere(sphere) => {
                if point_covers_circle(point, pose_to_point(sphere.pose()), sphere.radius()) {
                    color = sphere.color();
                }
            }
        }
    }
    color
}

fn point_covers_segment(point: Point, start: Point, end: Point, radius: f32) -> bool {
    let point_x = point.x as f32;
    let point_y = point.y as f32;
    let start_x = start.x as f32;
    let start_y = start.y as f32;
    let segment_x = (end.x - start.x) as f32;
    let segment_y = (end.y - start.y) as f32;
    let length_squared = segment_x * segment_x + segment_y * segment_y;
    if length_squared == 0.0 {
        return point_covers_circle(point, start, radius);
    }

    let projection = (((point_x - start_x) * segment_x + (point_y - start_y) * segment_y)
        / length_squared)
        .clamp(0.0, 1.0);
    let closest_x = start_x + projection * segment_x;
    let closest_y = start_y + projection * segment_y;
    let distance_x = point_x - closest_x;
    let distance_y = point_y - closest_y;
    distance_x * distance_x + distance_y * distance_y <= radius * radius
}

fn point_covers_circle(point: Point, center: Point, radius: f32) -> bool {
    let radius = radius * DANCE_SCALE;
    let dx = (point.x - center.x) as f32;
    let dy = (point.y - center.y) as f32;
    dx * dx + dy * dy <= radius * radius
}

fn point_covers_ring(point: Point, center: Point, radius: f32, width: f32) -> bool {
    let dx = (point.x - center.x) as f32;
    let dy = (point.y - center.y) as f32;
    let distance_squared = dx * dx + dy * dy;
    let outer = radius + width * 0.5;
    let inner = (radius - width * 0.5).max(0.0);
    distance_squared <= outer * outer && distance_squared >= inner * inner
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

fn wifi_label(wifi_mode: &str) -> &str {
    match wifi_mode {
        "connected" => "OK",
        "connecting" => "...",
        "connect failed" => "fail",
        "setup CydDance" => "setup",
        _ => wifi_mode,
    }
}

pub struct DanceTime {
    text: heapless::String<16>,
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl DanceTime {
    pub fn new(hours: u8, minutes: u8, seconds: u8) -> Result<Self, fmt::Error> {
        let mut text = heapless::String::<16>::new();
        fmt::Write::write_fmt(
            &mut text,
            format_args!("{:02}:{:02}:{:02}", hours, minutes, seconds),
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
        [second, minute, hour]
    }
}
