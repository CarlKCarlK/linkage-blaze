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
use embassy_time::{Duration, Instant};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_6X10, ascii::FONT_10X20},
    pixelcolor::{IntoStorage, Rgb565},
    pixelcolor::{Rgb888, WebColors},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use linkage_blaze_core::{LinkageFixed, LinkageView, Projection, linkage, linkage_fixed};
use linkage_blaze_cyd_core::{Cyd, CydFrame, Orientation};
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
const TIME_TEXT_CAPACITY: usize = 16;
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
// todo000 reorder the consts.
const LINKAGE0: LinkageFixed<2, 2, 48> = linkage_fixed!("clock.lb.rs");
const LINKAGE: LinkageView<2, 2> = LINKAGE0.view();

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
    let background = Rgb565::from(BACKGROUND);
    let foreground = Rgb565::from(FOREGROUND);

    loop {
        let loop_started = Instant::now();

        // Wait for a tick and get the time.
        let tick = clock_sync.wait_for_tick().await;
        let tick_ready = Instant::now();
        let local_time = &tick.local_time;
        let (hour_12, minute, second) = h12_m_s(local_time);
        let hour_24 = local_time.hour();
        let time_text = text_12h(hour_12, minute, hour_24);
        info!("tick {}", time_text.as_str());

        let text_started = Instant::now();

        // Draw the time explicitly so the surface default font/color can stay
        // dedicated to the small WiFi status text, while preserving the old
        // 2x enlarged clock text look.
        {
            let mut time_frame = cyd.frame_mut(TIME_REGION);
            draw_scaled_time(
                &mut time_frame,
                time_text.as_str(),
                background,
                foreground,
                time_text_unscaled_buffer,
                time_text_scaled_buffer,
            );
            time_frame.flush().await.map_err(Error::Flush)?;
        }
        let text_done = Instant::now();

        let params_started = Instant::now();
        let params = linkage_params(hour_24, minute, second);
        let params_done = Instant::now();

        let prepare_started = Instant::now();
        let primitive_pixels = cyd.prepare_linkage_primitives::<{ LINKAGE.draw_item_count() }, _>(
            CLOCK_BOUNDS,
            background,
            LINKAGE.draw_items(&params),
            &PROJECTION,
        );
        let prepare_done = Instant::now();

        let primitives_started = Instant::now();
        cyd.fill_contiguous(primitive_pixels.bounds(), primitive_pixels.iter())
            .map_err(Error::Flush)?;
        let primitives_done = Instant::now();

        // info!(
        //     "clock timing: wait={}us text={}us params={}us prepare={}us primitives={}us active={}us total={}us",
        //     micros(tick_ready - loop_started),
        //     micros(text_done - text_started),
        //     micros(params_done - params_started),
        //     micros(prepare_done - prepare_started),
        //     micros(primitives_done - primitives_started),
        //     micros(primitives_done - tick_ready),
        //     micros(primitives_done - loop_started),
        );
    }
}

fn micros(duration: Duration) -> u64 {
    duration.as_micros()
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
    background: Rgb565,
    foreground: Rgb565,
    time_text_unscaled_buffer: &mut [u16; TIME_TEXT_UNSCALED_PIXELS],
    time_text_scaled_buffer: &mut [u16; TIME_TEXT_SCALED_PIXELS],
) {
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
fn text_12h(hour_12: u8, minute: u8, hour_24: u8) -> heapless::String<TIME_TEXT_CAPACITY> {
    let meridiem = if hour_24 % 24 < 12 { "AM" } else { "PM" };
    let mut text = heapless::String::new();
    core::fmt::write(&mut text, format_args!("{hour_12}:{minute:02} {meridiem}"))
        .expect("clock string fits in TIME_TEXT_CAPACITY bytes");
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
/// Both variants are converted explicitly at the call site (`.map_err(...)`),
/// the same flush-error convention as
/// [`skeleton_clock::Error`](crate::skeleton_clock::Error).
#[derive(Debug)]
pub enum Error<F> {
    /// Flushing a frame to the display failed.
    Flush(F),
}
