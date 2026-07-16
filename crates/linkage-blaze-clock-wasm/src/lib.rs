#![no_std]

use core::convert::Infallible;

use device_envoy_core::cyd::{CydDisplay, display::CydFrame};
use device_envoy_core::wasm::{
    CydWebAppConfig, CydWebAppHandle, CydWebAppWasm, CydWebCommand, CydWebPageInfo,
    start_cyd_web_app,
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
const PAGE_INFO: CydWebPageInfo = CydWebPageInfo::new(
    "Clock",
    "An analog linkage clock with a digital strip and WiFi status.",
    "An analog clock whose hands are a tiny linkage posed by the time of day.",
    "It follows your local clock. Use the shared time control to scrub to any time of day.",
    "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/clock.rs",
);

#[wasm_bindgen]
pub fn show_case_alignment_controls() -> bool {
    false
}

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<CydWebAppHandle, wasm_bindgen::JsValue> {
    start_cyd_web_app(canvas_id, WEB_APP, PAGE_INFO, inner_main)
}

async fn inner_main(
    mut cyd_web_app_wasm: CydWebAppWasm,
) -> Result<CydWebCommand, linkage_blaze_core::examples::clock::Error<Infallible>> {
    cyd_web_app_wasm.clock_sync.show();
    let mut display = cyd_web_app_wasm.cyd.display();
    clock_splash(&mut display).await?;

    let wifi_outcome = match cyd_web_app_wasm
        .wifi_simulator
        .connect(&mut cyd_web_app_wasm.button, async |event| {
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
    match clock(
        &mut display,
        &cyd_web_app_wasm.clock_sync,
        &mut cyd_web_app_wasm.button,
    )
    .await?
    {
        Exit::ResetWifi => Ok(CydWebCommand::ResetWifi),
    }
}
