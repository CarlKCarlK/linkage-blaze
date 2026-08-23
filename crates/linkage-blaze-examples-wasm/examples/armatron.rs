#![no_std]

use core::convert::Infallible;

use device_envoy_core::cyd::display::Orientation;
use device_envoy_core::wasm::cyd_web;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use linkage_blaze::examples::armatron::{
    self, BACKGROUND_COLOR, Error as ArmatronError, Exit, FOREGROUND_COLOR,
};
use wasm_bindgen::prelude::wasm_bindgen;

const WEB_APP: cyd_web::Config = cyd_web::Config::new(
    "linkage-blaze/armatron",
    Orientation::Landscape,
    BACKGROUND_COLOR,
    FOREGROUND_COLOR,
    &FONT_6X10,
);
const PAGE_INFO: cyd_web::PageInfo = cyd_web::PageInfo::new(
    "Armatron",
    "A six-joint robot arm driven by inverse kinematics.",
    "A robot arm with six joints, modeled as a linkage and driven by inverse kinematics.",
    "Drag the controls on the panel to pose the arm or run the solver.",
    "https://github.com/CarlKCarlK/linkage-blaze/blob/main/crates/linkage-blaze/src/examples/armatron/main.rs",
);

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<cyd_web::Handle, wasm_bindgen::JsValue> {
    cyd_web::start(canvas_id, WEB_APP, PAGE_INFO, inner_main)
}

async fn inner_main(
    capabilities: cyd_web::Capabilities,
) -> Result<cyd_web::Command, ArmatronError<Infallible>> {
    let mut cyd = capabilities.cyd;
    let mut button = capabilities.button;
    match armatron::run(&mut cyd, &mut button).await? {
        Exit::CalibrationRequested => Ok(cyd_web::Command::CalibrationNotNeeded),
    }
}
