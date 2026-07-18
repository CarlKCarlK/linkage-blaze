//! The generic "clock" example: an analog clock face whose hands are driven by a
//! tiny [`linkage`](crate::linkage), with a digital time read-out
//! above it.

use core::{fmt, iter};

use crate::{
    DrawItem3dExt, Error as LinkageError, LinkageFixed, LinkageView, Projection, linkage,
    linkage_fixed,
};
use device_envoy_core::{
    UnwrapInfallible,
    button::Button,
    clock_sync::{ClockSync, h12_m_s},
    cyd::{
        CydDisplay,
        display::{
            CydFrame, DrawItem, Image565Fixed, Image565View, Orientation, tga,
            tiling::max_rectangle_pixel_count,
        },
    },
};
use embassy_futures::select::{Either, select};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    pixelcolor::Rgb888,
    prelude::{Point, Size},
    primitives::Rectangle,
    text::{Alignment, Baseline, Text, TextStyle, TextStyleBuilder},
};
use log::info;
use profont::PROFONT_18_POINT;
use time::OffsetDateTime;

// ── Public constants ────────────────────────────────────────────────────────────────

pub const BACKGROUND_COLOR: Rgb888 = Rgb888::new(3, 7, 14); // near-black blue (3, 7, 14)
pub const FOREGROUND_COLOR: Rgb888 = Rgb888::new(210, 160, 80); // dim gold (210, 160, 80)
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
const BACKGROUND_BITMAP: Image565Fixed<320, 240, { 320 * 240 }> =
    tga!("../assets/astronomy_window_background.tga").to_565();
const BACKGROUND_BITMAP_VIEW: Image565View = BACKGROUND_BITMAP.view();
const PROJECTION: Projection = Projection::top_orthographic(
    Point::new(160, 130), // target origin
    1.375,                // scale
);
const CLOCK_BACKGROUND_VIEW: Image565View = BACKGROUND_BITMAP.view_rect(CLOCK_BOUNDS);
const CLOCK_BACKGROUND_BITMAP: DrawItem = DrawItem::Bitmap {
    view: CLOCK_BACKGROUND_VIEW,
    top_left: CLOCK_BOUNDS.top_left,
};
const LINKAGE0: LinkageFixed<2, 2, 50> = linkage_fixed!("../assets/examples/clock.lb.rs");
const LINKAGE: LinkageView<2, 2> = LINKAGE0.view();

/// Run the clock render loop forever, driven by `clock_sync` ticks and drawn
/// onto `cyd`.
pub async fn run<CydDisplayDevice, ClockSyncDevice>(
    display: &mut CydDisplayDevice,
    clock_sync: &ClockSyncDevice,
    button: &mut impl Button,
) -> Result<Exit, Error<CydDisplayDevice::Error>>
where
    CydDisplayDevice: CydDisplay,
    ClockSyncDevice: ClockSync,
{
    let background565 = Rgb565::from(BACKGROUND_COLOR);
    let time_color = Rgb565::from(TIME_COLOR);

    loop {
        // ── Wait for a tick and get the time. ────────────────────────────────────────
        let tick = match select(button.wait_for_press(), clock_sync.wait_for_tick()).await {
            Either::First(()) => return Ok(Exit::ResetWifi),
            Either::Second(tick) => tick,
        };
        let local_time = &tick.local_time;
        let time_text = text_12h(local_time)?;
        info!("tick {}", time_text.as_str());

        // ── Render the digital time strip (using embedded graphics methods). ─────────
        let mut time_frame = display.frame_mut(TIME_RECTANGLE);
        time_frame.fill(background565);
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
            .draw_items_3d(&params)?
            .map(|draw_item_3d| draw_item_3d.project(&PROJECTION));

        // Stream the pixels row-major straight to the display with no frame or
        // tile buffer, with the background_bitmap as the first pixel source.
        display
            .draw_items::<{ 1 + LINKAGE.draw_item_3d_count() }>(
                CLOCK_BOUNDS,
                background565, // color, but will be overridden by the background_bitmap
                iter::once(CLOCK_BACKGROUND_BITMAP).chain(draw_items_2d),
            )
            .map_err(Error::Flush)?;
    }
}

/// Draw the static full-screen clock background_bitmap.
pub async fn splash<CydDisplayDevice>(
    display: &mut CydDisplayDevice,
) -> Result<(), Error<CydDisplayDevice::Error>>
where
    CydDisplayDevice: CydDisplay,
{
    display
        .fill_contiguous_full(BACKGROUND_BITMAP_VIEW.rgb565_iter())
        .map_err(Error::Flush)?;
    Ok(())
}

/// Actions requested by the Clock's physical BOOT button.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Exit {
    /// Return to Wi-Fi setup before resuming the clock.
    ResetWifi,
}

