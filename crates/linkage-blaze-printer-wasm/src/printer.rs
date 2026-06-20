extern crate alloc;
use alloc::vec::Vec;

use crate::gcode::parse_gcode;
use crate::geometry::BoundingBox;

#[derive(Debug, Clone)]
pub struct Segment {
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub extruding: bool,
    pub layer: u32,
}

pub struct PrinterSim {
    pub segments: Vec<Segment>,
    pub current_index: usize,
}

impl PrinterSim {
    pub fn new(gcode: &str) -> Self {
        Self {
            segments: parse_gcode(gcode),
            current_index: 0,
        }
    }

    pub fn reset(&mut self) {
        self.current_index = 0;
    }

    pub fn advance(&mut self, count: usize) {
        self.current_index = (self.current_index + count).min(self.segments.len());
    }

    pub fn is_done(&self) -> bool {
        self.current_index >= self.segments.len()
    }

    pub fn toolhead_position(&self) -> (f32, f32, f32) {
        if self.current_index == 0 || self.segments.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let seg = &self.segments[self.current_index - 1];
        (seg.x1, seg.y1, seg.z1)
    }

    pub fn current_layer(&self) -> u32 {
        if self.current_index == 0 || self.segments.is_empty() {
            return 0;
        }
        self.segments[self.current_index - 1].layer
    }

    pub fn progress(&self) -> f32 {
        if self.segments.is_empty() {
            return 0.0;
        }
        self.current_index as f32 / self.segments.len() as f32
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn extrusion_segments_flat(&self) -> Vec<f32> {
        let mut flat = Vec::new();
        for seg in self.segments[..self.current_index].iter().filter(|seg| seg.extruding) {
            flat.extend_from_slice(&[seg.x0, seg.y0, seg.z0, seg.x1, seg.y1, seg.z1]);
        }
        flat
    }

    pub fn travel_segments_flat(&self) -> Vec<f32> {
        let mut flat = Vec::new();
        for seg in self.segments[..self.current_index].iter().filter(|seg| !seg.extruding) {
            flat.extend_from_slice(&[seg.x0, seg.y0, seg.z0, seg.x1, seg.y1, seg.z1]);
        }
        flat
    }

    pub fn bounding_box(&self) -> BoundingBox {
        let mut bbox = BoundingBox::empty();
        for seg in &self.segments {
            bbox.extend(seg.x0, seg.y0, seg.z0);
            bbox.extend(seg.x1, seg.y1, seg.z1);
        }
        bbox
    }
}
