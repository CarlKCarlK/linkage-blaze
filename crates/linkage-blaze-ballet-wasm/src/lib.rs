#![allow(long_running_const_eval)]

use device_envoy_core::wasm::{CydSimulatorControlWasm, CydSimulatorWasm};
use linkage_blaze_core::examples::ballet::{BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, ballet};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{HtmlCanvasElement, window};

#[wasm_bindgen]
pub fn show_case_alignment_controls() -> bool {
    false
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
    let (cyd, button, control) = simulator.into_parts();
    wasm_bindgen_futures::spawn_local(async move {
        let mut display = cyd.display();
        match ballet(&mut display, &button).await {
            Ok(never) => match never {},
            Err(error) => web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format!(
                "ballet stopped: {error:?}"
            ))),
        }
    });
    Ok(control)
}
