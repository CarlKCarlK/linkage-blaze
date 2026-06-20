#![forbid(unsafe_code)]

extern crate alloc;

mod gcode;
mod geometry;
mod linkage;
mod printer;

use linkage::toolhead_points;
use printer::PrinterSim;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub struct PrinterSimWasm {
    sim: PrinterSim,
}

#[wasm_bindgen]
impl PrinterSimWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { sim: PrinterSim::new("") }
    }

    /// Replace the current G-code and reset playback to the start.
    pub fn load(&mut self, gcode: &str) {
        self.sim = PrinterSim::new(gcode);
    }

    /// Reset playback to the beginning without re-parsing.
    pub fn reset(&mut self) {
        self.sim.reset();
    }

    /// Advance playback by `count` segments.
    pub fn advance(&mut self, count: usize) {
        self.sim.advance(count);
    }

    #[wasm_bindgen(js_name = isDone)]
    pub fn is_done(&self) -> bool {
        self.sim.is_done()
    }

    #[wasm_bindgen(js_name = currentLayer)]
    pub fn current_layer(&self) -> u32 {
        self.sim.current_layer()
    }

    /// Playback progress in the range 0.0–1.0.
    pub fn progress(&self) -> f32 {
        self.sim.progress()
    }

    #[wasm_bindgen(js_name = segmentCount)]
    pub fn segment_count(&self) -> usize {
        self.sim.segment_count()
    }

    /// Current toolhead position as `[x, y, z]` in millimetres.
    #[wasm_bindgen(js_name = toolheadPosition)]
    pub fn toolhead_position(&self) -> Vec<f32> {
        let (x, y, z) = self.sim.toolhead_position();
        alloc::vec![x, y, z]
    }

    /// Flat `[x0,y0,z0, x1,y1,z1, ...]` array for all extrusion segments played so far.
    #[wasm_bindgen(js_name = extrusionSegments)]
    pub fn extrusion_segments(&self) -> Vec<f32> {
        self.sim.extrusion_segments_flat()
    }

    /// Flat `[x0,y0,z0, x1,y1,z1, ...]` array for all travel segments played so far.
    #[wasm_bindgen(js_name = travelSegments)]
    pub fn travel_segments(&self) -> Vec<f32> {
        self.sim.travel_segments_flat()
    }

    /// Bounding box of the entire G-code path as `[min_x, min_y, min_z, max_x, max_y, max_z]`.
    #[wasm_bindgen(js_name = boundingBox)]
    pub fn bounding_box(&self) -> Vec<f32> {
        alloc::vec::Vec::from(self.sim.bounding_box().to_flat_array())
    }
}

impl Default for PrinterSimWasm {
    fn default() -> Self {
        Self::new()
    }
}

/// Flat `[x,y,z, ...]` pose points for the printer toolhead at the given G-code position.
///
/// Useful for overlaying the kinematic chain on the Three.js scene.
#[wasm_bindgen(js_name = printerToolheadPoints)]
pub fn printer_toolhead_points(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    toolhead_points(x_mm, y_mm, z_mm)
}
