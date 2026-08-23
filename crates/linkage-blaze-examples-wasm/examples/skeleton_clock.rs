#![no_std]

use core::convert::Infallible;

use device_envoy_core::cyd::{CydDisplay, display::CydFrame};
use device_envoy_core::{
    wasm::{WifiConnectOutcome, cyd_web},
    wifi_auto::WifiAutoEvent,
};
use linkage_blaze::examples::skeleton_clock::{
    self, BACKGROUND_COLOR, Error as SkeletonClockError, Exit, FOREGROUND_COLOR, ORIENTATION,
    TOP_FONT, WIFI_STATUS_RECTANGLE,
};
use wasm_bindgen::prelude::wasm_bindgen;

const WEB_APP: cyd_web::Config = cyd_web::Config::new(
    "linkage-blaze/skeleton-clock",
    ORIENTATION,
    BACKGROUND_COLOR,
    FOREGROUND_COLOR,
    &TOP_FONT,
);
const PAGE_INFO: cyd_web::PageInfo = cyd_web::PageInfo::new(
    "Skeleton Clock",
    "A motion-captured figure holds the hour and minute on placards.",
    "A clock told by a motion-captured figure whose placards show the hour and minute.",
    "It follows your local clock. Use the shared time control to scrub to any time of day.",
    "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze/src/examples/skeleton_clock.rs",
);

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<cyd_web::Handle, wasm_bindgen::JsValue> {
    cyd_web::start(canvas_id, WEB_APP, PAGE_INFO, inner_main)
}

async fn inner_main(
    capabilities: cyd_web::Capabilities,
) -> Result<cyd_web::Command, SkeletonClockError<Infallible>> {
    let cyd = capabilities.cyd;
    let mut button = capabilities.button;
    let clock_sync = capabilities.clock_sync;
    let wifi_simulator = capabilities.wifi_simulator;
    clock_sync.show();
    let mut display = cyd.display();
    skeleton_clock::splash(&mut display).await?;

    let wifi_outcome = match wifi_simulator
        .connect(&mut button, async |event| {
            let status = match event {
                WifiAutoEvent::CaptivePortalReady => "WiFi: setup SkelClock",
                WifiAutoEvent::Connecting { .. } => "WiFi: connecting",
                WifiAutoEvent::ConnectionFailed => "WiFi: connect failed",
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
    if matches!(wifi_outcome, WifiConnectOutcome::ResetRequested) {
        return Ok(cyd_web::Command::ResetWifi);
    }

    let mut wifi_frame = display.frame_mut(WIFI_STATUS_RECTANGLE);
    wifi_frame.clear().write_text("WiFi: OK");
    match wifi_frame.flush().await {
        Ok(()) => {}
        Err(error) => match error {},
    }
    match skeleton_clock::run(&mut display, &clock_sync, &mut button).await? {
        Exit::ResetWifi => Ok(cyd_web::Command::ResetWifi),
    }
}
