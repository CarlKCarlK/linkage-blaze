#![forbid(unsafe_code)]

use core::convert::Infallible;

use embassy_time::Instant;
use embedded_graphics::{
    Pixel,
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Drawable, OriginDimensions, Size},
};
use linkage_blaze_armatron_core::{CydSim as CoreCydSim, FrameBuffer, TickOut};
use embedded_graphics::prelude::WebColors;
use linkage_blaze_core::{Linkage, Pose, Rgb888, Vec3};
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
        Self::new_from_core_sim(CoreCydSim::new()) // or new_with_fps() for testing fps in wasm
    }

    #[wasm_bindgen(js_name = newWithFps)]
    pub fn new_with_fps() -> Self {
        Self::new_from_core_sim(CoreCydSim::new_with_fps())
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
        use linkage_blaze_armatron_core::TouchInputEvent;
        let _ = self
            .sim
            .handle_touch_input_event(TouchInputEvent::Down { x, y });
        self.draw_frame();
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        use linkage_blaze_armatron_core::TouchInputEvent;
        let _ = self
            .sim
            .handle_touch_input_event(TouchInputEvent::Move { x, y });
        self.draw_frame();
    }

    pub fn touch_up(&mut self) {
        use linkage_blaze_armatron_core::TouchInputEvent;
        let _ = self.sim.handle_touch_input_event(TouchInputEvent::Up);
        self.draw_frame();
    }

    #[wasm_bindgen(js_name = paramCount)]
    pub fn param_count() -> usize {
        CoreCydSim::param_count()
    }

    #[wasm_bindgen(js_name = paramName)]
    pub fn param_name(index: usize) -> String {
        CoreCydSim::param_name(index).to_string()
    }

    #[wasm_bindgen(js_name = paramDefault)]
    pub fn param_default(index: usize) -> f32 {
        CoreCydSim::param_default(index)
    }

    #[wasm_bindgen(js_name = getParam)]
    pub fn get_param(&self, index: usize) -> f32 {
        self.sim.get_param(index)
    }

    #[wasm_bindgen(js_name = setParam)]
    pub fn set_param(&mut self, index: usize, value: f32) {
        self.sim.set_param_by_index(index, value);
    }

    #[wasm_bindgen(js_name = drawViewOnly)]
    pub fn draw_view_only(&mut self) {
        self.sim.draw_view_only(&mut self.display).ok();
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

    pub fn tick_at(&mut self, now_micros: f64) -> bool {
        match self.sim.tick(Instant::from_micros(now_micros as u64), None) {
            TickOut::Draw => {
                self.draw_frame();
                true
            }
            TickOut::Calibrate | TickOut::Nada => false,
        }
    }
}

impl CydSim {
    fn new_from_core_sim(sim: CoreCydSim) -> Self {
        let mut display = WasmDisplay::new();
        sim.draw(&mut display).ok();
        Self { sim, display }
    }

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
        let mut bytes = Vec::with_capacity(self.frame_buffer.raw_pixels().len() * 4);
        for pixel in self.frame_buffer.raw_pixels() {
            bytes.push(scale_rgb565_channel(((pixel >> 11) & 0x1f) as u8, 31));
            bytes.push(scale_rgb565_channel(((pixel >> 5) & 0x3f) as u8, 63));
            bytes.push(scale_rgb565_channel((pixel & 0x1f) as u8, 31));
            bytes.push(255);
        }
        bytes
    }
}

impl DrawTarget for WasmDisplay {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.frame_buffer.clear(color);
        Ok(())
    }

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

// ---- Three.js viewer exports ----

const VIEWER_LINKAGE: Linkage<6, 21> = include!("../../linkage-blaze-armatron-core/src/armatron1.lb.rs");

#[wasm_bindgen]
pub fn dof() -> usize {
    VIEWER_LINKAGE.dof()
}

#[wasm_bindgen]
pub fn param_name(index: usize) -> String {
    VIEWER_LINKAGE.param_name(index).to_owned()
}

#[wasm_bindgen]
pub fn param_default(index: usize) -> f32 {
    VIEWER_LINKAGE.param_default(index)
}

#[wasm_bindgen]
pub fn len() -> usize {
    VIEWER_LINKAGE.len()
}

#[wasm_bindgen]
pub fn linkage_points(params: Vec<f32>) -> Vec<f32> {
    let params = params
        .as_slice()
        .try_into()
        .expect("expected linkage param count");
    VIEWER_LINKAGE
        .poses(params)
        .map(Pose::position)
        .flat_map(Vec3::into_array)
        .collect()
}
