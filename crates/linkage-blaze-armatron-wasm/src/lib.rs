#![no_std]

use device_envoy_core::cyd::CydParts;
use device_envoy_core::cyd::display::Orientation;
use device_envoy_core::cyd::touch::calibration::{
    CalibrationConfig, EnsureCalibrationSettings, ensure_calibration_with_settings,
};
use device_envoy_core::flash_block::FlashBlock;
use device_envoy_core::wasm::{CydSimulatorControlWasm, CydSimulatorWasm, CydWasm, FlashBlockWasm};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use linkage_blaze_core::examples::armatron::{ArmatronExit, BACKGROUND, FOREGROUND, armatron};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{HtmlCanvasElement, window};

const CALIBRATION_STORAGE_KEY: &str = "linkage-blaze/armatron/calibration";
// The default calibration flow settings assume ESP/RP frame pacing; the
// browser needs more frames to reach the verify hold.
const BROWSER_VERIFY_TIMEOUT_FRAMES: usize = 10 * 60;

/// Exact inverse of `distort_demo_screen_to_raw`'s affine distortion, so a
/// fresh browser session starts already calibrated instead of forcing every
/// first-time visitor through the four-tap flow. Tapping the on-screen "cal"
/// button (or BOOT) still clears this and re-runs the real calibration flow.
const PREVIEW_DEFAULT_CALIBRATION: CalibrationConfig = CalibrationConfig::new(
    0.891_909,
    -0.039_321,
    -160.036_33,
    0.025_894,
    1.074_127,
    -164.861_27,
);

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
    let mut calibration_flash_block = FlashBlockWasm::new(CALIBRATION_STORAGE_KEY)
        .map_err(|_error| wasm_bindgen::JsValue::from_str("calibration flash unavailable"))?;

    if calibration_flash_block
        .load::<CalibrationConfig>()
        .unwrap_or(None)
        .is_none()
        && calibration_flash_block
            .save(&PREVIEW_DEFAULT_CALIBRATION)
            .is_err()
    {
        web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
            "failed to seed default calibration",
        ));
    }

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            let (mut display, uncalibrated_touch) = cyd.parts_uncalibrated();
            let touch = match ensure_calibration_with_settings(
                &mut display,
                uncalibrated_touch,
                &mut calibration_flash_block,
                &mut button,
                Some("Touch calibrated"),
                EnsureCalibrationSettings::new(BROWSER_VERIFY_TIMEOUT_FRAMES),
            )
            .await
            {
                Ok((touch, outcome)) => {
                    // A freshly saved calibration means the exercise UI just
                    // ran on this display; rebuild from a clean pass instead
                    // of trusting its leftover transient state.
                    if outcome.was_saved() {
                        cyd = CydWasm::from_parts(display, touch);
                        continue;
                    }
                    touch
                }
                Err(error) => {
                    drop(error);
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
                        "armatron calibration failed",
                    ));
                    break;
                }
            };
            cyd = CydWasm::from_parts(display, touch);

            match armatron(&mut cyd, &mut button).await {
                // BOOT (or the on-screen "cal" button) requested calibration.
                // Real hardware clears calibration flash and reboots, which
                // re-enters the calibration exercise on startup; clear flash
                // and loop back into ensure_calibration as the WASM
                // equivalent.
                Ok(ArmatronExit::CalibrationRequested) => {
                    if calibration_flash_block.clear().is_err() {
                        web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
                            "armatron calibration clear failed",
                        ));
                        break;
                    }
                    // A stray queued "up" from the touch/BOOT that triggered
                    // this exit may still be sitting in the queue, but the
                    // calibration exercise's Armed state only reacts to a
                    // fresh "down" and silently ignores an "up", so it is
                    // safe to leave as is. Explicitly clearing it here (via
                    // wait_for_fresh_press) also discards a genuinely fresh
                    // down that arrives before the exercise starts, which
                    // then reads as a swallowed first tap on the first
                    // target.
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
