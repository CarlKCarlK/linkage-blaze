#![no_std]

use device_envoy_core::button::Button;
use device_envoy_core::cyd::display::Orientation;
use device_envoy_core::wasm::{CydSimulatorControlWasm, CydSimulatorWasm, next_animation_frame};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use linkage_blaze_core::examples::armatron::{ArmatronExit, BACKGROUND, FOREGROUND, armatron};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{HtmlCanvasElement, window};

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
        Orientation::Landscape,
        BACKGROUND,
        FOREGROUND,
        &FONT_6X10,
    )?;
    let (mut cyd, mut button, control) = simulator.into_parts();
    wasm_bindgen_futures::spawn_local(async move {
        loop {
            match armatron(&mut cyd, &mut button).await {
                // BOOT (or the on-screen "cal" button) requested calibration.
                // Real hardware clears calibration flash and reboots into a
                // fresh calibration pass; the browser touch mapping is always
                // exact, so restarting the app is the WASM equivalent. Wait
                // for BOOT to be released first so one held press cannot
                // become an endless sequence of restarts.
                Ok(ArmatronExit::CalibrationRequested) => {
                    while button.is_pressed() {
                        next_animation_frame().await;
                    }
                }
                Err(error) => {
                    drop(error);
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str("armatron stopped"));
                    break;
                }
            }
        }
    });
    Ok(control)
}
