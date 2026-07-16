#![no_std]

use core::convert::Infallible;

use device_envoy_core::wasm::{
    ButtonWasm, CydDisplayWasm, CydWebAppConfig, CydWebAppHandle, CydWebCommand,
    WifiSimulatorWasm, start_cyd_display_web_app,
};
use device_envoy_core::{
    cyd::{CydDisplay, display::CydFrame},
    wasm::clock::ClockSyncWasm,
};
use linkage_blaze_core::examples::skeleton_clock::{
    BACKGROUND, Exit, FOREGROUND, ORIENTATION, TOP_FONT, WIFI_STATUS_RECTANGLE, skeleton_clock,
    skeleton_clock_splash,
};
use wasm_bindgen::prelude::wasm_bindgen;

const WEB_APP: CydWebAppConfig = CydWebAppConfig::new(
    "linkage-blaze/skeleton-clock",
    ORIENTATION,
    BACKGROUND,
    FOREGROUND,
    &TOP_FONT,
);

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
    device_envoy_core::wasm::clock::set_time_of_day(seconds_of_day);
    Ok(())
}

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<CydWebAppHandle, wasm_bindgen::JsValue> {
    start_cyd_display_web_app(canvas_id, WEB_APP, inner_main)
}

async fn inner_main(
    display: &mut CydDisplayWasm,
    button: &mut ButtonWasm,
) -> Result<CydWebCommand, linkage_blaze_core::examples::skeleton_clock::Error<Infallible>> {
    let clock_sync = ClockSyncWasm::new();
    skeleton_clock_splash(display).await?;

    let wifi_simulator = WifiSimulatorWasm::new(WEB_APP.storage_namespace);
    let wifi_outcome = match wifi_simulator
        .connect(button, async |event| {
            let status = match event {
                device_envoy_core::wifi_auto::WifiAutoEvent::CaptivePortalReady => {
                    "WiFi: setup SkelClock"
                }
                device_envoy_core::wifi_auto::WifiAutoEvent::Connecting { .. } => {
                    "WiFi: connecting"
                }
                device_envoy_core::wifi_auto::WifiAutoEvent::ConnectionFailed => {
                    "WiFi: connect failed"
                }
            };
            display
                .frame_mut(WIFI_STATUS_RECTANGLE)
                .clear()
                .write_text(status)
                .flush()
                .await
        })
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => match error {},
    };
    if matches!(
        wifi_outcome,
        device_envoy_core::wasm::WifiConnectOutcome::ResetRequested
    ) {
        return Ok(CydWebCommand::ResetWifi);
    }

    let mut wifi_frame = display.frame_mut(WIFI_STATUS_RECTANGLE);
    wifi_frame.clear().write_text("WiFi: OK");
    match wifi_frame.flush().await {
        Ok(()) => {}
        Err(error) => match error {},
    }
    match skeleton_clock(display, &clock_sync, button).await? {
        Exit::ResetWifi => Ok(CydWebCommand::ResetWifi),
    }
}
