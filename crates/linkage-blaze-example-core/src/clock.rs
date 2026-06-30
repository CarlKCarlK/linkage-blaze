//! The generic "clock" example: an analog clock face whose hands are driven by a
//! tiny [`linkage`](linkage_blaze_core::linkage), with a digital time read-out
//! above it.
//!
//! This mirrors [`skeleton_clock`](crate::skeleton_clock): the device-agnostic
//! game loop lives here (written against the [`Cyd`] and
//! [`ClockSync`] traits) while the esp32 binary
//! (`linkage-blaze-classic/examples/clock/main.rs`) only wires up the hardware
//! and hands a `&mut CydEsp` to [`clock`].

use core::convert::Infallible;

use device_envoy_core::clock_sync::{ClockSync, h12_m_s};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_6X10, ascii::FONT_10X20},
    pixelcolor::{IntoStorage, Rgb565},
    pixelcolor::{Rgb888, WebColors},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use linkage_blaze_core::{
    DiskItem, DrawItem, LinkageFixed, Pose, SphereItem, Vec3, linkage, linkage_fixed,
};
use linkage_blaze_cyd_core::{Cyd, CydFrame, DrawPrimitive, Ellipse, LineSegment, Orientation};
use log::info;
use static_cell::StaticCell;
use time::OffsetDateTime;

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BACKGROUND: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
pub const FOREGROUND: Rgb888 = Rgb888::CSS_NAVY;

// ── Screen / region layout ─────────────────────────────────────────────────────

pub const ORIENTATION: Orientation = Orientation::Landscape;

// todo0000 too complex
const TIME_FONT: MonoFont<'static> = FONT_10X20;
const TIME_TEXT_SCALE: usize = 2;
const TIME_TEXT_MAX_CHARS: usize = 8; // "12:59 PM"
const TIME_TEXT_UNSCALED_WIDTH: usize = TIME_TEXT_MAX_CHARS * 10;
const TIME_TEXT_UNSCALED_HEIGHT: usize = 20;
const TIME_TEXT_SCALED_WIDTH: usize = TIME_TEXT_UNSCALED_WIDTH * TIME_TEXT_SCALE;
const TIME_TEXT_SCALED_HEIGHT: usize = TIME_TEXT_UNSCALED_HEIGHT * TIME_TEXT_SCALE;
const TIME_TEXT_UNSCALED_PIXELS: usize = TIME_TEXT_UNSCALED_WIDTH * TIME_TEXT_UNSCALED_HEIGHT;
const TIME_TEXT_SCALED_PIXELS: usize = TIME_TEXT_SCALED_WIDTH * TIME_TEXT_SCALED_HEIGHT;

pub const WIFI_STATUS_FONT: MonoFont<'static> = FONT_6X10;

pub const WIFI_STATUS_REGION: Rectangle = Rectangle::new(Point::new(240, 8), Size::new(70, 10));
pub const TIME_REGION: Rectangle = Rectangle::new(Point::new(80, 34), Size::new(160, 40));

const CLOCK_BOUNDS: Rectangle = Rectangle::new(Point::new(80, 80), Size::new(160, 160));
const CLOCK_CENTER: Point = Point::new(80, 80);
const HAND_SCALE: f32 = 1.0; // todo000 part of projection?
const CLOCK_HANDS: LinkageFixed<2, 2, 48> = linkage_fixed!("clock.lb.rs");
const CLOCK_PRIMITIVE_CAPACITY: usize = CLOCK_HANDS.view().draw_item_count();

// ── Main function ────────────────────────────────────────────────────────

