#![forbid(unsafe_code)]

use device_envoy_core::flash_block::FlashBlock as _;
use embedded_graphics::mono_font::ascii::FONT_9X15_BOLD;
use linkage_blaze_core::{LinkageFixed, Pose, Rgb888, Vec3, linkage, linkage_fixed};
use linkage_blaze_cyd_core::{Orientation, ensure_calibration};
use linkage_blaze_cyd_wasm::{CydTouchWasmSource, CydWasm, CydWasmCalibrationFlashBlock};
use linkage_blaze_example_core::{
    armatron::{ArmatronOutcome, BACKGROUND, FOREGROUND, armatron},
    infallible::InfallibleResultExt,
};
use wasm_bindgen::{JsCast, closure::Closure, prelude::wasm_bindgen};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, PointerEvent};

const ORIENTATION: Orientation = Orientation::Landscape;

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

    let touch_source = CydTouchWasmSource::new();
    install_touch_handlers(&canvas, touch_source.clone())?;
    let mut calibration_flash_block = CydWasmCalibrationFlashBlock::new_precalibrated();

    wasm_bindgen_futures::spawn_local(async move {
        loop {
            let mut cyd = CydWasm::new_with_touch_source(
                context.clone(),
                ORIENTATION,
                BACKGROUND,
                FOREGROUND,
                &FONT_9X15_BOLD,
                touch_source.clone(),
            );

            match ensure_calibration(&mut cyd, &mut calibration_flash_block, || false).await {
                Ok(calibration_outcome) => {
                    if calibration_outcome.was_saved() {
                        continue;
                    }
                    let calibration_config = calibration_outcome.calibration_config();
                    cyd.set_calibration(calibration_config);
                }
                Err(infallible) => match infallible {},
            }

            match armatron(&mut cyd).await {
                Ok(ArmatronOutcome::CalibrateRequested) => {
                    cyd.clear_calibration();
                    calibration_flash_block.clear().unwrap_infallible();
                    touch_source.wait_for_fresh_press();
                }
                Err(error) => {
                    web_sys::console::error_1(&format!("armatron stopped: {error:?}").into());
                    break;
                }
            }
        }
    });

    Ok(())
}

fn install_touch_handlers(
    canvas: &HtmlCanvasElement,
    touch_source: CydTouchWasmSource,
) -> Result<(), wasm_bindgen::JsValue> {
    let pointer_down_canvas = canvas.clone();
    let pointer_down_source = touch_source.clone();
    let pointer_down = Closure::wrap(Box::new(move |event: PointerEvent| {
        event.prevent_default();
        if let Err(error) = pointer_down_canvas.set_pointer_capture(event.pointer_id()) {
            web_sys::console::error_1(&error);
        }
        let (x, y) = event_to_screen(&pointer_down_canvas, &event);
        pointer_down_source.touch_down(x, y);
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("pointerdown", pointer_down.as_ref().unchecked_ref())?;
    pointer_down.forget();

    let pointer_move_canvas = canvas.clone();
    let pointer_move_source = touch_source.clone();
    let pointer_move = Closure::wrap(Box::new(move |event: PointerEvent| {
        if event.buttons() & 1 == 0 {
            return;
        }
        event.prevent_default();
        let (x, y) = event_to_screen(&pointer_move_canvas, &event);
        pointer_move_source.touch_move(x, y);
    }) as Box<dyn FnMut(_)>);
    canvas
        .add_event_listener_with_callback("pointermove", pointer_move.as_ref().unchecked_ref())?;
    pointer_move.forget();

    let pointer_up_canvas = canvas.clone();
    let pointer_up_source = touch_source.clone();
    let pointer_up = Closure::wrap(Box::new(move |event: PointerEvent| {
        event.prevent_default();
        if let Err(error) = pointer_up_canvas.release_pointer_capture(event.pointer_id()) {
            web_sys::console::error_1(&error);
        }
        pointer_up_source.touch_up();
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback("pointerup", pointer_up.as_ref().unchecked_ref())?;
    pointer_up.forget();

    let pointer_cancel_source = touch_source;
    let pointer_cancel = Closure::wrap(Box::new(move |event: PointerEvent| {
        event.prevent_default();
        pointer_cancel_source.touch_up();
    }) as Box<dyn FnMut(_)>);
    canvas.add_event_listener_with_callback(
        "pointercancel",
        pointer_cancel.as_ref().unchecked_ref(),
    )?;
    pointer_cancel.forget();

    Ok(())
}

fn event_to_screen(canvas: &HtmlCanvasElement, event: &PointerEvent) -> (f32, f32) {
    let bounds = canvas.get_bounding_client_rect();
    let x =
        (f64::from(event.client_x()) - bounds.left()) * f64::from(canvas.width()) / bounds.width();
    let y =
        (f64::from(event.client_y()) - bounds.top()) * f64::from(canvas.height()) / bounds.height();
    (x as f32, y as f32)
}

// ---- Three.js viewer exports ----

const VIEWER_LINKAGE: LinkageFixed<6, 1, 25> =
    linkage_fixed!("../../linkage-blaze-armatron-core/src/armatron1.lb.rs");

#[wasm_bindgen]
pub fn dof() -> usize {
    VIEWER_LINKAGE.dof()
}

#[wasm_bindgen]
pub fn param_name(index: usize) -> String {
    VIEWER_LINKAGE.view().param(index).name().to_owned()
}

#[wasm_bindgen]
pub fn param_default(index: usize) -> f32 {
    VIEWER_LINKAGE.view().param(index).default()
}

#[wasm_bindgen]
pub fn len() -> usize {
    VIEWER_LINKAGE.view().len()
}

#[wasm_bindgen]
pub fn linkage_points(params: Vec<f32>) -> Vec<f32> {
    let params = params
        .as_slice()
        .try_into()
        .expect("expected linkage param count");
    VIEWER_LINKAGE
        .view()
        .poses(params)
        .map(Pose::position)
        .flat_map(Vec3::into_array)
        .collect()
}
