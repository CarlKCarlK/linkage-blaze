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
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    pixelcolor::{Rgb888, WebColors},
    prelude::{Point, Size},
    primitives::Rectangle,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use linkage_blaze_core::{LinkageFixed, LinkageView, Projection, linkage, linkage_fixed};
use linkage_blaze_cyd_core::{ContiguousPixels, Cyd, CydFrame, DrawItem3dExt, Orientation};
use log::info;
use profont::PROFONT_18_POINT;

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BACKGROUND: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
pub const FOREGROUND: Rgb888 = Rgb888::CSS_NAVY;

// ── Screen / region layout ─────────────────────────────────────────────────────

pub const ORIENTATION: Orientation = Orientation::Landscape;

const TIME_FONT: MonoFont<'static> = PROFONT_18_POINT;
const TIME_TEXT_CAPACITY: usize = 16;
const TIME_TEXT_TOP_PADDING: i32 = -1;

pub const WIFI_STATUS_FONT: MonoFont<'static> = FONT_6X10;

pub const WIFI_STATUS_REGION: Rectangle = Rectangle::new(Point::new(270, 6), Size::new(50, 10));
pub const TIME_REGION: Rectangle = Rectangle::new(Point::new(50, 0), Size::new(220, 20));

const CLOCK_BOUNDS: Rectangle = Rectangle::new(Point::new(50, 20), Size::new(220, 220));
const PROJECTION: Projection = Projection::top_orthographic(
    /* target origin */ Point::new(160, 130),
    /* scale */ 1.375,
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

        let draw_time_started = Instant::now();

        // Draw the time explicitly so the surface default font/color can stay
        // dedicated to the small WiFi status text.
        {
            let mut time_frame = cyd.frame_mut(TIME_REGION);
            draw_time(&mut time_frame, time_text.as_str(), background, foreground);
            time_frame.flush().await.map_err(Error::Flush)?;
        }
        let draw_time_done = Instant::now();

        let linkage_params_started = Instant::now();
        let params = linkage_params(hour_24, minute, second);
        let linkage_params_done = Instant::now();

        let contiguous_pixels =
            ContiguousPixels::<{ LINKAGE.draw_item_3d_count() }>::from_draw_items_2d(
                CLOCK_BOUNDS,
                background,
                LINKAGE
                    .draw_items_3d(&params)
                    .map(|draw_item_3d| draw_item_3d.project(&PROJECTION)),
            );

        let fill_contiguous_started = Instant::now();
        cyd.fill_contiguous(contiguous_pixels.bounds(), contiguous_pixels.iter())
            .map_err(Error::Flush)?;
        let fill_contiguous_done = Instant::now();

        info!(
            "clock timing: wait_for_tick={}us draw_time={}us linkage_params={}us from_draw_items_2d={}us fill_contiguous={}us active={}us total={}us",
            micros(tick_ready - loop_started),
            micros(draw_time_done - draw_time_started),
            micros(linkage_params_done - linkage_params_started),
            micros(fill_contiguous_started - linkage_params_done),
            micros(fill_contiguous_done - fill_contiguous_started),
            micros(fill_contiguous_done - tick_ready),
            micros(fill_contiguous_done - loop_started),
        );
    }
}

fn micros(duration: Duration) -> u64 {
    duration.as_micros()
}

fn draw_time<FrameError>(
    time_frame: &mut impl CydFrame<Error = FrameError>,
    text: &str,
    background: Rgb565,
    foreground: Rgb565,
) {
    let time_style = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Top)
        .build();
    time_frame.fill(background);
    Text::with_text_style(
        text,
        Point::new(
            time_frame.region().size.width as i32 / 2,
            TIME_TEXT_TOP_PADDING,
        ),
        MonoTextStyle::new(&TIME_FONT, foreground),
        time_style,
    )
    .draw(time_frame)
    .expect("drawing text into the time frame cannot fail");
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
