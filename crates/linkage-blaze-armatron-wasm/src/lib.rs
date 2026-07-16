#![no_std]

use device_envoy_core::{
    cyd::display::Orientation,
    wasm::{
        CydWebAppConfig, CydWebAppHandle, CydWebAppWasm, CydWebCommand, CydWebPageInfo,
        start_cyd_web_app,
    },
};
use embedded_graphics::mono_font::ascii::FONT_6X10;
use linkage_blaze_core::examples::armatron::{ArmatronExit, BACKGROUND, FOREGROUND, armatron};
use wasm_bindgen::prelude::wasm_bindgen;

const WEB_APP: CydWebAppConfig = CydWebAppConfig::new(
    "linkage-blaze/armatron",
    Orientation::Landscape,
    BACKGROUND,
    FOREGROUND,
    &FONT_6X10,
);
const PAGE_INFO: CydWebPageInfo = CydWebPageInfo::new(
    "Armatron",
    "A six-joint robot arm driven by inverse kinematics.",
    "A robot arm with six joints, modeled as a linkage and driven by inverse kinematics.",
    "Drag the controls on the panel to pose the arm or run the solver.",
    "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/armatron/main.rs",
);

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<CydWebAppHandle, wasm_bindgen::JsValue> {
    start_cyd_web_app(canvas_id, WEB_APP, PAGE_INFO, inner_main)
}

async fn inner_main(
    mut cyd_web_app_wasm: CydWebAppWasm,
) -> Result<CydWebCommand, linkage_blaze_core::examples::armatron::Error<core::convert::Infallible>>
{
    match armatron(&mut cyd_web_app_wasm.cyd, &mut cyd_web_app_wasm.button).await? {
        ArmatronExit::CalibrationRequested => Ok(CydWebCommand::CalibrationNotNeeded),
    }
}