/// Run the clock render loop forever, driven by `clock_sync` ticks and drawn
/// onto `cyd`.
pub async fn clock<CydDevice, ClockSyncDevice>(
    cyd: &mut CydDevice,
    clock_sync: &ClockSyncDevice,
) -> Result<Infallible, Error<CydDevice::Error>>
where
    CydDevice: Cyd,
    ClockSyncDevice: ClockSync,
{
    static TIME_TEXT_UNSCALED_BUFFER: StaticCell<[u16; TIME_TEXT_UNSCALED_PIXELS]> =
        StaticCell::new();
    static TIME_TEXT_SCALED_BUFFER: StaticCell<[u16; TIME_TEXT_SCALED_PIXELS]> = StaticCell::new();
    let time_text_unscaled_buffer =
        &mut *TIME_TEXT_UNSCALED_BUFFER.init([0; TIME_TEXT_UNSCALED_PIXELS]);
    let time_text_scaled_buffer = &mut *TIME_TEXT_SCALED_BUFFER.init([0; TIME_TEXT_SCALED_PIXELS]);

    loop {
        // Wait for a tick and get the time.
        let tick = clock_sync.wait_for_tick().await;
        let local_time = &tick.local_time;
        // todo000 why?
        let clock_time = ClockTime::from_time(local_time);
        info!("tick {}", clock_time.as_str());

        // Draw the time explicitly so the surface default font/color can stay
        // dedicated to the small WiFi status text, while preserving the old
        // 2x enlarged clock text look.
        {
            let mut time_frame = cyd.frame_mut(TIME_REGION);
            draw_scaled_time(
                &mut time_frame,
                clock_time.as_str(),
                time_text_unscaled_buffer,
                time_text_scaled_buffer,
            );
            time_frame.flush().await.map_err(Error::Flush)?;
        }

        let params = clock_time.params();
        let mut primitives = heapless::Vec::<DrawPrimitive, CLOCK_PRIMITIVE_CAPACITY>::new();
        for draw_item in CLOCK_HANDS.view().draw_items(&params) {
            let Some(primitive) = draw_item_to_primitive(draw_item) else {
                continue;
            };
            push_primitive(&mut primitives, primitive)?;
        }
        cyd.draw_primitives(CLOCK_BOUNDS, Rgb565::from(BACKGROUND), &primitives)
            .map_err(Error::Flush)?;
    }
}

#[derive(Debug, derive_more::From)]
pub struct PrimitiveOverflowError(pub DrawPrimitive);

fn push_primitive<const N: usize>(
    primitives: &mut heapless::Vec<DrawPrimitive, N>,
    primitive: DrawPrimitive,
) -> Result<(), PrimitiveOverflowError> {
    match primitives.push(primitive) {
        Ok(()) => Ok(()),
        Err(primitive) => Err(PrimitiveOverflowError(primitive)),
    }
}

fn draw_item_to_primitive(draw_item: DrawItem) -> Option<DrawPrimitive> {
    match draw_item {
        DrawItem::Stroke(stroke) => {
            let start = clock_pose_point(stroke.start());
            let end = clock_pose_point(stroke.end());
            if start == end {
                return None;
            }
            Some(DrawPrimitive::LineSegment(LineSegment {
                start,
                end,
                width: clock_width_pixels(stroke.width()),
                color: Rgb565::from(stroke.color()),
            }))
        }
        DrawItem::Disk(disk) => Some(DrawPrimitive::Ellipse(disk_to_ellipse(disk))),
        DrawItem::Sphere(sphere) => Some(DrawPrimitive::Ellipse(sphere_to_ellipse(sphere))),
    }
}

fn disk_to_ellipse(disk: DiskItem) -> Ellipse {
    let orientation = disk.pose().orientation();
    Ellipse {
        center: clock_pose_point(disk.pose()),
        axis_a: project_dir(orientation.forward(), disk.radius()),
        axis_b: project_dir(orientation.left(), disk.radius()),
        radius: disk.radius() * HAND_SCALE,
        stroke_width: 0,
        color: Rgb565::from(disk.color()),
        filled: true,
    }
}

fn sphere_to_ellipse(sphere: SphereItem) -> Ellipse {
    let radius = sphere.radius() * HAND_SCALE;
    Ellipse {
        center: clock_pose_point(sphere.pose()),
        axis_a: (radius, 0.0),
        axis_b: (0.0, radius),
        radius,
        stroke_width: 0,
        color: Rgb565::from(sphere.color()),
        filled: true,
    }
}

fn clock_pose_point(pose: Pose) -> Point {
    let position = pose.position();
    Point::new(
        CLOCK_BOUNDS.top_left.x + CLOCK_CENTER.x + project_x(position, HAND_SCALE),
        CLOCK_BOUNDS.top_left.y + CLOCK_CENTER.y + project_y(position, HAND_SCALE),
    )
}

fn project_x(position: Vec3, scale: f32) -> i32 {
    -(position[1] * scale) as i32
}

fn project_y(position: Vec3, scale: f32) -> i32 {
    -(position[0] * scale) as i32
}

fn project_dir(world_dir: Vec3, radius: f32) -> (f32, f32) {
    (
        -world_dir[1] * radius * HAND_SCALE,
        -world_dir[0] * radius * HAND_SCALE,
    )
}

fn clock_width_pixels(width: f32) -> u16 {
    ((width * HAND_SCALE + 0.5) as u16).max(1)
}

