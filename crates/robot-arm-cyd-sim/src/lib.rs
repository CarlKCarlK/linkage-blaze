#![forbid(unsafe_code)]

use embedded_graphics::pixelcolor::RgbColor;
use robot_arm_core::cyd::CydSim as CoreCydSim;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub struct CydSim {
    sim: CoreCydSim,
}

#[wasm_bindgen]
impl CydSim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            sim: CoreCydSim::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.sim.width()
    }

    pub fn height(&self) -> usize {
        self.sim.height()
    }

    pub fn rgba(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.sim.pixels().len() * 4);
        for pixel in self.sim.pixels() {
            bytes.push(scale_rgb565_channel(pixel.r(), 31));
            bytes.push(scale_rgb565_channel(pixel.g(), 63));
            bytes.push(scale_rgb565_channel(pixel.b(), 31));
            bytes.push(255);
        }
        bytes
    }

    pub fn touch_down(&mut self, x: f32, y: f32) {
        self.sim.touch_down(x, y);
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        self.sim.touch_move(x, y);
    }

    pub fn touch_up(&mut self) {
        self.sim.touch_up();
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
