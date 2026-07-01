//! Browser entry point for the "classic" CYD `clock` example.
//!
//! Wires the page's `<canvas id="screen">` to a [`CydWasm`] and spawns the
//! device-agnostic [`clock`] render loop, driven by a browser-clock
//! [`WasmClockSync`](crate::clock::WasmClockSync). The loop is unchanged from the
//! esp32 build; it paces itself via `CydFrameWasm::flush`
//! (`requestAnimationFrame`) and ticks once per second via embassy-time.

mod clock;

use clock::WasmClockSync;
use linkage_blaze_cyd_core::{Cyd, CydFrame};
use linkage_blaze_cyd_wasm::CydWasm;
use linkage_blaze_example_core::clock::{
    BACKGROUND, FOREGROUND, ORIENTATION, WIFI_STATUS_FONT, WIFI_STATUS_REGION, clock, clock_splash,
};
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

/// Start the clock animation on the canvas with `canvas_id`.
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

    let cyd = CydWasm::new(
        context,
        ORIENTATION,
        BACKGROUND,
        FOREGROUND,
        &WIFI_STATUS_FONT,
    );

    // `async move` owns `cyd` and the clock, making the spawned future `'static`
    // while `clock` borrows them for the whole run.
    wasm_bindgen_futures::spawn_local(async move {
        let mut cyd = cyd;
        let clock_sync = WasmClockSync::new();
        clock_splash(&mut cyd)
            .await
            .expect("flushing the Infallible wasm background cannot fail");
        cyd.frame_mut(WIFI_STATUS_REGION)
            .clear()
            .write_text("WiFi: OK")
            .flush()
            .await
            .expect("flushing the Infallible wasm frame cannot fail");
        let Err(error) = clock(&mut cyd, &clock_sync).await;
        web_sys::console::error_1(&format!("clock stopped: {error:?}").into());
    });

    Ok(())
}

/// Drive the displayed time of day from the page's slider: `seconds_of_day` is
/// `0..86400` (midnight to midnight). Pass a negative value to release the
/// override and resume the browser's real clock. The change is picked up on the
/// next one-second tick.
#[wasm_bindgen]
pub fn set_time_of_day(seconds_of_day: i32) {
    let seconds_of_day = if (0..86_400).contains(&seconds_of_day) {
        Some(seconds_of_day as u32)
    } else {
        None
    };
    clock::set_time_override(seconds_of_day);
}
