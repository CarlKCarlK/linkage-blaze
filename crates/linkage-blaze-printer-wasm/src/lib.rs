#![forbid(unsafe_code)]

extern crate alloc;

mod gcode;
mod geometry;
mod linkage;
mod printer;

use linkage::{draw_items_from, printer_points_from};
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
        Self {
            sim: PrinterSim::new(""),
        }
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

    /// Current playback index (number of segments that have been played).
    #[wasm_bindgen(js_name = currentIndex)]
    pub fn current_index(&self) -> usize {
        self.sim.current_index
    }

    /// Current number of draw items produced by the Rust-owned print linkage.
    #[wasm_bindgen(js_name = printDrawItemCount)]
    pub fn print_draw_item_count(&self) -> usize {
        self.sim.print_draw_item_count()
    }

    /// Current number of steps in the Rust-owned print linkage.
    #[wasm_bindgen(js_name = printLinkageStepCount)]
    pub fn print_linkage_step_count(&self) -> usize {
        self.sim.print_linkage_step_count()
    }

    /// Flat draw-item records produced by the print linkage since `from_item`.
    #[wasm_bindgen(js_name = printDrawItemsSince)]
    pub fn print_draw_items_since(&self, from_item: usize) -> Vec<f32> {
        self.sim.print_draw_items_flat_since(from_item)
    }

    /// Flat `[x0,y0,z0, x1,y1,z1, ...]` for ALL extrusion segments played so far.
    #[wasm_bindgen(js_name = extrusionSegments)]
    pub fn extrusion_segments(&self) -> Vec<f32> {
        self.sim.extrusion_segments_flat()
    }

    /// Flat `[x0,y0,z0, x1,y1,z1, ...]` for extrusion segments in `[from_seg, current_index)`.
    ///
    /// Use this for incremental updates: pass the last `currentIndex()` value to get only new data.
    #[wasm_bindgen(js_name = extrusionSegmentsSince)]
    pub fn extrusion_segments_since(&self, from_seg: usize) -> Vec<f32> {
        self.sim.extrusion_segments_flat_since(from_seg)
    }

    /// Flat `[x0,y0,z0, x1,y1,z1, ...]` for ALL travel segments played so far.
    #[wasm_bindgen(js_name = travelSegments)]
    pub fn travel_segments(&self) -> Vec<f32> {
        self.sim.travel_segments_flat()
    }

    /// Flat `[x0,y0,z0, x1,y1,z1, ...]` for travel segments in `[from_seg, current_index)`.
    #[wasm_bindgen(js_name = travelSegmentsSince)]
    pub fn travel_segments_since(&self, from_seg: usize) -> Vec<f32> {
        self.sim.travel_segments_flat_since(from_seg)
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

/// Draw items for the Cartesian printer as a flat array (12 floats per item).
///
/// Each record: `[type, x0,y0,z0, x1,y1,z1, r,g,b, size1, size2]`
/// - type 0 = Stroke (x0..z0 = start, x1..z1 = end, size1 = width)
/// - type 1 = Sphere (x0..z0 = center, size1 = radius)
/// - type 2 = Disk   (x0..z0 = center, size1 = radius)
#[wasm_bindgen(js_name = printerDrawItems)]
pub fn printer_draw_items(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    draw_items_from(x_mm, y_mm, z_mm)
}

/// Kinematic poses for the bed-slinger printer as a flat `[x,y,z, ...]` array.
///
/// X and Z move the nozzle; Y moves the bed under the fixed-Y nozzle.
#[wasm_bindgen(js_name = printerPoints)]
pub fn printer_points(x_mm: f32, y_mm: f32, z_mm: f32) -> Vec<f32> {
    printer_points_from(x_mm, y_mm, z_mm)
}
