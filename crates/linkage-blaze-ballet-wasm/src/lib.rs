#![allow(long_running_const_eval)]

use device_envoy_core::wasm::{
    ButtonWasm, CydDisplayWasm, CydWebAppConfig, CydWebAppHandle, CydWebCommand,
    start_cyd_display_web_app,
};
use linkage_blaze_core::examples::ballet::{BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, ballet};
use wasm_bindgen::prelude::wasm_bindgen;

const WEB_APP: CydWebAppConfig = CydWebAppConfig::new(
    "linkage-blaze/ballet",
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
pub fn start(canvas_id: &str) -> Result<CydWebAppHandle, wasm_bindgen::JsValue> {
    start_cyd_display_web_app(canvas_id, WEB_APP, inner_main)
}

async fn inner_main(
    display: &mut CydDisplayWasm,
    button: &mut ButtonWasm,
) -> Result<CydWebCommand, linkage_blaze_core::examples::ballet::Error<core::convert::Infallible>> {
    match ballet(display, button).await {
        Ok(never) => match never {},
        Err(error) => Err(error),
    }
}
