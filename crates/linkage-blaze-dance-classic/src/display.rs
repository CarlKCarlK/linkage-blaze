use core::fmt;

use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{IntoStorage, Rgb565, WebColors},
    prelude::{Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use esp_hal::time::Instant;
use linkage_blaze_core::{DrawItem, LinkageFixed, Pose, Rgb888, linkage, linkage_fixed};
use linkage_blaze_cyd::{Cyd, CydError, RectView, RectWorkspace};
use log::info;
use static_cell::StaticCell;

const BG: Rgb888 = Rgb888::CSS_BLACK;
const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;
const DANCE_TOP_LEFT: Point = Point::new(60, 36);
const DANCE_WIDTH: usize = 200;
const DANCE_HEIGHT: usize = 200;
const DANCE_TILE_WIDTH: usize = 100;
const DANCE_TILE_HEIGHT: usize = 100;
const DANCE_TILE_PIXELS: usize = DANCE_TILE_WIDTH * DANCE_TILE_HEIGHT;
const DANCE_CENTER_X: i32 = 100;
const DANCE_BASELINE_Y: i32 = 188;
const DANCE_SCALE: f32 = 0.56;
const SMALL_GLYPH_WIDTH: usize = 6;
const SMALL_GLYPH_HEIGHT: usize = 10;
const TIME_TEXT_TOP_LEFT: Point = Point::new(136, 12);
const WIFI_TEXT_TOP_LEFT: Point = Point::new(8, 12);
const GLYPH_WORKSPACE_PIXELS: usize = SMALL_GLYPH_WIDTH * SMALL_GLYPH_HEIGHT;
const DANCE: LinkageFixed<3, 4, 377> = linkage_fixed!("dance.lb.rs");

type GlyphWorkspace = RectWorkspace<GLYPH_WORKSPACE_PIXELS>;
type DanceWorkspace = RectWorkspace<DANCE_TILE_PIXELS>;

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
    dance_workspace: &'static mut DanceWorkspace,
    background_cleared: bool,
    last_time_text: heapless::String<16>,
    last_wifi_text: heapless::String<32>,
}

impl CydDanceDisplay {
    pub fn new(cyd: Cyd) -> Self {
        static GLYPH_WORKSPACE: StaticCell<GlyphWorkspace> = StaticCell::new();
        static DANCE_WORKSPACE: StaticCell<DanceWorkspace> = StaticCell::new();

        Self {
            cyd,
            glyph_workspace: GlyphWorkspace::init_static(&GLYPH_WORKSPACE),
            dance_workspace: DanceWorkspace::init_static(&DANCE_WORKSPACE),
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
            "dance draw start: params=({:.3}, {:.3}, {:.3}), area={}x{}, tile={}x{}",
            params[0],
            params[1],
            params[2],
            DANCE_WIDTH,
            DANCE_HEIGHT,
            DANCE_TILE_WIDTH,
            DANCE_TILE_HEIGHT
        );
        let t0 = Instant::now();
        for tile_y in [0, DANCE_TILE_HEIGHT] {
            for tile_x in [0, DANCE_TILE_WIDTH] {
                let tile_origin = Point::new(tile_x as i32, tile_y as i32);
                let tile_top_left = Point::new(
                    DANCE_TOP_LEFT.x + tile_origin.x,
                    DANCE_TOP_LEFT.y + tile_origin.y,
                );
                let mut dance_buffer = self
                    .dance_workspace
                    .view_mut(DANCE_TILE_WIDTH, DANCE_TILE_HEIGHT);
                dance_buffer.clear(rgb565(BG));
                draw_dance_buffer(&mut dance_buffer, &params, tile_origin);
                self.cyd.flush(&dance_buffer, tile_top_left)?;
            }
        }
        let elapsed_ms = (Instant::now() - t0).as_millis();
        info!("dance draw complete: {elapsed_ms} ms");
        Ok(())
    }
}

fn draw_dance_buffer(dance_buffer: &mut RectView<'_>, params: &[f32; 3], tile_origin: Point) {
    for draw_item in DANCE.view().draw_items(params) {
        match draw_item {
            DrawItem::Stroke(stroke) => {
                draw_segment(
                    dance_buffer,
                    pose_to_point(stroke.start()),
                    pose_to_point(stroke.end()),
                    tile_origin,
                    stroke.color(),
                );
            }
            DrawItem::Disk(disk) => {
                draw_filled_circle(
                    dance_buffer,
                    pose_to_point(disk.pose()),
                    disk.radius(),
                    tile_origin,
                    disk.color(),
                );
            }
            DrawItem::Ring(ring) => {
                draw_ring(
                    dance_buffer,
                    pose_to_point(ring.pose()),
                    ring.radius(),
                    ring.width(),
                    tile_origin,
                    ring.color(),
                );
            }
            DrawItem::Sphere(sphere) => {
                draw_filled_circle(
                    dance_buffer,
                    pose_to_point(sphere.pose()),
                    sphere.radius(),
                    tile_origin,
                    sphere.color(),
                );
            }
        }
    }
}

fn draw_segment(
    dance_buffer: &mut RectView<'_>,
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
        put_pixel(dance_buffer, current_x, current_y, tile_origin, color);
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

fn draw_filled_circle(
    dance_buffer: &mut RectView<'_>,
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
                    dance_buffer,
                    center.x + local_x,
                    center.y + local_y,
                    tile_origin,
                    color,
                );
            }
        }
    }
}

fn draw_ring(
    dance_buffer: &mut RectView<'_>,
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
                    dance_buffer,
                    center.x + local_x,
                    center.y + local_y,
                    tile_origin,
                    color,
                );
            }
        }
    }
}

fn put_pixel(dance_buffer: &mut RectView<'_>, x: i32, y: i32, tile_origin: Point, color: Rgb888) {
    let x = x - tile_origin.x;
    let y = y - tile_origin.y;
    if x < 0 || y < 0 {
        return;
    }

    let x = x as usize;
    let y = y as usize;
    if x >= DANCE_TILE_WIDTH || y >= DANCE_TILE_HEIGHT {
        return;
    }

    dance_buffer.raw_pixels_mut()[y * DANCE_TILE_WIDTH + x] = rgb565(color).into_storage();
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
