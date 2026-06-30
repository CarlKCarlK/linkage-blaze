//! A browser-simulated CYD device.
//!
//! [`CydWasm`] implements the device-agnostic
//! [`Cyd`](linkage_blaze_cyd_core::Cyd) trait against an HTML canvas, so the
//! same generic example code that drives the real esp32 `CydEsp` also runs in a
//! web page. Its [`CydFrameWasm::flush`] awaits the next browser animation
//! frame (see [`animation_frame`]), blits the frame to the canvas, then
//! resolves — turning a platform-neutral `loop { draw; flush().await?; }`
//! into smooth, repaint-paced animation without inverting the loop into a state
//! machine.

mod animation_frame;

use core::convert::Infallible;

use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{MonoFont, MonoTextStyle},
    pixelcolor::{IntoStorage, Rgb565, Rgb888},
    prelude::{Dimensions, DrawTarget, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use linkage_blaze_core::PixelTarget;
use linkage_blaze_cyd_core::{Cyd, CydFrame, CydInfallibleError, Orientation, TouchInputEvent};
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
        Self {
            context,
            size: orientation.size(),
            background,
            foreground,
            background565: Rgb565::from(background),
            foreground565: Rgb565::from(foreground),
            font,
        }
    }
}

impl Cyd for CydWasm {
    // Presenting to a canvas cannot fail, so the device-agnostic render loop
    // never has a real error to propagate.
    type Error = CydInfallibleError;
    type Frame<'a> = CydFrameWasm<'a>;

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

    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, CydInfallibleError> {
        // Touch is not wired up yet; the WASM examples (ballet) do not use it.
        Ok(None)
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

        put_image_data(&self.context, rectangle, &bytes);
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
        let mut bytes =
            Vec::with_capacity(rectangle.size.width as usize * rectangle.size.height as usize * 4);
        for pixel in pixels {
            push_rgb565_rgba(&mut bytes, pixel.into_storage());
        }

        put_image_data(&self.context, rectangle, &bytes);
        Ok(())
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
    bytes.push(scale_channel((pixel >> 11) & 0x1f, 31));
    bytes.push(scale_channel((pixel >> 5) & 0x3f, 63));
    bytes.push(scale_channel(pixel & 0x1f, 31));
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
            bytes.push(scale_channel((pixel >> 11) & 0x1f, 31));
            bytes.push(scale_channel((pixel >> 5) & 0x3f, 63));
            bytes.push(scale_channel(pixel & 0x1f, 31));
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
        // The frame boundary: yield to the browser, then present the
        // freshly-drawn buffer so it paints on this animation frame.
        next_animation_frame().await;
        self.present();
        Ok(())
    }
}

/// Expand a 5- or 6-bit `Rgb565` channel to 8 bits.
fn scale_channel(value: u16, max: u16) -> u8 {
    ((value * 255) / max) as u8
}
