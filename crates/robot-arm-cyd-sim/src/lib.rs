#![forbid(unsafe_code)]

use embedded_graphics::prelude::*;
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
        let mut bytes = Vec::with_capacity(self.frame_buffer.pixels().len() * 4);
        for pixel in self.frame_buffer.pixels() {
            bytes.push(scale_rgb565_channel(pixel.r(), 31));
            bytes.push(scale_rgb565_channel(pixel.g(), 63));
            bytes.push(scale_rgb565_channel(pixel.b(), 31));
            bytes.push(255);
        }
        bytes
    }

    pub fn touch_down(&mut self, x: f32, y: f32) {
        use robot_arm_core::cyd::TouchInputEvent;
        let _ = self
            .sim
            .handle_touch_input_event(TouchInputEvent::Down { x, y });
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        use robot_arm_core::cyd::TouchInputEvent;
        let _ = self
            .sim
            .handle_touch_input_event(TouchInputEvent::Move { x, y });
        self.sim.render_to(&mut self.frame_buffer);
    }

    pub fn touch_up(&mut self) {
        use robot_arm_core::cyd::TouchInputEvent;
        let _ = self.sim.handle_touch_input_event(TouchInputEvent::Up);
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
