#![forbid(unsafe_code)]

use core::convert::Infallible;

use embassy_time::Instant;
use embedded_graphics::{
    Pixel,
    pixelcolor::{Rgb565, RgbColor},
    prelude::{DrawTarget, Drawable, OriginDimensions, Size},
};
use robot_arm_core::cyd::{CydSim as CoreCydSim, FrameBuffer};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub struct CydSim {
    sim: CoreCydSim,
    display: WasmDisplay,
}

struct WasmDisplay {
    frame_buffer: FrameBuffer,
}

#[wasm_bindgen]
impl CydSim {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let sim = CoreCydSim::new();
        let mut display = WasmDisplay::new();
        sim.draw(&mut display).ok();
        Self { sim, display }
    }

    pub fn width(&self) -> usize {
        self.sim.width()
    }

    pub fn height(&self) -> usize {
        self.sim.height()
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.display.rgba()
    }

    pub fn touch_down(&mut self, x: f32, y: f32) {
        use robot_arm_core::cyd::TouchInputEvent;
        let _ = self
            .sim
            .handle_touch_input_event(TouchInputEvent::Down { x, y });
        self.draw_frame();
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        use robot_arm_core::cyd::TouchInputEvent;
        let _ = self
            .sim
            .handle_touch_input_event(TouchInputEvent::Move { x, y });
        self.draw_frame();
    }

    pub fn touch_up(&mut self) {
        use robot_arm_core::cyd::TouchInputEvent;
        let _ = self.sim.handle_touch_input_event(TouchInputEvent::Up);
        self.draw_frame();
    }

    pub fn reverse_kinematics(&mut self) -> f32 {
        let distance = self.sim.reverse_kinematics();
        self.draw_frame();
        distance
    }

    pub fn start_reverse_kinematics(&mut self) {
        self.sim.start_reverse_kinematics();
        self.draw_frame();
    }

    pub fn stop_reverse_kinematics(&mut self) {
        self.sim.stop_reverse_kinematics();
        self.draw_frame();
    }

    pub fn is_reverse_kinematics_running(&self) -> bool {
        self.sim.is_reverse_kinematics_running()
    }

    pub fn tick_reverse_kinematics_at(&mut self, now_micros: f64) -> bool {
        let running = self
            .sim
            .tick_reverse_kinematics_at(Instant::from_micros(now_micros as u64));
        self.draw_frame();
        running
    }
}

impl CydSim {
    fn draw_frame(&mut self) {
        self.sim.draw(&mut self.display).ok();
    }
}

impl WasmDisplay {
    fn new() -> Self {
        Self {
            frame_buffer: FrameBuffer::new(),
        }
    }

    fn rgba(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.frame_buffer.pixels().len() * 4);
        for pixel in self.frame_buffer.pixels() {
            bytes.push(scale_rgb565_channel(pixel.r(), 31));
            bytes.push(scale_rgb565_channel(pixel.g(), 63));
            bytes.push(scale_rgb565_channel(pixel.b(), 31));
            bytes.push(255);
        }
        bytes
    }
}

impl DrawTarget for WasmDisplay {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.frame_buffer.draw_iter(pixels)
    }
}

impl OriginDimensions for WasmDisplay {
    fn size(&self) -> Size {
        self.frame_buffer.size()
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
