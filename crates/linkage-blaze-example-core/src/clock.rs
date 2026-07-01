//! The generic "clock" example: an analog clock face whose hands are driven by a
//! tiny [`linkage`](linkage_blaze_core::linkage), with a digital time read-out
//! above it.

use core::{convert::Infallible, fmt, iter};

use device_envoy_core::clock_sync::{ClockSync, h12_m_s};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    pixelcolor::Rgb888,
    prelude::{Point, Size},
    primitives::Rectangle,
    text::{Alignment, Baseline, Text, TextStyle, TextStyleBuilder},
};
use linkage_blaze_core::{LinkageFixed, LinkageView, Projection, linkage, linkage_fixed};
use linkage_blaze_cyd_core::{
    BitmapItem565, ContiguousPixels, Cyd, CydFrame, DrawItem2d, DrawItem3dExt, Image565Fixed,
    Orientation, tga565, tiling::max_rectangle_pixel_count,
};
use log::info;
use profont::PROFONT_18_POINT;
use time::OffsetDateTime;

use crate::infallible::InfallibleResultExt;

// ── Public constants ────────────────────────────────────────────────────────────────

pub const BACKGROUND: Rgb888 = Rgb888::new(3, 7, 14); // near-black blue (3, 7, 14)
pub const FOREGROUND: Rgb888 = Rgb888::new(210, 160, 80); // dim gold (210, 160, 80)
pub const ORIENTATION: Orientation = Orientation::Landscape;
pub const WIFI_STATUS_FONT: MonoFont<'static> = FONT_6X10;
pub const WIFI_STATUS_RECTANGLE: Rectangle = Rectangle::new(Point::new(256, 5), Size::new(62, 10));
pub const MAX_FRAME_PIXEL_COUNT: usize =
    max_rectangle_pixel_count(WIFI_STATUS_RECTANGLE, TIME_RECTANGLE);

// ── Private constants ─────────────────────────────────────────────────────────

const TIME_RECTANGLE: Rectangle = Rectangle::new(Point::new(55, 0), Size::new(200, 22));
const TIME_COLOR: Rgb888 = Rgb888::new(255, 218, 118); // pale gold (255, 218, 118)
const TIME_FONT: MonoFont<'static> = PROFONT_18_POINT;
const TIME_TEXT_STYLE: TextStyle = TextStyleBuilder::new()
    .alignment(Alignment::Center)
    .baseline(Baseline::Top)
    .build();
const TIME_TEXT_CAPACITY: usize = 16;
const TIME_TEXT_TOP_PADDING: i32 = -1;

const CLOCK_BOUNDS: Rectangle = Rectangle::new(Point::new(50, 20), Size::new(220, 220));
const BACKGROUND_BITMAP_TOP_LEFT: Point = Point::zero();
const BACKGROUND_BITMAP_RECTANGLE: Rectangle =
    Rectangle::new(BACKGROUND_BITMAP_TOP_LEFT, Size::new(320, 240));
const BACKGROUND_BITMAP: Image565Fixed<320, 240, { 320 * 240 }> =
    tga565!("../assets/astronomy_window_background.tga", 320, 240);
const PROJECTION: Projection = Projection::top_orthographic(
    /* target origin */ Point::new(160, 130),
    /* scale */ 1.375,
);
const LINKAGE0: LinkageFixed<2, 2, 50> = linkage_fixed!("clock.lb.rs");
const LINKAGE: LinkageView<2, 2> = LINKAGE0.view();

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
    let time_color = Rgb565::from(TIME_COLOR);

    loop {
        // Wait for a tick and get the time.
        let tick = clock_sync.wait_for_tick().await;
        let local_time = &tick.local_time;
        let time_text = text_12h(local_time)?;
        info!("tick {}", time_text.as_str());

        // Write the time string in non-default font via embedded-graphics
        let mut time_frame = cyd.frame_mut(TIME_RECTANGLE);
        time_frame.fill(background);
        Text::with_text_style(
            time_text.as_str(),
            Point::new(TIME_RECTANGLE.size.width as i32 / 2, TIME_TEXT_TOP_PADDING),
            MonoTextStyle::new(&TIME_FONT, time_color),
            TIME_TEXT_STYLE,
        )
        .draw(&mut time_frame)
        .unwrap_never();
        time_frame.flush().await.map_err(Error::Flush)?;
        drop(time_frame);

        let params = linkage_params(local_time);

        let clock_background = DrawItem2d::Bitmap(BitmapItem565::new(
            BACKGROUND_BITMAP.view(),
            bitmap_source_for_screen_rectangle(CLOCK_BOUNDS),
            CLOCK_BOUNDS.top_left,
        ));
        let draw_items_2d = iter::once(clock_background).chain(
            LINKAGE
                .draw_items_3d(&params)
                .map(|draw_item_3d| draw_item_3d.project(&PROJECTION)),
        );
        let contiguous_pixels =
            ContiguousPixels::<{ LINKAGE.draw_item_3d_count() + 1 }>::from_draw_items_2d(
                CLOCK_BOUNDS,
                background,
                draw_items_2d,
            );

        cyd.fill_contiguous(contiguous_pixels.bounds(), contiguous_pixels.iter())
            .map_err(Error::Flush)?;
    }
}

/// Draw the static full-screen clock background.
pub async fn clock_splash<CydDevice>(cyd: &mut CydDevice) -> Result<(), Error<CydDevice::Error>>
where
    CydDevice: Cyd,
{
    cyd.fill_contiguous(BACKGROUND_BITMAP_RECTANGLE, BACKGROUND_BITMAP.rgb565_iter())
        .map_err(Error::Flush)?;
    Ok(())
}

/// Error from the generic clock loop, generic over the surface's flush error `F`.
///
/// Both variants are converted explicitly at the call site (`.map_err(...)`),
/// the same flush-error convention as
/// [`skeleton_clock::Error`](crate::skeleton_clock::Error).
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// Formatting the time string failed.
    Text(fmt::Error),
    /// Flushing a frame to the display failed.
    #[from(ignore)]
    Flush(F),
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn bitmap_source_for_screen_rectangle(screen_rectangle: Rectangle) -> Rectangle {
    Rectangle::new(
        screen_rectangle.top_left - BACKGROUND_BITMAP_TOP_LEFT,
        screen_rectangle.size,
    )
}

// ── Clock time ──────────────────────────────────────────────────────────────────

/// Format a 12-hour clock string with AM/PM.
fn text_12h(
    local_time: &OffsetDateTime,
) -> Result<heapless::String<TIME_TEXT_CAPACITY>, fmt::Error> {
    let (hour_12, minute, _) = h12_m_s(local_time);
    let meridiem = if local_time.hour() < 12 { "AM" } else { "PM" };
    let mut text = heapless::String::new();
    fmt::write(&mut text, format_args!("{hour_12}:{minute:02} {meridiem}"))?;
    Ok(text)
}

fn linkage_params(local_time: &OffsetDateTime) -> [f32; 2] {
    let (hour_12, minute, second) = h12_m_s(local_time);
    let second_turn = second as f32 / 60.0;
    let minute_turn = (minute as f32 + second_turn) / 60.0;
    let hour = ((hour_12 % 12) as f32 + minute_turn) / 12.0;
    let face_spin = (((second % 20) as f32) / 20.0 + 0.5) % 1.0;
    [hour, face_spin]
}
