#![no_std]

mod clock_sync {
    use core::{
        cell::Cell,
        sync::atomic::{AtomicI64, Ordering},
    };

    use device_envoy_core::clock_sync::{ClockSync, ClockSyncTick, UnixSeconds};
    use embassy_time::{Duration, Timer};
    use time::{OffsetDateTime, Time, UtcOffset};

    pub struct BrowserClockSync {
        offset_minutes: Cell<i32>,
    }

    static SELECTED_TIME_OF_DAY: AtomicI64 = AtomicI64::new(-1);

    pub fn set_time_of_day(seconds_of_day: i32) {
        SELECTED_TIME_OF_DAY.store(i64::from(seconds_of_day), Ordering::Relaxed);
    }

    impl BrowserClockSync {
        pub fn new() -> Self {
            Self {
                offset_minutes: Cell::new(-(js_sys::Date::new_0().get_timezone_offset() as i32)),
            }
        }
    }

    impl ClockSync for BrowserClockSync {
        async fn wait_for_tick(&self) -> ClockSyncTick {
            Timer::after(Duration::from_secs(1)).await;
            ClockSyncTick {
                local_time: self.now_local(),
                since_last_sync: Duration::from_secs(0),
            }
        }

        fn now_local(&self) -> OffsetDateTime {
            let unix_seconds = (js_sys::Date::now() / 1000.0) as i64;
            let Ok(utc) = OffsetDateTime::from_unix_timestamp(unix_seconds) else {
                return OffsetDateTime::UNIX_EPOCH;
            };
            let Ok(offset) = UtcOffset::from_whole_seconds(self.offset_minutes.get() * 60) else {
                return utc;
            };
            let local = utc.to_offset(offset);
            let selected_time_of_day = SELECTED_TIME_OF_DAY.load(Ordering::Relaxed);
            if !(0..86_400).contains(&selected_time_of_day) {
                return local;
            }
            let seconds_of_day = selected_time_of_day as u32;
            let hour = (seconds_of_day / 3600) as u8;
            let minute = ((seconds_of_day % 3600) / 60) as u8;
            let second = (seconds_of_day % 60) as u8;
            let Ok(time) = Time::from_hms(hour, minute, second) else {
                return local;
            };
            local.replace_time(time)
        }

        fn set_offset_minutes(&self, minutes: i32) {
            self.offset_minutes.set(minutes);
        }
        fn offset_minutes(&self) -> i32 {
            self.offset_minutes.get()
        }
        fn set_tick_interval(&self, _interval: Option<Duration>) {}
        fn set_speed(&self, _speed_multiplier: f32) {}
        fn set_utc_time(&self, _unix_seconds: UnixSeconds) {}
    }
}

use device_envoy_core::wasm::{CydSimulatorControlWasm, CydSimulatorWasm};
use linkage_blaze_core::examples::skeleton_clock::{
    BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, skeleton_clock, skeleton_clock_splash,
};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{HtmlCanvasElement, window};

#[wasm_bindgen]
pub fn show_case_alignment_controls() -> bool {
    false
}

#[wasm_bindgen]
pub fn set_time_of_day(seconds_of_day: i32) -> Result<(), wasm_bindgen::JsValue> {
    if seconds_of_day != -1 && !(0..86_400).contains(&seconds_of_day) {
        return Err(wasm_bindgen::JsValue::from_str(
            "time of day must be between 0 and 86399 seconds",
        ));
    }
    clock_sync::set_time_of_day(seconds_of_day);
    Ok(())
}

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<CydSimulatorControlWasm, wasm_bindgen::JsValue> {
    let document = window()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("browser window unavailable"))?
        .document()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("document unavailable"))?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("canvas element unavailable"))?
        .dyn_into::<HtmlCanvasElement>()?;
    let simulator =
        CydSimulatorWasm::new_with_style(canvas, ORIENTATION, BACKGROUND, FOREGROUND, &TOP_FONT)?;
    let (cyd, _, control) = simulator.into_parts();
    wasm_bindgen_futures::spawn_local(async move {
        let mut display = cyd.display();
        let clock_sync = clock_sync::BrowserClockSync::new();
        if let Err(error) = skeleton_clock_splash(&mut display).await {
            drop(error);
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
                "skeleton clock splash stopped",
            ));
            return;
        }
        match skeleton_clock(&mut display, &clock_sync).await {
            Ok(never) => match never {},
            Err(error) => {
                drop(error);
                web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
                    "skeleton clock stopped",
                ));
            }
        }
    });
    Ok(control)
}
