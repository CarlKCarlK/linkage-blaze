//! A browser-simulated CYD device.
//!
//! [`CydWasm`] offers the device-agnostic
//! [`Cyd`](linkage_blaze_cyd_core::Cyd) display/touch parts against an HTML
//! canvas, so the same generic example code that drives the real esp32 `CydEsp`
//! also runs in a web page. Its [`CydFrameWasm::flush`] awaits the next browser animation
//! frame (see [`animation_frame`]), blits the frame to the canvas, then
//! resolves — turning a platform-neutral `loop { draw; flush().await?; }`
//! into smooth, repaint-paced animation without inverting the loop into a state
//! machine.

mod animation_frame;

use core::{
    cell::{Cell, RefCell},
    convert::Infallible,
};
use std::{collections::VecDeque, rc::Rc};

use device_envoy_core::flash_block::FlashBlock;
use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{MonoFont, MonoTextStyle},
    pixelcolor::{IntoStorage, Rgb565, Rgb888},
    prelude::{Dimensions, DrawTarget, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use linkage_blaze_core::{PixelTarget, RgbColor, rgb888_from_rgb565};
use linkage_blaze_cyd_core::{
    CalibrationConfig, CalibrationCorner, Cyd, CydDisplay, CydFrame, CydInfallibleError,
    CydRawTouch, CydTouch, Orientation, RawTouchEvent, TouchEvent, calibration_corner_center,
    distort_demo_screen_to_raw,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData};

pub use animation_frame::next_animation_frame;

/// A CYD display simulated on an HTML canvas.
pub struct CydWasm {
    context: CanvasRenderingContext2d,
    size: Size,
    background: Rgb888,
    foreground: Rgb888,
    background565: Rgb565,
    foreground565: Rgb565,
    font: &'static MonoFont<'static>,
    raw_touch_events: RawTouchEvents,
    interaction_state: Rc<Cell<InteractionState>>,
    calibration_config: Option<CalibrationConfig>,
}

pub struct CydDisplayWasmPart<'a> {
    context: &'a CanvasRenderingContext2d,
    size: Size,
    background: Rgb888,
    foreground: Rgb888,
    background565: Rgb565,
    foreground565: Rgb565,
    font: &'static MonoFont<'static>,
}

pub struct CydTouchWasmPart {
    raw_touch_events: RawTouchEvents,
    calibration_config: Option<CalibrationConfig>,
}

#[derive(Clone)]
pub struct CydTouchWasmSource {
    raw_touch_events: RawTouchEvents,
    interaction_state: Rc<Cell<InteractionState>>,
}

pub struct CydWasmCalibrationFlashBlock {
    bytes: Rc<RefCell<Option<Vec<u8>>>>,
}

type RawTouchEvents = Rc<RefCell<VecDeque<RawTouchEvent>>>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum InteractionState {
    Ready,
    PointerDown,
    WaitingForFreshPress,
}

impl CydWasm {
    /// Build a simulated CYD that presents onto `context`, sized for `orientation`.
    #[must_use]
    pub fn new(
        context: CanvasRenderingContext2d,
        orientation: Orientation,
        background: Rgb888,
        foreground: Rgb888,
        font: &'static MonoFont<'static>,
    ) -> Self {
        Self::new_with_touch_source(
            context,
            orientation,
            background,
            foreground,
            font,
            CydTouchWasmSource::new(),
        )
    }

    #[must_use]
    pub fn new_with_touch_source(
        context: CanvasRenderingContext2d,
        orientation: Orientation,
        background: Rgb888,
        foreground: Rgb888,
        font: &'static MonoFont<'static>,
        touch_source: CydTouchWasmSource,
    ) -> Self {
        Self {
            context,
            size: orientation.size(),
            background,
            foreground,
            background565: Rgb565::from(background),
            foreground565: Rgb565::from(foreground),
            font,
            raw_touch_events: touch_source.raw_touch_events,
            interaction_state: touch_source.interaction_state,
            calibration_config: None,
        }
    }

    #[must_use]
    pub fn touch_source(&self) -> CydTouchWasmSource {
        CydTouchWasmSource {
            raw_touch_events: self.raw_touch_events.clone(),
            interaction_state: self.interaction_state.clone(),
        }
    }

    pub fn set_calibration(&mut self, calibration_config: CalibrationConfig) {
        self.calibration_config = Some(calibration_config);
    }

    pub fn clear_calibration(&mut self) {
        self.calibration_config = None;
    }
}

impl CydTouchWasmSource {
    #[must_use]
    pub fn new() -> Self {
        Self {
            raw_touch_events: Rc::new(RefCell::new(VecDeque::new())),
            interaction_state: Rc::new(Cell::new(InteractionState::Ready)),
        }
    }

