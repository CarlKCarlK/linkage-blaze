#![no_std]

mod clock_sync;

use device_envoy_core::{
    button::Button,
    cyd::{CydDisplay, display::CydFrame},
    wasm::{
        ButtonWasm, CydSimulatorControlWasm, CydSimulatorWasm, SimulatorNoticeDisposition,
        SimulatorNoticeRequest, WifiConnectEvent, WifiConnectOutcome, next_animation_frame,
        simulate_wifi_connect, simulator_notice_disposition,
    },
};
use linkage_blaze_core::examples::clock::{
    BACKGROUND, Exit, FOREGROUND, ORIENTATION, WIFI_STATUS_FONT, WIFI_STATUS_RECTANGLE, clock,
    clock_splash,
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
    let (cyd, mut button, control) = simulator.into_parts();
    wasm_bindgen_futures::spawn_local(async move {
        let mut display = cyd.display();
        let clock_sync = clock_sync::BrowserClockSync::new();
        if let Err(error) = clock_splash(&mut display).await {
            drop(error);
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str("clock splash stopped"));
            return;
        }
        loop {
            let wifi_outcome = simulate_wifi_connect(&mut button, async |event| {
                let (notice_request, status) = match event {
                    WifiConnectEvent::CaptivePortalReady => {
                        (SimulatorNoticeRequest::wifi_setup(), "WiFi setup")
                    }
                    WifiConnectEvent::Connecting { .. } => {
                        (SimulatorNoticeRequest::wifi_connecting(), "WiFi ...")
                    }
                    WifiConnectEvent::ConnectionFailed => {
                        (SimulatorNoticeRequest::wifi_unavailable(), "WiFi fail")
                    }
                };
                if matches!(
                    simulator_notice_disposition(notice_request),
                    SimulatorNoticeDisposition::Terminate
                ) {
                    return Ok::<(), wasm_bindgen::JsValue>(());
                }
                display
                    .frame_mut(WIFI_STATUS_RECTANGLE)
                    .clear()
                    .write_text(status)
                    .flush()
                    .await
                    .map_err(|_error| wasm_bindgen::JsValue::from_str("Wi-Fi status failed"))
            })
            .await;
            let wifi_outcome = match wifi_outcome {
                Ok(wifi_outcome) => wifi_outcome,
                Err(error) => {
                    web_sys::console::error_1(&error);
                    return;
                }
            };
            if matches!(wifi_outcome, WifiConnectOutcome::ResetRequested) {
                wait_for_button_release(&button).await;
                continue;
            }
            if let Err(error) = display
                .frame_mut(WIFI_STATUS_RECTANGLE)
                .clear()
                .write_text("WiFi OK")
                .flush()
                .await
            {
                match error {}
            }
            match clock(&mut display, &clock_sync, &mut button).await {
                Ok(Exit::ResetWifi) => wait_for_button_release(&button).await,
                Err(_error) => {
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str("clock stopped"));
                    return;
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
