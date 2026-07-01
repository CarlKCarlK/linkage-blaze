extern crate alloc;
use alloc::vec::Vec;
use core::mem;

use crate::gcode::parse_gcode;
use crate::geometry::BoundingBox;
use embedded_graphics_core::pixelcolor::RgbColor;
use linkage_blaze_core::{Linkage, LinkageBuf, Rgb888};

const EXTRUSION_COLOR: Rgb888 = Rgb888::new(21, 96, 130);
const TRAVEL_COLOR: Rgb888 = Rgb888::new(173, 181, 189);
const EXTRUSION_WIDTH: f32 = 0.8;
const TRAVEL_WIDTH: f32 = 0.35;
const DRAW_ITEM_STRIDE: usize = 12;

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
    print_linkage: LinkageBuf<0, 0>,
    print_position: (f32, f32, f32),
    print_draw_items_3d_flat: Vec<f32>,
}

impl PrinterSim {
    pub fn new(gcode: &str) -> Self {
        Self {
            segments: parse_gcode(gcode),
            current_index: 0,
            print_linkage: LinkageBuf::start(),
            print_position: (0.0, 0.0, 0.0),
            print_draw_items_3d_flat: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.current_index = 0;
        self.print_linkage = LinkageBuf::start();
        self.print_position = (0.0, 0.0, 0.0);
        self.print_draw_items_3d_flat.clear();
    }

    pub fn advance(&mut self, count: usize) {
        let next_index = (self.current_index + count).min(self.segments.len());
        while self.current_index < next_index {
            let segment = self.segments[self.current_index].clone();
            self.append_segment_to_print(&segment);
            self.current_index += 1;
        }
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

    pub fn print_draw_item_3d_count(&self) -> usize {
        self.print_draw_items_3d_flat.len() / DRAW_ITEM_STRIDE
    }

    pub fn print_linkage_step_count(&self) -> usize {
        self.print_linkage.len()
    }

    pub fn print_draw_items_3d_flat_since(&self, from_item: usize) -> Vec<f32> {
        let start = (from_item * DRAW_ITEM_STRIDE).min(self.print_draw_items_3d_flat.len());
        self.print_draw_items_3d_flat[start..].to_vec()
    }

    pub fn extrusion_segments_flat(&self) -> Vec<f32> {
        self.extrusion_segments_flat_since(0)
    }

    pub fn extrusion_segments_flat_since(&self, from_seg: usize) -> Vec<f32> {
        let start = from_seg.min(self.current_index);
        self.segments[start..self.current_index]
            .iter()
            .filter(|s| s.extruding)
            .flat_map(|s| [s.x0, s.y0, s.z0, s.x1, s.y1, s.z1])
            .collect()
    }

    pub fn travel_segments_flat(&self) -> Vec<f32> {
        self.travel_segments_flat_since(0)
    }

    pub fn travel_segments_flat_since(&self, from_seg: usize) -> Vec<f32> {
        let start = from_seg.min(self.current_index);
        self.segments[start..self.current_index]
            .iter()
            .filter(|s| !s.extruding)
            .flat_map(|s| [s.x0, s.y0, s.z0, s.x1, s.y1, s.z1])
            .collect()
    }

    pub fn bounding_box(&self) -> BoundingBox {
        let mut bbox = BoundingBox::empty();
        for seg in &self.segments {
            bbox.extend(seg.x0, seg.y0, seg.z0);
            bbox.extend(seg.x1, seg.y1, seg.z1);
        }
        bbox
    }

    fn append_segment_to_print(&mut self, segment: &Segment) {
        let color = if segment.extruding {
            EXTRUSION_COLOR
        } else {
            TRAVEL_COLOR
        };
        let width = if segment.extruding {
            EXTRUSION_WIDTH
        } else {
            TRAVEL_WIDTH
        };
        let (position_x, position_y, position_z) = self.print_position;
        let linkage = mem::replace(&mut self.print_linkage, LinkageBuf::start())
            .pen_up()
            .forward(segment.x0 - position_x)
            .left(segment.y0 - position_y)
            .up(segment.z0 - position_z)
            .pen_color(color)
            .pen_width(width)
            .pen_down()
            .forward(segment.x1 - segment.x0)
            .left(segment.y1 - segment.y0)
            .up(segment.z1 - segment.z0)
            .pen_up();
        self.print_linkage = linkage;
        self.print_position = (segment.x1, segment.y1, segment.z1);
        self.push_print_draw_item_3d(segment, color, width);
    }

    fn push_print_draw_item_3d(&mut self, segment: &Segment, color: Rgb888, width: f32) {
        self.print_draw_items_3d_flat.extend_from_slice(&[
            0.0,
            segment.x0,
            segment.y0,
            segment.z0,
            segment.x1,
            segment.y1,
            segment.z1,
            color.r() as f32,
            color.g() as f32,
            color.b() as f32,
            width,
            0.0,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_grows_print_linkage_incrementally() {
        let mut printer_sim = PrinterSim::new(
            "\
G90
G0 X0 Y0 Z0.2
G1 X10 Y0 E1.0
G1 X10 Y10 E2.0
",
        );

        assert_eq!(printer_sim.print_draw_item_3d_count(), 0);
        let initial_step_count = printer_sim.print_linkage_step_count();

        printer_sim.advance(1);
        assert_eq!(printer_sim.print_draw_item_3d_count(), 1);
        assert!(printer_sim.print_linkage_step_count() > initial_step_count);

        let first_batch = printer_sim.print_draw_items_3d_flat_since(0);
        assert_eq!(first_batch.len(), DRAW_ITEM_STRIDE);

        printer_sim.advance(2);
        assert_eq!(printer_sim.print_draw_item_3d_count(), 3);
        let second_batch = printer_sim.print_draw_items_3d_flat_since(1);
        assert_eq!(second_batch.len(), DRAW_ITEM_STRIDE * 2);
    }

    #[test]
    fn reset_clears_print_linkage() {
        let mut printer_sim = PrinterSim::new("G1 X10 Y0 E1.0\n");
        printer_sim.advance(1);
        assert!(printer_sim.print_draw_item_3d_count() > 0);
        assert!(printer_sim.print_linkage_step_count() > LinkageBuf::<0>::start().len());

        printer_sim.reset();

        assert_eq!(printer_sim.print_draw_item_3d_count(), 0);
        assert_eq!(
            printer_sim.print_linkage_step_count(),
            LinkageBuf::<0>::start().len()
        );
    }
}