    pub fn touch_down(&self, x: f32, y: f32) {
        match self.interaction_state.get() {
            InteractionState::WaitingForFreshPress => return,
            InteractionState::Ready | InteractionState::PointerDown => {
                self.interaction_state.set(InteractionState::PointerDown);
            }
        }
        let raw_point = distort_demo_screen_to_raw(x, y);
        self.push(RawTouchEvent::Down {
            raw_x: raw_point.x,
            raw_y: raw_point.y,
        });
    }

    pub fn touch_move(&self, x: f32, y: f32) {
        if self.interaction_state.get() != InteractionState::PointerDown {
            return;
        }
        let raw_point = distort_demo_screen_to_raw(x, y);
        self.push(RawTouchEvent::Move {
            raw_x: raw_point.x,
            raw_y: raw_point.y,
        });
    }

    pub fn touch_up(&self) {
        let interaction_state = self.interaction_state.get();
        self.interaction_state.set(InteractionState::Ready);
        if interaction_state == InteractionState::WaitingForFreshPress {
            return;
        }
        self.push(RawTouchEvent::Up);
    }

    pub fn wait_for_fresh_press(&self) {
        self.raw_touch_events.borrow_mut().clear();
        self.interaction_state
            .set(InteractionState::WaitingForFreshPress);
    }

    fn push(&self, raw_touch_event: RawTouchEvent) {
        self.raw_touch_events.borrow_mut().push_back(raw_touch_event);
    }
}

impl Default for CydTouchWasmSource {
    fn default() -> Self {
        Self::new()
    }
}

impl CydWasmCalibrationFlashBlock {
    #[must_use]
    pub fn new_precalibrated() -> Self {
        let calibration_corners = [
            CalibrationCorner::UpperLeft,
            CalibrationCorner::UpperRight,
            CalibrationCorner::LowerRight,
            CalibrationCorner::LowerLeft,
        ];
        let mut raw_points = [distort_demo_screen_to_raw(0.0, 0.0); 4];

        for (point_index, calibration_corner) in calibration_corners.into_iter().enumerate() {
            let screen_point = calibration_corner_center(calibration_corner);
            raw_points[point_index] =
                distort_demo_screen_to_raw(screen_point.x as f32, screen_point.y as f32);
        }

        let calibration_config = CalibrationConfig::from_four_points(raw_points);
        let bytes = postcard::to_stdvec(&calibration_config)
            .expect("CalibrationConfig postcard serialization must succeed");

        Self {
            bytes: Rc::new(RefCell::new(Some(bytes))),
        }
    }
}

impl FlashBlock for CydWasmCalibrationFlashBlock {
    type Error = Infallible;

    fn load<T>(&mut self) -> Result<Option<T>, Self::Error>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let bytes_ref = self.bytes.borrow();
        let Some(bytes) = bytes_ref.as_ref() else {
            return Ok(None);
        };
        Ok(postcard::from_bytes(bytes).ok())
    }

    fn save<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let bytes =
            postcard::to_stdvec(value).expect("WASM in-memory flash serialization must succeed");
        *self.bytes.borrow_mut() = Some(bytes);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        *self.bytes.borrow_mut() = None;
        Ok(())
    }
}

impl Cyd for CydWasm {
    // Presenting to a canvas cannot fail, so the device-agnostic render loop
    // never has a real error to propagate.
    type Error = CydInfallibleError;
    type Display<'a> = CydDisplayWasmPart<'a>;
    type Touch<'a> = CydTouchWasmPart;

    fn parts(&mut self) -> (CydDisplayWasmPart<'_>, CydTouchWasmPart) {
        (
            CydDisplayWasmPart {
                context: &self.context,
                size: self.size,
                background: self.background,
                foreground: self.foreground,
                background565: self.background565,
                foreground565: self.foreground565,
                font: self.font,
            },
            CydTouchWasmPart {
                raw_touch_events: self.raw_touch_events.clone(),
                calibration_config: self.calibration_config,
            },
        )
    }
}

