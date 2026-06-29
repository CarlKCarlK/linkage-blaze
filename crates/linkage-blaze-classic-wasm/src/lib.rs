// The embedded `MOTION` capture is a heavy const; its evaluation happens here,
// where the generic `ballet::<CydWasm>` is instantiated, so the allow lives here
// (mirroring the esp32 `ballet` example).
#![allow(long_running_const_eval)]

//! Browser entry point for the "classic" CYD `ballet` example.
//!
//! Wires the page's `<canvas id="screen">` to a [`CydWasm`] and spawns the
//! device-agnostic [`ballet`] render loop. The loop is unchanged from the esp32
//! build; it paces itself to the browser via
//! [`CydFrameWasm::flush`](linkage_blaze_cyd_wasm::CydFrameWasm), which awaits
//! `requestAnimationFrame`.

use linkage_blaze_cyd_wasm::CydWasm;
use linkage_blaze_example_core::ballet::{BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, ballet};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

const SHOW_CASE_ALIGNMENT_CONTROLS: bool = false;

/// Whether the browser page should show the case/cord alignment tuner.
///
/// Set [`SHOW_CASE_ALIGNMENT_CONTROLS`] to `true` while adjusting the framed
/// device photo, then copy the generated CSS values back into `www/index.html`.
#[wasm_bindgen]
pub fn show_case_alignment_controls() -> bool {
    SHOW_CASE_ALIGNMENT_CONTROLS
}

/// Start the ballet animation on the canvas with `canvas_id`.
///
/// Sizes the canvas to the simulated portrait CYD panel, then spawns the render
/// loop. Returns once the loop is scheduled; the loop itself runs forever,
/// driven by `requestAnimationFrame`.
#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
    let document = web_sys::window()
        .expect("a browser window exists")
        .document()
        .expect("the window has a document");
    let canvas: HtmlCanvasElement = document
        .get_element_by_id(canvas_id)
        .expect("the canvas element exists")
        .dyn_into()
        .expect("the element is a <canvas>");

    let size = ORIENTATION.size();
    canvas.set_width(size.width);
    canvas.set_height(size.height);

    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .expect("the canvas supports a 2d context")
        .dyn_into()
        .expect("the context is a CanvasRenderingContext2d");

    let cyd = CydWasm::new(context, ORIENTATION, BACKGROUND, FOREGROUND, &TOP_FONT);

    // `async move` owns `cyd`, making the spawned future `'static` while `ballet`
    // borrows it for the whole run.
    wasm_bindgen_futures::spawn_local(async move {
        let mut cyd = cyd;
        match ballet(&mut cyd).await {
            Ok(never) => match never {},
            Err(error) => panic!("ballet stopped: {error:?}"),
        }
    });

    Ok(())
}
