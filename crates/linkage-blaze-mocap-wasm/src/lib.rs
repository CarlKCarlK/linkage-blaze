#![forbid(unsafe_code)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use embedded_graphics_core::pixelcolor::RgbColor;
use linkage_blaze_core::{DrawItem, LinkageBuf};
use linkage_blaze_mocap::{
    BvhFrame, BvhParameterLayout, build_bvh_linkage_buf, bvh_frame_params, discover_bvh_parameters,
    parse_bvh,
};
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

const DOF: usize = 256;
const MARKS: usize = 64;
const STRIDE: usize = 12;

#[wasm_bindgen]
pub struct MocapClipWasm {
    linkage: LinkageBuf<DOF, MARKS>,
    layout: BvhParameterLayout,
    frames: Vec<BvhFrame>,
}

#[wasm_bindgen]
impl MocapClipWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(bvh_source: &str) -> Result<Self, JsValue> {
        let clip = parse_bvh(bvh_source).map_err(to_js_error)?;
        let layout = discover_bvh_parameters(&clip).map_err(to_js_error)?;
        let linkage = build_bvh_linkage_buf::<DOF, MARKS>(&clip, &layout).map_err(to_js_error)?;

        Ok(Self {
            linkage,
            layout,
            frames: clip.frames,
        })
    }

    #[wasm_bindgen(js_name = frameCount)]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    #[wasm_bindgen(js_name = parameterCount)]
    pub fn parameter_count(&self) -> usize {
        self.layout.len()
    }

    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(&self, frame_index: usize) -> Result<Vec<f32>, JsValue> {
        let params = self.params_for_frame(frame_index)?;

        Ok(self
            .linkage
            .view()
            .draw_items(&params)
            .flat_map(flatten_draw_item)
            .collect())
    }

    #[wasm_bindgen(js_name = frameParams)]
    pub fn frame_params(&self, frame_index: usize) -> Result<Vec<f32>, JsValue> {
        let params = self.params_for_frame(frame_index)?;

        Ok(Vec::from(params))
    }
}

impl MocapClipWasm {
    fn params_for_frame(&self, frame_index: usize) -> Result<[f32; DOF], JsValue> {
        let frame = self
            .frames
            .get(frame_index)
            .ok_or_else(|| JsValue::from_str("frame index out of range"))?;
        bvh_frame_params::<DOF>(&self.layout, frame).map_err(to_js_error)
    }
}

fn flatten_draw_item(draw_item: DrawItem) -> [f32; STRIDE] {
    let mut record = [0.0; STRIDE];

    match draw_item {
        DrawItem::Stroke(stroke) => {
            record[0] = 0.0;
            let [x, y, z] = stroke.start().position().into_array();
            record[1] = x;
            record[2] = y;
            record[3] = z;
            let [x, y, z] = stroke.end().position().into_array();
            record[4] = x;
            record[5] = y;
            record[6] = z;
            let color = stroke.color();
            record[7] = color.r() as f32;
            record[8] = color.g() as f32;
            record[9] = color.b() as f32;
            record[10] = stroke.width();
        }
        DrawItem::Sphere(sphere) => {
            record[0] = 1.0;
            let [x, y, z] = sphere.pose().position().into_array();
            record[1] = x;
            record[2] = y;
            record[3] = z;
            let color = sphere.color();
            record[7] = color.r() as f32;
            record[8] = color.g() as f32;
            record[9] = color.b() as f32;
            record[10] = sphere.radius();
        }
        DrawItem::Disk(disk) => {
            record[0] = 2.0;
            let [x, y, z] = disk.pose().position().into_array();
            record[1] = x;
            record[2] = y;
            record[3] = z;
            let color = disk.color();
            record[7] = color.r() as f32;
            record[8] = color.g() as f32;
            record[9] = color.b() as f32;
            record[10] = disk.radius();
        }
        DrawItem::Ring(ring) => {
            record[0] = 3.0;
            let [x, y, z] = ring.pose().position().into_array();
            record[1] = x;
            record[2] = y;
            record[3] = z;
            let color = ring.color();
            record[7] = color.r() as f32;
            record[8] = color.g() as f32;
            record[9] = color.b() as f32;
            record[10] = ring.radius();
            record[11] = ring.width();
        }
    }

    record
}

fn to_js_error(error: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&String::from(error.to_string()))
}
