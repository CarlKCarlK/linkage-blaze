#![forbid(unsafe_code)]

use embedded_graphics::pixelcolor::RgbColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, StyledDrawable};
use robot_arm_core::cyd::{CydSim as CoreCydSim, FrameBuffer};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub struct CydSim {
    sim: CoreCydSim,
    frame_buffer: FrameBuffer,
}

#[wasm_bindgen]
impl CydSim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let sim = CoreCydSim::new();
        let mut frame_buffer = FrameBuffer::new();
        sim.render_to(&mut frame_buffer);

        Self { sim, frame_buffer }
    }

    pub fn width(&self) -> usize {
        self.sim.width()
    }

    pub fn height(&self) -> usize {
        self.sim.height()
    }

    pub fn rgba(&self) -> Vec<u8> {
        // Create a mutable copy of the frame buffer to draw the cursor on
        let mut buffer = FrameBuffer::new();
        buffer
            .pixels_mut()
            .copy_from_slice(self.frame_buffer.pixels());

        // Draw cursor overlay if present
        if let Some((cursor_x, cursor_y)) = self.sim.touch_cursor() {
            draw_touch_cursor(&mut buffer, cursor_x as usize, cursor_y as usize);
        }

        let mut bytes = Vec::with_capacity(buffer.pixels().len() * 4);
        for pixel in buffer.pixels() {
            bytes.push(scale_rgb565_channel(pixel.r(), 31));
            bytes.push(scale_rgb565_channel(pixel.g(), 63));
            bytes.push(scale_rgb565_channel(pixel.b(), 31));
            bytes.push(255);
        }
        bytes
    }

    pub fn touch_down(&mut self, x: f32, y: f32) {
        self.sim.touch_down(x, y);
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        self.sim.touch_move(x, y);
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn touch_up(&mut self) {
        self.sim.touch_up();
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn reverse_kinematics(&mut self) -> f32 {
        let distance = self.sim.reverse_kinematics();
        self.sim.render_to(&mut self.frame_buffer);
        distance
    }

    pub fn start_reverse_kinematics(&mut self) {
        self.sim.start_reverse_kinematics();
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn stop_reverse_kinematics(&mut self) {
        self.sim.stop_reverse_kinematics();
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn is_reverse_kinematics_running(&self) -> bool {
        self.sim.is_reverse_kinematics_running()
    }

    pub fn set_frame_dt_seconds(&mut self, dt_seconds: f32) {
        self.sim.set_frame_dt_seconds(dt_seconds);
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn tick_reverse_kinematics(&mut self, dt_seconds: f32) -> bool {
        let running = self.sim.tick_reverse_kinematics(dt_seconds);
        self.sim.render_to(&mut self.frame_buffer);
        running
    }
}

impl Default for CydSim {
    fn default() -> Self {
        Self::new()
    }
}

fn scale_rgb565_channel(value: u8, max: u8) -> u8 {
    ((u16::from(value) * 255) / u16::from(max)) as u8
}

fn draw_touch_cursor(buffer: &mut FrameBuffer, x: usize, y: usize) {
    let x = x as i32;
    let y = y as i32;
    let radius = 8;
    let white_style = PrimitiveStyle::with_stroke(RgbColor::WHITE, 1);

    // Draw horizontal line
    let _ = Line::new(Point::new(x - radius, y), Point::new(x + radius, y))
        .draw_styled(&white_style, buffer);

    // Draw vertical line
    let _ = Line::new(Point::new(x, y - radius), Point::new(x, y + radius))
        .draw_styled(&white_style, buffer);
}
