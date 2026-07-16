#![no_std]

use device_envoy_core::{
    button::Button,
    cyd::display::Orientation,
    wasm::{ButtonWasm, CydSimulatorControlWasm, CydSimulatorWasm, next_animation_frame},
};
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
                Ok(ArmatronExit::CalibrationRequested) => {
                    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(
                        "calibration is not simulated in the browser",
                    ));
                    wait_for_button_release(&button).await;
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

async fn wait_for_button_release(button: &ButtonWasm) {
    while button.is_pressed() {
        next_animation_frame().await;
    }
}
