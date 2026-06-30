//! A browser-backed [`ClockSync`].
//!
//! [`WasmClockSync`] reads wall-clock time from the JavaScript `Date` object and
//! ticks once per second via an [`embassy_time::Timer`] (driven by embassy-time's
//! WASM driver). It is the minimum [`ClockSync`] the device-agnostic
//! [`clock`](linkage_blaze_example_core::clock::clock) loop needs.

use core::cell::Cell;

use device_envoy_core::clock_sync::{ClockSync, ClockSyncTick, UnixSeconds};
use embassy_time::{Duration, Timer};
use time::{OffsetDateTime, Time, UtcOffset};

/// One-second tick interval for the clock.
const TICK_INTERVAL: Duration = Duration::from_secs(1);

thread_local! {
    /// When `Some(seconds_of_day)` (0..86400), the clock reports today's date
    /// with this manually-chosen time of day instead of the real wall clock.
    /// Driven by the page's time-of-day slider; `None` follows the OS clock.
    static TIME_OVERRIDE: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Override the reported time of day to `seconds_of_day` (0..86400), or pass
/// `None` to resume following the browser's real clock. The date stays today's.
pub fn set_time_override(seconds_of_day: Option<u32>) {
    TIME_OVERRIDE.with(|cell| cell.set(seconds_of_day));
}

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
        let local = utc.to_offset(offset);
        // The slider, when engaged, replaces only the time of day; the date and
        // zone stay as the real clock's.
        match TIME_OVERRIDE.with(Cell::get) {
            Some(seconds_of_day) => {
                let hour = (seconds_of_day / 3600) as u8;
                let minute = ((seconds_of_day % 3600) / 60) as u8;
                let second = (seconds_of_day % 60) as u8;
                let time = Time::from_hms(hour, minute, second)
                    .expect("seconds_of_day in 0..86400 yields a valid time");
                local.replace_time(time)
            }
            None => local,
        }
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