impl CydDisplay for CydDisplayWasmPart<'_> {
    type Error = CydInfallibleError;
    type Frame<'a>
        = CydFrameWasm<'a>
    where
        Self: 'a;

    fn screen_size(&self) -> Size {
        self.size
    }

    fn background(&self) -> Rgb888 {
        self.background
    }

    fn foreground(&self) -> Rgb888 {
        self.foreground
    }

    fn background_565(&self) -> Rgb565 {
        self.background565
    }

    fn foreground_565(&self) -> Rgb565 {
        self.foreground565
    }

    fn frame_mut(&mut self, region: Rectangle) -> CydFrameWasm<'_> {
        self.frame_mut_with_tile_top_left(region, Point::zero())
    }

    fn frame_mut_with_tile_top_left(
        &mut self,
        region: Rectangle,
        tile_top_left: Point,
    ) -> CydFrameWasm<'_> {
        let size = region.size;
        let pixel_count = size.width as usize * size.height as usize;
        // Every new frame starts cleared to the device background so callers
        // never have to clear it themselves.
        let pixels = vec![self.background565.into_storage(); pixel_count];
        CydFrameWasm {
            context: &self.context,
            pixels,
            region,
            tile_top_left,
            background565: self.background565,
            foreground565: self.foreground565,
            font: self.font,
        }
    }

    fn fill_rectangle(
        &mut self,
        rectangle: Rectangle,
        color: Rgb565,
    ) -> Result<(), CydInfallibleError> {
        let screen_rectangle = Rectangle::new(Point::zero(), self.size);
        let rectangle = rectangle.intersection(&screen_rectangle);
        if rectangle.size.width == 0 || rectangle.size.height == 0 {
            return Ok(());
        }

        let pixel_count = rectangle.size.width as usize * rectangle.size.height as usize;
        let mut bytes = Vec::with_capacity(pixel_count * 4);
        for _pixel_index in 0..pixel_count {
            push_rgb565_rgba(&mut bytes, color.into_storage());
        }

        put_image_data(self.context, rectangle, &bytes);
        Ok(())
    }

    fn fill_contiguous<I>(
        &mut self,
        rectangle: Rectangle,
        pixels: I,
    ) -> Result<(), CydInfallibleError>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        if rectangle.size.width == 0 || rectangle.size.height == 0 {
            return Ok(());
        }

        let mut bytes =
            Vec::with_capacity(rectangle.size.width as usize * rectangle.size.height as usize * 4);
        for pixel in pixels {
            push_rgb565_rgba(&mut bytes, pixel.into_storage());
        }

        put_image_data(self.context, rectangle, &bytes);
        Ok(())
    }
}

impl CydTouch for CydTouchWasmPart {
    type Error = CydInfallibleError;

    fn read(&mut self) -> Result<Option<TouchEvent>, CydInfallibleError> {
        let Some(calibration_config) = self.calibration_config else {
            return Ok(None);
        };
        Ok(self
            .raw_touch_events
            .borrow_mut()
            .pop_front()
            .map(|raw_touch_event| match raw_touch_event {
                RawTouchEvent::Down { raw_x, raw_y } => {
                    let (x, y) = calibration_config.map_raw_to_screen(raw_x, raw_y);
                    TouchEvent::Down { x, y }
                }
                RawTouchEvent::Move { raw_x, raw_y } => {
                    let (x, y) = calibration_config.map_raw_to_screen(raw_x, raw_y);
                    TouchEvent::Move { x, y }
                }
                RawTouchEvent::Up => TouchEvent::Up,
            }))
    }
}

impl CydRawTouch for CydWasm {
    type Error = CydInfallibleError;

    fn read_raw_touch_event(&mut self) -> Result<Option<RawTouchEvent>, CydInfallibleError> {
        Ok(self.raw_touch_events.borrow_mut().pop_front())
    }
}

fn put_image_data(context: &CanvasRenderingContext2d, rectangle: Rectangle, bytes: &[u8]) {
    let image_data = ImageData::new_with_u8_clamped_array_and_sh(
        Clamped(bytes),
        rectangle.size.width,
        rectangle.size.height,
    )
    .expect("ImageData dimensions match the rectangle");
    context
        .put_image_data(
            &image_data,
            f64::from(rectangle.top_left.x),
            f64::from(rectangle.top_left.y),
        )
        .expect("put_image_data with in-bounds coordinates cannot fail");
}

fn push_rgb565_rgba(bytes: &mut Vec<u8>, pixel: u16) {
    let color = rgb888_from_rgb565(pixel);
    bytes.push(color.r());
    bytes.push(color.g());
    bytes.push(color.b());
    bytes.push(255);
}

/// A single in-progress frame backed by an `Rgb565` pixel buffer.
pub struct CydFrameWasm<'a> {
    context: &'a CanvasRenderingContext2d,
    pixels: Vec<u16>,
    // Where this frame presents and how large it is: set from the `Rectangle`
    // passed to `frame_mut`, so `flush` needs no separate position argument.
    region: Rectangle,
    // Tile top-left in screen coordinates. Drawing coordinates are translated
    // by this point before reaching the local frame buffer.
    tile_top_left: Point,
    background565: Rgb565,
    foreground565: Rgb565,
    font: &'static MonoFont<'static>,
}

