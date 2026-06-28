//! A browser-simulated CYD device.
//!
//! [`CydWasm`] implements the device-agnostic
//! [`Cyd`](linkage_blaze_cyd_core::Cyd) trait against an HTML canvas, so the
//! same generic example code that drives the real esp32 `CydEsp` also runs in a
//! web page. Its [`CydFrameWasm::flush_at`] awaits the next browser animation
//! frame (see [`animation_frame`]), blits the frame to the canvas, then
//! resolves — turning a platform-neutral `loop { draw; flush_at(..).await?; }`
//! into smooth, repaint-paced animation without inverting the loop into a state
//! machine.

mod animation_frame;

use core::convert::Infallible;

use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{MonoFont, MonoTextStyle},
    pixelcolor::{IntoStorage, Rgb565, Rgb888},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    text::{Baseline, Text},
};
use linkage_blaze_core::PixelTarget;
use linkage_blaze_cyd_core::{Cyd, CydFrame, Orientation, TouchInputEvent};
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData};

pub use animation_frame::next_animation_frame;

/// A CYD display simulated on an HTML canvas.
pub struct CydWasm {
    context: CanvasRenderingContext2d,
    size: Size,
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
            background565: Rgb565::from(background),
            foreground565: Rgb565::from(foreground),
            font,
        }
    }
}

impl Cyd for CydWasm {
    // Presenting to a canvas cannot fail, so the device-agnostic render loop
    // never has a real error to propagate.
    type Error = Infallible;
    type Frame<'a> = CydFrameWasm<'a>;

    fn screen_size(&self) -> Size {
        self.size
    }

    fn frame_mut(&mut self, size: Size) -> CydFrameWasm<'_> {
        let pixel_count = size.width as usize * size.height as usize;
        // Every new frame starts cleared to the device background so callers
        // never have to clear it themselves.
        let pixels = vec![self.background565.into_storage(); pixel_count];
        CydFrameWasm {
            context: &self.context,
            pixels,
            size,
            foreground565: self.foreground565,
            font: self.font,
        }
    }

    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, Infallible> {
        // Touch is not wired up yet; the WASM examples (ballet) do not use it.
        Ok(None)
    }
}

/// A single in-progress frame backed by an `Rgb565` pixel buffer.
pub struct CydFrameWasm<'a> {
    context: &'a CanvasRenderingContext2d,
    pixels: Vec<u16>,
    size: Size,
    foreground565: Rgb565,
    font: &'static MonoFont<'static>,
}

impl CydFrameWasm<'_> {
    fn width(&self) -> usize {
        self.size.width as usize
    }

    fn height(&self) -> usize {
        self.size.height as usize
    }

    /// Convert the `Rgb565` buffer to RGBA8 and `putImageData` it at `top_left`.
    fn present(&self, top_left: Point) {
        let mut bytes = Vec::with_capacity(self.pixels.len() * 4);
        for pixel in &self.pixels {
            bytes.push(scale_channel((pixel >> 11) & 0x1f, 31));
            bytes.push(scale_channel((pixel >> 5) & 0x3f, 63));
            bytes.push(scale_channel(pixel & 0x1f, 31));
            bytes.push(255);
        }
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&bytes),
            self.size.width,
            self.size.height,
        )
        .expect("ImageData dimensions match the pixel buffer");
        self.context
            .put_image_data(&image_data, f64::from(top_left.x), f64::from(top_left.y))
            .expect("put_image_data with in-bounds coordinates cannot fail");
    }
}

impl DrawTarget for CydFrameWasm<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.pixels.fill(color.into_storage());
        Ok(())
    }

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let width = self.width() as i32;
        let height = self.height() as i32;
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.x < width && point.y >= 0 && point.y < height {
                let index = point.y as usize * self.width() + point.x as usize;
                self.pixels[index] = color.into_storage();
            }
        }
        Ok(())
    }
}

impl OriginDimensions for CydFrameWasm<'_> {
    fn size(&self) -> Size {
        self.size
    }
}

impl PixelTarget for CydFrameWasm<'_> {
    fn width(&self) -> usize {
        CydFrameWasm::width(self)
    }

    fn height(&self) -> usize {
        CydFrameWasm::height(self)
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let stride = self.width();
        self.pixels[y * stride + x] = Rgb565::from(color).into_storage();
    }

    /// The frame buffer already stores RGB565, so a decoded image pixel can be
    /// written verbatim with no RGB888 round-trip.
    fn put_pixel_565(&mut self, x: usize, y: usize, rgb565: u16) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let stride = self.width();
        self.pixels[y * stride + x] = rgb565;
    }
}

impl CydFrame for CydFrameWasm<'_> {
    type Error = Infallible;

    fn write_text(&mut self, text: &str) -> &mut Self {
        let style = MonoTextStyle::new(self.font, self.foreground565);
        Text::with_baseline(text, Point::new(0, 0), style, Baseline::Top)
            .draw(self)
            .expect("drawing onto an Infallible frame cannot fail");
        self
    }

    async fn flush_at(&mut self, top_left: Point) -> Result<(), Infallible> {
        // The frame boundary: yield to the browser, then present the
        // freshly-drawn buffer so it paints on this animation frame.
        next_animation_frame().await;
        self.present(top_left);
        Ok(())
    }
}

/// Expand a 5- or 6-bit `Rgb565` channel to 8 bits.
fn scale_channel(value: u16, max: u16) -> u8 {
    ((value * 255) / max) as u8
}
