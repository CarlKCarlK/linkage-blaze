#![allow(long_running_const_eval)]

use core::convert::Infallible;

use device_envoy_core::wasm::cyd_web;
use linkage_blaze_core::examples::ballet::{
    self, BACKGROUND_COLOR, Error as BalletError, FOREGROUND_COLOR, ORIENTATION, TOP_FONT,
};
use wasm_bindgen::prelude::wasm_bindgen;

const WEB_APP: cyd_web::Config = cyd_web::Config::new(
    "linkage-blaze/ballet",
    ORIENTATION,
    BACKGROUND_COLOR,
    FOREGROUND_COLOR,
    &TOP_FONT,
);
const PAGE_INFO: cyd_web::PageInfo = cyd_web::PageInfo::new(
    "Ballet",
    "A motion-captured pirouette replayed as a linkage skeleton.",
    "A motion-captured pirouette converted into a linkage skeleton and replayed full screen.",
    "Sit back and watch.",
    "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/ballet.rs",
);

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<cyd_web::Handle, wasm_bindgen::JsValue> {
    cyd_web::start(canvas_id, WEB_APP, PAGE_INFO, inner_main)
}

async fn inner_main(
    capabilities: cyd_web::Capabilities,
) -> Result<cyd_web::Command, BalletError<Infallible>> {
    let cyd = capabilities.cyd;
    let mut button = capabilities.button;
    let mut display = cyd.display();
    match ballet::run(&mut display, &mut button).await {
        Ok(never) => match never {},
        Err(error) => Err(error),
    }
}