impl CydFrameWasm<'_> {
    fn width(&self) -> usize {
        self.region.size.width as usize
    }

    fn height(&self) -> usize {
        self.region.size.height as usize
    }

    fn local_x(&self, x: i32) -> Option<usize> {
        usize::try_from(x.checked_sub(self.tile_top_left.x)?).ok()
    }

    fn local_y(&self, y: i32) -> Option<usize> {
        usize::try_from(y.checked_sub(self.tile_top_left.y)?).ok()
    }

    pub fn clear(&mut self) -> &mut Self {
        self.fill(self.background565)
    }

    pub fn fill(&mut self, color: Rgb565) -> &mut Self {
        self.pixels.fill(color.into_storage());
        self
    }

    /// Convert the `Rgb565` buffer to RGBA8 and `putImageData` it at the frame's top-left.
    fn present(&self) {
        let mut bytes = Vec::with_capacity(self.pixels.len() * 4);
        for pixel in &self.pixels {
            let color = rgb888_from_rgb565(*pixel);
            bytes.push(color.r());
            bytes.push(color.g());
            bytes.push(color.b());
            bytes.push(255);
        }
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&bytes),
            self.region.size.width,
            self.region.size.height,
        )
        .expect("ImageData dimensions match the pixel buffer");
        self.context
            .put_image_data(
                &image_data,
                f64::from(self.region.top_left.x),
                f64::from(self.region.top_left.y),
            )
            .expect("put_image_data with in-bounds coordinates cannot fail");
    }
}

impl DrawTarget for CydFrameWasm<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill(color);
        Ok(())
    }

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            let Some(local_x) = self.local_x(point.x) else {
                continue;
            };
            let Some(local_y) = self.local_y(point.y) else {
                continue;
            };
            if local_x < CydFrameWasm::width(self) && local_y < CydFrameWasm::height(self) {
                let index = local_y * CydFrameWasm::width(self) + local_x;
                self.pixels[index] = color.into_storage();
            }
        }
        Ok(())
    }
}

impl Dimensions for CydFrameWasm<'_> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.tile_top_left, self.region.size)
    }
}

impl PixelTarget for CydFrameWasm<'_> {
    fn width(&self) -> usize {
        usize::try_from(self.tile_top_left.x)
            .expect("tile top-left x must be non-negative")
            .checked_add(CydFrameWasm::width(self))
            .expect("frame width must fit in usize")
    }

    fn height(&self) -> usize {
        usize::try_from(self.tile_top_left.y)
            .expect("tile top-left y must be non-negative")
            .checked_add(CydFrameWasm::height(self))
            .expect("frame height must fit in usize")
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        let Some(local_x) = self.local_x(x as i32) else {
            return;
        };
        let Some(local_y) = self.local_y(y as i32) else {
            return;
        };
        if local_x >= CydFrameWasm::width(self) || local_y >= CydFrameWasm::height(self) {
            return;
        }
        let stride = CydFrameWasm::width(self);
        self.pixels[local_y * stride + local_x] = Rgb565::from(color).into_storage();
    }

    /// The frame buffer already stores RGB565, so a decoded image pixel can be
    /// written verbatim with no RGB888 round-trip.
    fn put_pixel_565(&mut self, x: usize, y: usize, rgb565: u16) {
        let Some(local_x) = self.local_x(x as i32) else {
            return;
        };
        let Some(local_y) = self.local_y(y as i32) else {
            return;
        };
        if local_x >= CydFrameWasm::width(self) || local_y >= CydFrameWasm::height(self) {
            return;
        }
        let stride = CydFrameWasm::width(self);
        self.pixels[local_y * stride + local_x] = rgb565;
    }
}

impl CydFrame for CydFrameWasm<'_> {
    type Error = CydInfallibleError;

    fn tile_top_left(&self) -> Point {
        self.tile_top_left
    }

    fn region(&self) -> Rectangle {
        self.region
    }

    fn clear(&mut self) -> &mut Self {
        CydFrameWasm::clear(self)
    }

    fn fill(&mut self, color: Rgb565) -> &mut Self {
        CydFrameWasm::fill(self, color)
    }

    fn copy_from_565(&mut self, src: &[u16]) -> Result<(), linkage_blaze_cyd_core::CopySizeError> {
        if self.pixels.len() != src.len() {
            return Err(linkage_blaze_cyd_core::CopySizeError {
                src_len: src.len(),
                frame_len: self.pixels.len(),
            });
        }
        self.pixels.copy_from_slice(src);
        Ok(())
    }

    fn write_text(&mut self, text: &str) -> &mut Self {
        let style = MonoTextStyle::new(self.font, self.foreground565);
        Text::with_baseline(text, Point::zero(), style, Baseline::Top)
            .draw(self)
            .expect("drawing onto an Infallible frame cannot fail");
        self
    }

    async fn flush(&mut self) -> Result<(), CydInfallibleError> {
        // Present immediately so the first drawn frame is visible without
        // waiting a browser tick, then yield to the next animation frame to
        // pace the loop.
        self.present();
        next_animation_frame().await;
        Ok(())
    }
}
