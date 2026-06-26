//! A browser-backed [`ClockSync`].
//!
//! [`WasmClockSync`] reads wall-clock time from the JavaScript `Date` object and
//! ticks once per second via an [`embassy_time::Timer`] (driven by embassy-time's
//! WASM driver). It is the minimum [`ClockSync`] the device-agnostic
//! [`skeleton_clock`](linkage_blaze_example_core::skeleton_clock) loop needs —
//! there is no NTP sync in the browser; the OS clock is already correct.

use core::cell::Cell;

use device_envoy_core::clock_sync::{ClockSync, ClockSyncTick, UnixSeconds};
use embassy_time::{Duration, Timer};
use time::{OffsetDateTime, UtcOffset};

/// One-second tick interval for the clock.
const TICK_INTERVAL: Duration = Duration::from_secs(1);

/// A [`ClockSync`] backed by the browser's local clock.
pub struct WasmClockSync {
    // The browser already knows the local zone; we cache its offset so the
    // trait's offset getter/setter behave, and honor it when forming local time.
    offset_minutes: Cell<i32>,
}

impl WasmClockSync {
    /// Build a clock seeded with the browser's current timezone offset.
    #[must_use]
    pub fn new() -> Self {
        // `Date::getTimezoneOffset` returns UTC-minus-local in minutes (e.g. -120
        // for UTC+2), so the east-positive offset is its negation.
        let offset_minutes = -(js_sys::Date::new_0().get_timezone_offset() as i32);
        Self {
            offset_minutes: Cell::new(offset_minutes),
        }
    }
}

impl Default for WasmClockSync {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockSync for WasmClockSync {
    async fn wait_for_tick(&self) -> ClockSyncTick {
        Timer::after(TICK_INTERVAL).await;
        ClockSyncTick {
            local_time: self.now_local(),
            // No NTP in the browser: the OS clock is the source of truth.
            since_last_sync: Duration::from_ticks(0),
        }
    }

    fn now_local(&self) -> OffsetDateTime {
        let unix_seconds = (js_sys::Date::now() / 1000.0) as i64;
        let utc = OffsetDateTime::from_unix_timestamp(unix_seconds)
            .expect("a current JavaScript timestamp is in range");
        let offset = UtcOffset::from_whole_seconds(self.offset_minutes.get() * 60)
            .expect("a timezone offset in minutes is in range");
        utc.to_offset(offset)
    }

    fn set_offset_minutes(&self, minutes: i32) {
        self.offset_minutes.set(minutes);
    }

    fn offset_minutes(&self) -> i32 {
        self.offset_minutes.get()
    }

    // The remaining controls are not needed by the browser clock: it always runs
    // at real time from the OS clock, with a fixed one-second tick.
    fn set_tick_interval(&self, _interval: Option<Duration>) {}

    fn set_speed(&self, _speed_multiplier: f32) {}

    fn set_utc_time(&self, _unix_seconds: UnixSeconds) {}
}