struct BufferTarget<'a> {
    pixels: &'a mut [u16],
    size: Size,
}

impl DrawTarget for BufferTarget<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        for embedded_graphics::Pixel(point, color) in pixels {
            let Ok(x) = usize::try_from(point.x) else {
                continue;
            };
            let Ok(y) = usize::try_from(point.y) else {
                continue;
            };
            if x < width && y < height {
                self.pixels[y * width + x] = color.into_storage();
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.pixels.fill(color.into_storage());
        Ok(())
    }
}

impl OriginDimensions for BufferTarget<'_> {
    fn size(&self) -> Size {
        self.size
    }
}

fn draw_scaled_time<FrameError>(
    time_frame: &mut impl CydFrame<Error = FrameError>,
    text: &str,
    time_text_unscaled_buffer: &mut [u16; TIME_TEXT_UNSCALED_PIXELS],
    time_text_scaled_buffer: &mut [u16; TIME_TEXT_SCALED_PIXELS],
) {
    let background = Rgb565::from(BACKGROUND);
    let foreground = Rgb565::from(FOREGROUND);
    let mut unscaled_target = BufferTarget {
        pixels: time_text_unscaled_buffer,
        size: Size::new(
            TIME_TEXT_UNSCALED_WIDTH as u32,
            TIME_TEXT_UNSCALED_HEIGHT as u32,
        ),
    };
    unscaled_target
        .clear(background)
        .expect("clearing the fixed time buffer cannot fail");
    Text::with_baseline(
        text,
        Point::zero(),
        MonoTextStyle::new(&TIME_FONT, foreground),
        Baseline::Top,
    )
    .draw(&mut unscaled_target)
    .expect("drawing text to the fixed time buffer cannot fail");

    for source_y in 0..TIME_TEXT_UNSCALED_HEIGHT {
        for source_x in 0..TIME_TEXT_UNSCALED_WIDTH {
            let color = time_text_unscaled_buffer[source_y * TIME_TEXT_UNSCALED_WIDTH + source_x];
            let scaled_x = source_x * TIME_TEXT_SCALE;
            let scaled_y = source_y * TIME_TEXT_SCALE;
            for offset_y in 0..TIME_TEXT_SCALE {
                for offset_x in 0..TIME_TEXT_SCALE {
                    time_text_scaled_buffer
                        [(scaled_y + offset_y) * TIME_TEXT_SCALED_WIDTH + scaled_x + offset_x] =
                        color;
                }
            }
        }
    }

    time_frame
        .copy_from_565(time_text_scaled_buffer)
        .expect("scaled time buffer must match TIME_REGION exactly");
}

// ── Clock time ──────────────────────────────────────────────────────────────────

/// A formatted snapshot of the wall-clock time.
///
pub struct ClockTime {
    text: heapless::String<16>,
    hour_24: u8,
    minute: u8,
    second: u8,
}

impl ClockTime {
    /// Build a `ClockTime` from `local_time`, formatting a `12:59 PM`-style read-out.
    pub fn from_time(local_time: &OffsetDateTime) -> Self {
        let (hour_12, minute, second) = h12_m_s(local_time);
        let meridiem = if local_time.hour() % 24 < 12 {
            "AM"
        } else {
            "PM"
        };
        let mut text = heapless::String::new();
        core::fmt::write(&mut text, format_args!("{hour_12}:{minute:02} {meridiem}"))
            .expect("clock string fits in 16 bytes");
        Self {
            text,
            hour_24: local_time.hour(),
            minute,
            second,
        }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn params(&self) -> [f32; 2] {
        let second = self.second as f32 / 60.0;
        let minute = (self.minute as f32 + second) / 60.0;
        let hour = ((self.hour_24 % 12) as f32 + minute) / 12.0;
        let face_spin = (((self.second % 20) as f32) / 20.0 + 0.5) % 1.0;
        [hour, face_spin]
    }
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Error from the generic clock loop, generic over the surface's flush error `F`.
///
/// Only flushing a frame can fail today; it is converted explicitly with
/// `.map_err(Error::Flush)` at the call site (the same flush-error convention as
/// [`skeleton_clock::Error`](crate::skeleton_clock::Error)).
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// Flushing a frame to the display failed.
    #[from(ignore)]
    Flush(F),
    /// The clock linkage produced more draw primitives than the fixed batch allows.
    PrimitiveOverflow(PrimitiveOverflowError),
}
