// The embedded skeleton `LINKAGE` is a heavy const; its evaluation happens here,
// where the generic `skeleton_clock::<CydWasm, _>` is instantiated, so the allow
// lives here (mirroring the esp32 examples).
#![allow(long_running_const_eval)]

//! Browser entry point for the "classic" CYD `skeleton_clock` example.
//!
//! Wires the page's `<canvas id="screen">` to a [`CydWasm`] and spawns the
//! device-agnostic [`skeleton_clock`] render loop, driven by a browser-clock
//! [`WasmClockSync`](crate::clock::WasmClockSync). The loop is unchanged from the
//! esp32 build; it paces itself via `CydFrameWasm::flush_at`
//! (`requestAnimationFrame`) and ticks once per second via embassy-time.

mod clock;

use clock::WasmClockSync;
use linkage_blaze_cyd_core::{Cyd, CydFrame};
use linkage_blaze_cyd_wasm::CydWasm;
use linkage_blaze_example_core::skeleton_clock::{
    BACKGROUND, FOREGROUND, ORIENTATION, TOP_FONT, WIFI_STATUS_POINT, WIFI_STATUS_SIZE,
    skeleton_clock,
};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// Start the skeleton-clock animation on the canvas with `canvas_id`.
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

    // `async move` owns `cyd` and the clock, making the spawned future `'static`
    // while `skeleton_clock` borrows them for the whole run.
    wasm_bindgen_futures::spawn_local(async move {
        let mut cyd = cyd;
        let clock_sync = WasmClockSync::new();
        // The browser uses the OS clock (no WiFi/NTP), but mirror the device's
        // status line so the framed display reads like the real one. Drawn once;
        // the per-tick loop only repaints the time and figure regions.
        cyd.frame_mut(WIFI_STATUS_SIZE)
            .write_text("WiFi: OK")
            .flush_at(WIFI_STATUS_POINT)
            .await
            .expect("flushing the Infallible wasm frame cannot fail");
        // `Ok` is `Infallible` (the loop never returns), so this binding is
        // irrefutable; only a `Mark` lookup failure can surface here.
        let Err(error) = skeleton_clock(&mut cyd, &clock_sync).await;
        web_sys::console::error_1(&format!("skeleton_clock stopped: {error:?}").into());
    });

    Ok(())
}
