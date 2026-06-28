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

use embedded_graphics::mono_font::ascii::FONT_6X10;
use linkage_blaze_cyd_core::Orientation;
use linkage_blaze_cyd_wasm::CydWasm;
use linkage_blaze_example_core::ballet::{BACKGROUND, FOREGROUND, ballet};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

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

    let size = Orientation::Portrait.size();
    canvas.set_width(size.width);
    canvas.set_height(size.height);

    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .expect("the canvas supports a 2d context")
        .dyn_into()
        .expect("the context is a CanvasRenderingContext2d");

    let cyd = CydWasm::new(
        context,
        Orientation::Portrait,
        BACKGROUND,
        FOREGROUND,
        &FONT_6X10,
    );

    // `async move` owns `cyd`, making the spawned future `'static` while `ballet`
    // borrows it for the whole run.
    wasm_bindgen_futures::spawn_local(async move {
        let mut cyd = cyd;
        // `ballet` loops forever and `CydWasm::Error` is `Infallible`, so its
        // `Result<Infallible, Infallible>` is uninhabited: both arms bind an
        // `Infallible` that the empty `match` discharges without any code.
        match ballet(&mut cyd).await {
            Ok(never) | Err(never) => match never {},
        }
    });

    Ok(())
}