/// Error from the generic clock loop, generic over the surface's flush error `F`.
///
/// Both variants are converted explicitly at the call site (`.map_err(...)`),
/// the same flush-error convention as
/// [`skeleton_clock::Error`](crate::skeleton_clock::Error).
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// A runtime linkage parameter was invalid.
    Linkage(LinkageError),
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
    use core::cell::Cell;

    use device_envoy_core::button::{__ButtonMonitor, Button};
    use device_envoy_core::clock_sync::{ClockSync, ClockSyncTick, UnixSeconds};
    use device_envoy_core::cyd::{CydDisplay, display::CydFrame};
    use device_envoy_core::memory::{CydMemory, assert_framebuffer_matches_expected_png};
    use futures_executor::block_on;
    use time::OffsetDateTime;

    use super::{
        BACKGROUND_COLOR, Exit, FOREGROUND_COLOR, ORIENTATION, WIFI_STATUS_FONT,
        WIFI_STATUS_RECTANGLE, run, splash,
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

    struct ImmediateButton;

    impl __ButtonMonitor for ImmediateButton {
        fn is_pressed_raw(&self) -> bool {
            false
        }

        async fn wait_until_pressed_state(&mut self, _pressed: bool) {}
    }

    impl Button for ImmediateButton {
        async fn wait_for_press(&mut self) {}
    }

    #[test]
    fn boot_requests_wifi_reset_before_rendering_the_next_tick() {
        let memory_cyd = CydMemory::new(
            ORIENTATION.size(),
            BACKGROUND_COLOR,
            FOREGROUND_COLOR,
            &WIFI_STATUS_FONT,
        );
        let clock_sync = FixedClockSync {
            local_time: OffsetDateTime::from_unix_timestamp(1_700_003_415)
                .expect("valid fixed timestamp"),
        };
        let mut button = ImmediateButton;

        let result = {
            let mut display = memory_cyd.display();
            block_on(run(&mut display, &clock_sync, &mut button))
        };

        assert_eq!(
            result.expect("BOOT should be a typed exit"),
            Exit::ResetWifi
        );
    }

    #[test]
    fn boot_requests_wifi_reset_after_a_rendered_tick() {
        let mut memory_cyd = CydMemory::new(
            ORIENTATION.size(),
            BACKGROUND_COLOR,
            FOREGROUND_COLOR,
            &WIFI_STATUS_FONT,
        );
        memory_cyd.set_frame_budget(100);
        let clock_sync = OneTickClockSync {
            local_time: OffsetDateTime::from_unix_timestamp(1_700_003_415)
                .expect("valid fixed timestamp"),
            ticks: Cell::new(0),
        };
        let mut button = AfterTickButton {
            waits: Cell::new(0),
        };

        let result = {
            let mut display = memory_cyd.display();
            block_on(run(&mut display, &clock_sync, &mut button))
        };

        assert_eq!(
            result.expect("BOOT should exit after a rendered tick"),
            Exit::ResetWifi
        );
        assert!(memory_cyd.flush_count() > 0);
    }

    #[test]
    fn clock_renders_expected_frame() {
        let mut memory_cyd = CydMemory::new(
            ORIENTATION.size(),
            BACKGROUND_COLOR,
            FOREGROUND_COLOR,
            &WIFI_STATUS_FONT,
        );
        memory_cyd.set_frame_budget(3);
        let clock_sync = FixedClockSync {
            local_time: OffsetDateTime::from_unix_timestamp(1_700_003_415)
                .expect("valid fixed timestamp"),
        };
        let mut memory_button = NeverButton;

        {
            let mut display = memory_cyd.display();
            block_on(splash(&mut display))
                .expect("clock splash should draw the static background_bitmap");
            block_on(
                display
                    .frame_mut(WIFI_STATUS_RECTANGLE)
                    .clear()
                    .write_text("WiFi: OK")
                    .flush(),
            )
            .expect("wifi status frame should flush during setup");
        }

        let clock_result = {
            let mut display = memory_cyd.display();
            block_on(run(&mut display, &clock_sync, &mut memory_button))
        };
        clock_result.expect_err("the free-running loop should stop at the frame budget");

        assert_framebuffer_matches_expected_png(
            &memory_cyd,
            env!("CARGO_MANIFEST_DIR"),
            "clock.png",
        )
        .expect("rendered frame should match the golden image");
    }

    struct NeverButton;

    impl __ButtonMonitor for NeverButton {
        fn is_pressed_raw(&self) -> bool {
            false
        }

        async fn wait_until_pressed_state(&mut self, _pressed: bool) {}
    }

    impl Button for NeverButton {
        async fn wait_for_press(&mut self) {
            core::future::pending().await
        }
    }

    struct AfterTickButton {
        waits: Cell<u8>,
    }

    impl __ButtonMonitor for AfterTickButton {
        fn is_pressed_raw(&self) -> bool {
            false
        }

        async fn wait_until_pressed_state(&mut self, _pressed: bool) {}
    }

    impl Button for AfterTickButton {
        async fn wait_for_press(&mut self) {
            let wait_number = self.waits.get();
            self.waits.set(wait_number + 1);
            if wait_number == 0 {
                core::future::pending().await
            }
        }
    }

    struct OneTickClockSync {
        local_time: OffsetDateTime,
        ticks: Cell<u8>,
    }

    impl ClockSync for OneTickClockSync {
        async fn wait_for_tick(&self) -> ClockSyncTick {
            if self.ticks.replace(1) == 0 {
                ClockSyncTick {
                    local_time: self.local_time,
                    since_last_sync: embassy_time::Duration::from_secs(0),
                }
            } else {
                core::future::pending().await
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
}
