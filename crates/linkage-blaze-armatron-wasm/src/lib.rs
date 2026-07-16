#![no_std]

use device_envoy_core::{
    cyd::display::Orientation,
    wasm::{ButtonWasm, CydWebAppConfig, CydWebAppHandle, CydWebCommand, start_cyd_web_app},
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

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<CydWebAppHandle, wasm_bindgen::JsValue> {
    start_cyd_web_app(canvas_id, WEB_APP, inner_main)
}

async fn inner_main(
    cyd: &mut device_envoy_core::wasm::CydWasm,
    button: &mut ButtonWasm,
) -> Result<CydWebCommand, linkage_blaze_core::examples::armatron::Error<core::convert::Infallible>>
{
    match armatron(cyd, button).await? {
        ArmatronExit::CalibrationRequested => Ok(CydWebCommand::CalibrationNotNeeded),
    }
}
