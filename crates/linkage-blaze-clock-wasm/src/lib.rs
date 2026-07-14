#![no_std]

mod clock_sync;

use device_envoy_core::wasm::{CydSimulatorControlWasm, CydSimulatorWasm};
use linkage_blaze_core::examples::clock::{
    BACKGROUND, FOREGROUND, ORIENTATION, WIFI_STATUS_FONT, clock, clock_splash,
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
    let simulator = CydSimulatorWasm::new_with_style(
        canvas,
        ORIENTATION,
        BACKGROUND,
        FOREGROUND,
        &WIFI_STATUS_FONT,
    )?;
    let (cyd, _, control) = simulator.into_parts();
    wasm_bindgen_futures::spawn_local(async move {
        let mut display = cyd.display();
        let clock_sync = clock_sync::BrowserClockSync::new();
        if let Err(error) = clock_splash(&mut display).await {
            drop(error);
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str("clock splash stopped"));
            return;
        }
        match clock(&mut display, &clock_sync).await {
            Ok(never) => match never {},
            Err(error) => {
                drop(error);
                web_sys::console::error_1(&wasm_bindgen::JsValue::from_str("clock stopped"));
            }
        }
    });
    Ok(control)
}
