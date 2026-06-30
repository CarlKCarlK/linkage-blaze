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
    DrawItem, LinkageFixed, ProjectedDrawItem, Projection, linkage, linkage_fixed,
};
use linkage_blaze_cyd_core::{Cyd, CydFrame, DrawPrimitive, Ellipse, LineSegment, Orientation};
use log::info;
use static_cell::StaticCell;

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
const PROJECTION: Projection = Projection::top_orthographic(
    /* target origin */ Point::new(160, 160),
    /* scale */ 1.0,
);
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
        let (hour_12, minute, second) = h12_m_s(local_time);
        let hour_24 = local_time.hour();
        let time_text = text_12h(hour_12, minute, hour_24);
        info!("tick {}", time_text.as_str());

        // Draw the time explicitly so the surface default font/color can stay
        // dedicated to the small WiFi status text, while preserving the old
        // 2x enlarged clock text look.
        {
            let mut time_frame = cyd.frame_mut(TIME_REGION);
            draw_scaled_time(
                &mut time_frame,
                time_text.as_str(),
                time_text_unscaled_buffer,
                time_text_scaled_buffer,
            );
            time_frame.flush().await.map_err(Error::Flush)?;
        }

        let params = linkage_params(hour_24, minute, second);
        let mut primitives = heapless::Vec::<DrawPrimitive, CLOCK_PRIMITIVE_CAPACITY>::new();
        for draw_item in CLOCK_HANDS.view().draw_items(&params) {
            let Some(primitive) = draw_item_to_primitive(draw_item) else {
                continue;
            };
            if let Err(primitive) = primitives.push(primitive) {
                return Err(Error::PrimitiveOverflow(primitive));
            }
        }
        cyd.draw_primitives(CLOCK_BOUNDS, Rgb565::from(BACKGROUND), &primitives)
            .map_err(Error::Flush)?;
    }
}

fn draw_item_to_primitive(draw_item: DrawItem) -> Option<DrawPrimitive> {
    match draw_item.project(&PROJECTION) {
        ProjectedDrawItem::Stroke {
            start,
            end,
            color,
            pixel_width,
        } => {
            let start = projected_point(start);
            let end = projected_point(end);
            if start == end {
                return None;
            }
            Some(DrawPrimitive::LineSegment(LineSegment {
                start,
                end,
                width: pixel_width_u16(pixel_width),
                color: Rgb565::from(color),
            }))
        }
        ProjectedDrawItem::Ellipse {
            center,
            axis_a,
            axis_b,
            color,
        } => Some(DrawPrimitive::Ellipse(Ellipse {
            center: projected_point(center),
            axis_a,
            axis_b,
            radius: ellipse_bound_radius(axis_a, axis_b),
            stroke_width: 0,
            color: Rgb565::from(color),
            filled: true,
        })),
        ProjectedDrawItem::Circle {
            center,
            pixel_radius,
            color,
        } => Some(DrawPrimitive::Ellipse(Ellipse {
            center: projected_point(center),
            axis_a: (pixel_radius, 0.0),
            axis_b: (0.0, pixel_radius),
            radius: pixel_radius,
            stroke_width: 0,
            color: Rgb565::from(color),
            filled: true,
        })),
    }
}

fn projected_point((x, y): (f32, f32)) -> Point {
    Point::new(x as i32, y as i32)
}

fn pixel_width_u16(width: f32) -> u16 {
    ((width + 0.5) as u16).max(1)
}

fn ellipse_bound_radius(axis_a: (f32, f32), axis_b: (f32, f32)) -> f32 {
    (axis_a.0.abs() + axis_b.0.abs()).max(axis_a.1.abs() + axis_b.1.abs())
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

/// Format a 12-hour clock string with AM/PM.
fn text_12h(hour_12: u8, minute: u8, hour_24: u8) -> heapless::String<16> {
    let meridiem = if hour_24 % 24 < 12 { "AM" } else { "PM" };
    let mut text = heapless::String::new();
    core::fmt::write(&mut text, format_args!("{hour_12}:{minute:02} {meridiem}"))
        .expect("clock string fits in 16 bytes");
    text
}

fn linkage_params(hour_24: u8, minute: u8, second: u8) -> [f32; 2] {
    let second_turn = second as f32 / 60.0;
    let minute_turn = (minute as f32 + second_turn) / 60.0;
    let hour = ((hour_24 % 12) as f32 + minute_turn) / 12.0;
    let face_spin = (((second % 20) as f32) / 20.0 + 0.5) % 1.0;
    [hour, face_spin]
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Error from the generic clock loop, generic over the surface's flush error `F`.
///
/// The device's flush error `F` is converted explicitly with
/// `.map_err(Error::Flush)` at the call site (the same flush-error convention as
/// [`skeleton_clock::Error`](crate::skeleton_clock::Error)).
#[derive(Debug)]
pub enum Error<F> {
    /// Flushing a frame to the display failed.
    Flush(F),
    /// The clock linkage produced more draw primitives than the fixed batch allows.
    PrimitiveOverflow(DrawPrimitive),
}
