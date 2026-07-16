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
use linkage_blaze_core::examples::clock::{
    BACKGROUND, Exit, FOREGROUND, ORIENTATION, WIFI_STATUS_FONT, WIFI_STATUS_RECTANGLE, clock,
    clock_splash,
};
use wasm_bindgen::prelude::wasm_bindgen;

const WEB_APP: CydWebAppConfig = CydWebAppConfig::new(
    "linkage-blaze/clock",
    ORIENTATION,
    BACKGROUND,
    FOREGROUND,
    &WIFI_STATUS_FONT,
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
) -> Result<CydWebCommand, linkage_blaze_core::examples::clock::Error<Infallible>> {
    let clock_sync = ClockSyncWasm::new();
    clock_splash(display).await?;

    let wifi_simulator = WifiSimulatorWasm::new(WEB_APP.storage_namespace);
    let wifi_outcome = match wifi_simulator
        .connect(button, async |event| {
            let status = match event {
                device_envoy_core::wifi_auto::WifiAutoEvent::CaptivePortalReady => "WiFi setup",
                device_envoy_core::wifi_auto::WifiAutoEvent::Connecting { .. } => "WiFi ...",
                device_envoy_core::wifi_auto::WifiAutoEvent::ConnectionFailed => "WiFi fail",
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
    wifi_frame.clear().write_text("WiFi OK");
    match wifi_frame.flush().await {
        Ok(()) => {}
        Err(error) => match error {},
    }
    match clock(display, &clock_sync, button).await? {
        Exit::ResetWifi => Ok(CydWebCommand::ResetWifi),
    }
}
