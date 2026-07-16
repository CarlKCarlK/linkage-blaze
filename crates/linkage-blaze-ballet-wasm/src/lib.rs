#![allow(long_running_const_eval)]

use device_envoy_core::wasm::{
    CydWebAppConfig, CydWebAppHandle, CydWebAppWasm, CydWebCommand, CydWebPageInfo,
    start_cyd_web_app,
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
const PAGE_INFO: CydWebPageInfo = CydWebPageInfo::new(
    "Ballet",
    "A motion-captured pirouette replayed as a linkage skeleton.",
    "A motion-captured pirouette converted into a linkage skeleton and replayed full screen.",
    "Sit back and watch.",
    "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/ballet.rs",
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
) -> Result<CydWebCommand, linkage_blaze_core::examples::ballet::Error<core::convert::Infallible>> {
    let mut display = cyd_web_app_wasm.cyd.display();
    match ballet(&mut display, &mut cyd_web_app_wasm.button).await {
        Ok(never) => match never {},
        Err(error) => Err(error),
    }
}
