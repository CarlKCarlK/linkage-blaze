//! The generic "clock" example: an analog clock face whose hands are driven by a
//! tiny [`linkage`](linkage_blaze_core::linkage), with a digital time read-out
//! above it.

use core::{convert::Infallible, fmt, iter};

use device_envoy_core::clock_sync::{ClockSync, h12_m_s};
use device_envoy_core::cyd::{
    ContiguousPixels, CydDisplay, CydFrame, DrawItem, Image565Fixed, Image565View, Orientation,
    tga565, tiling::max_rectangle_pixel_count,
};
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
use linkage_blaze_cyd_3d::DrawItem3dExt;
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
const BACKGROUND_BITMAP_RECTANGLE: Rectangle = Rectangle::new(Point::zero(), Size::new(320, 240));
const BACKGROUND_BITMAP: Image565Fixed<320, 240, { 320 * 240 }> =
    tga565!("../assets/astronomy_window_background.tga", 320, 240);
const PROJECTION: Projection = Projection::top_orthographic(
    /* target origin */ Point::new(160, 130),
    /* scale */ 1.375,
);
const CLOCK_BACKGROUND_VIEW: Image565View = BACKGROUND_BITMAP.view_rect(CLOCK_BOUNDS);
const CLOCK_BACKGROUND_BITMAP: DrawItem = DrawItem::Bitmap {
    view: CLOCK_BACKGROUND_VIEW,
    top_left: CLOCK_BOUNDS.top_left,
};
const LINKAGE0: LinkageFixed<2, 2, 50> = linkage_fixed!("clock.lb.rs");
const LINKAGE: LinkageView<2, 2> = LINKAGE0.view();

/// Run the clock render loop forever, driven by `clock_sync` ticks and drawn
/// onto `cyd`.
pub async fn clock<CydDisplayDevice, ClockSyncDevice>(
    display: &mut CydDisplayDevice,
    clock_sync: &ClockSyncDevice,
) -> Result<Infallible, Error<CydDisplayDevice::Error>>
where
    CydDisplayDevice: CydDisplay,
    ClockSyncDevice: ClockSync,
{
    let background = Rgb565::from(BACKGROUND);
    let time_color = Rgb565::from(TIME_COLOR);

    loop {
        // ── Wait for a tick and get the time. ────────────────────────────────────────
        let tick = clock_sync.wait_for_tick().await;
        let local_time = &tick.local_time;
        let time_text = text_12h(local_time)?;
        info!("tick {}", time_text.as_str());

        // ── Render the digital time strip (using embedded graphics methods). ─────────
        let mut time_frame = display.frame_mut(TIME_RECTANGLE);
        time_frame.fill(background);
        Text::with_text_style(
            time_text.as_str(),
            Point::new(TIME_RECTANGLE.size.width as i32 / 2, TIME_TEXT_TOP_PADDING),
            MonoTextStyle::new(&TIME_FONT, time_color),
            TIME_TEXT_STYLE,
        )
        .draw(&mut time_frame)
        .unwrap_infallible();
        time_frame.flush().await.map_err(Error::Flush)?;
        drop(time_frame);

        // ── Stream the pixels of the updated clock ────────────────────────────────────────

        // Compute the time-dependent linkage parameters, then project the clock's
        // 3D draw items into pixel-space 2D draw items.
        let params = linkage_params(local_time);
        let draw_items_2d = LINKAGE
            .draw_items_3d(&params)
            .map(|draw_item_3d| draw_item_3d.project(&PROJECTION));

        // `ContiguousPixels` yields row-major colors for `fill_contiguous` without
        // allocating a frame or tile buffer. It includes a background bitmap.
        let contiguous_pixels =
            ContiguousPixels::<{ 1 + LINKAGE.draw_item_3d_count() }>::from_draw_items(
                CLOCK_BOUNDS,
                background, // color, but will be overridden by the bitmap background
                iter::once(CLOCK_BACKGROUND_BITMAP).chain(draw_items_2d),
            );

        display
            .fill_contiguous(contiguous_pixels.bounds(), contiguous_pixels.iter())
            .map_err(Error::Flush)?;
    }
}

/// Draw the static full-screen clock background.
pub async fn clock_splash<CydDisplayDevice>(
    display: &mut CydDisplayDevice,
) -> Result<(), Error<CydDisplayDevice::Error>>
where
    CydDisplayDevice: CydDisplay,
{
    display
        .fill_contiguous(BACKGROUND_BITMAP_RECTANGLE, BACKGROUND_BITMAP.rgb565_iter())
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

#[cfg(test)]
mod tests {
    use device_envoy_core::clock_sync::{ClockSync, ClockSyncTick, UnixSeconds};
    use device_envoy_core::cyd::{Cyd as _, CydDisplay, CydFrame};
    use device_envoy_core::memory::{MemoryCyd, assert_framebuffer_matches_expected_png};
    use futures_executor::block_on;
    use time::OffsetDateTime;

    use super::{
        BACKGROUND, FOREGROUND, ORIENTATION, WIFI_STATUS_FONT, WIFI_STATUS_RECTANGLE, clock,
        clock_splash,
    };

    /// A `ClockSync` test double that ticks instantly with a fixed time,
    /// rather than waiting on real NTP/timer infrastructure.
    struct FixedClockSync {
        local_time: OffsetDateTime,
    }

    impl ClockSync for FixedClockSync {
        async fn wait_for_tick(&self) -> ClockSyncTick {
            ClockSyncTick {
                local_time: self.local_time,
                since_last_sync: embassy_time::Duration::from_secs(0),
            }
        }

        fn now_local(&self) -> OffsetDateTime {
            self.local_time
        }

        fn set_offset_minutes(&self, _minutes: i32) {}

        fn offset_minutes(&self) -> i32 {
            0
        }

        fn set_tick_interval(&self, _interval: Option<embassy_time::Duration>) {}

        fn set_speed(&self, _speed_multiplier: f32) {}

        fn set_utc_time(&self, _unix_seconds: UnixSeconds) {}
    }

    #[test]
    fn clock_renders_expected_frame() {
        let mut memory_cyd = MemoryCyd::new(
            ORIENTATION.size(),
            BACKGROUND,
            FOREGROUND,
            &WIFI_STATUS_FONT,
        );
        memory_cyd.set_frame_budget(3);
        let clock_sync = FixedClockSync {
            local_time: OffsetDateTime::from_unix_timestamp(1_700_003_415)
                .expect("valid fixed timestamp"),
        };

        {
            let (mut display, _touch) = memory_cyd.parts();
            block_on(clock_splash(&mut display))
                .expect("clock splash should draw the static background");
            let background565 = display.background_565();
            block_on(
                display
                    .frame_mut(WIFI_STATUS_RECTANGLE)
                    .fill(background565)
                    .write_text("WiFi: OK")
                    .flush(),
            )
            .expect("wifi status frame should flush during setup");
        }

        let clock_result = {
            let (mut display, _touch) = memory_cyd.parts();
            block_on(clock(&mut display, &clock_sync))
        };
        clock_result.expect_err("the free-running loop should stop at the frame budget");

        assert_framebuffer_matches_expected_png(
            &memory_cyd,
            env!("CARGO_MANIFEST_DIR"),
            "clock.png",
        )
        .expect("rendered frame should match the golden image");
    }
}
