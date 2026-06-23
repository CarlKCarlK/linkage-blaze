use core::convert::Infallible;

use embedded_graphics::{
    Pixel,
    pixelcolor::{IntoStorage, Rgb565},
    prelude::{DrawTarget, OriginDimensions, Size},
};
use static_cell::StaticCell;

// todo000 review this name w.r.t. PixelBuffer.
pub trait RectPixels {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn raw_pixels(&self) -> &[u16];
}

// todo000 review this name w.r.t. PixelBuffer.
pub struct RectBuffer<const WIDTH: usize, const HEIGHT: usize, const PIXELS: usize> {
    pixels: [u16; PIXELS],
}

pub struct PixelBuffer<const PIXELS: usize> {
    pixels: [u16; PIXELS],
}

// todo000 review this name w.r.t. PixelBuffer.
pub struct RectView<'a> {
    width: usize,
    height: usize,
    pixels: &'a mut [u16],
}

impl<const WIDTH: usize, const HEIGHT: usize, const PIXELS: usize>
    RectBuffer<WIDTH, HEIGHT, PIXELS>
{
    #[must_use]
    pub fn new() -> Self {
        assert!(PIXELS == WIDTH * HEIGHT, "PIXELS must equal WIDTH * HEIGHT");
        Self {
            pixels: [0; PIXELS],
        }
    }

    pub fn init_static(
        storage: &'static StaticCell<Self>,
    ) -> &'static mut RectBuffer<WIDTH, HEIGHT, PIXELS> {
        storage.init_with(Self::new)
    }

    pub fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color.into_storage());
    }

    #[must_use]
    pub fn raw_pixels(&self) -> &[u16; PIXELS] {
        &self.pixels
    }
}

impl<const WIDTH: usize, const HEIGHT: usize, const PIXELS: usize> Default
    for RectBuffer<WIDTH, HEIGHT, PIXELS>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize, const PIXELS: usize> RectPixels
    for RectBuffer<WIDTH, HEIGHT, PIXELS>
{
    fn width(&self) -> usize {
        WIDTH
    }

    fn height(&self) -> usize {
        HEIGHT
    }

    fn raw_pixels(&self) -> &[u16] {
        &self.pixels
    }
}

impl<const WIDTH: usize, const HEIGHT: usize, const PIXELS: usize> DrawTarget
    for RectBuffer<WIDTH, HEIGHT, PIXELS>
{
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let point_x = point.x as usize;
            let point_y = point.y as usize;
            if point_x >= WIDTH || point_y >= HEIGHT {
                continue;
            }
            self.pixels[point_y * WIDTH + point_x] = color.into_storage();
        }
        Ok(())
    }
}

impl<const PIXELS: usize> PixelBuffer<PIXELS> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: [0; PIXELS],
        }
    }

    pub fn init_static(storage: &'static StaticCell<Self>) -> &'static mut PixelBuffer<PIXELS> {
        storage.init_with(Self::new)
    }

    pub fn view_mut(&mut self, width: usize, height: usize) -> RectView<'_> {
        let pixel_count = width * height;
        assert!(pixel_count <= PIXELS, "view must fit in workspace");
        RectView {
            width,
            height,
            pixels: &mut self.pixels[..pixel_count],
        }
    }
}

impl<const PIXELS: usize> Default for PixelBuffer<PIXELS> {
    fn default() -> Self {
        Self::new()
    }
}

// todo00 understand this code.
/// A pixel buffer that a [`Cyd`](crate::Cyd) can own: it can be initialized into
/// a `'static` cell and hand out [`RectView`]s. Implemented for any
/// [`PixelBuffer<PIXELS>`], so an app picks the size via the buffer type it names
/// in its [`CydStatic`](crate::CydStatic).
pub trait DynPixelBuffer: 'static {
    /// Initialize this buffer inside a `'static` cell, returning a unique reference.
    fn init_static(cell: &'static StaticCell<Self>) -> &'static mut Self
    where
        Self: Sized;

    /// Borrow a `width`×`height` view out of the buffer (must fit the capacity).
    fn view_mut(&mut self, width: usize, height: usize) -> RectView<'_>;
}

impl<const PIXELS: usize> DynPixelBuffer for PixelBuffer<PIXELS> {
    fn init_static(cell: &'static StaticCell<Self>) -> &'static mut Self {
        cell.init_with(Self::new)
    }

    fn view_mut(&mut self, width: usize, height: usize) -> RectView<'_> {
        PixelBuffer::view_mut(self, width, height)
    }
}

impl RectView<'_> {
    pub fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color.into_storage());
    }

    pub fn raw_pixels_mut(&mut self) -> &mut [u16] {
        self.pixels
    }
}

impl RectPixels for RectView<'_> {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn raw_pixels(&self) -> &[u16] {
        self.pixels
    }
}

impl DrawTarget for RectView<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let point_x = point.x as usize;
            let point_y = point.y as usize;
            if point_x >= self.width || point_y >= self.height {
                continue;
            }
            self.pixels[point_y * self.width + point_x] = color.into_storage();
        }
        Ok(())
    }
}

impl OriginDimensions for RectView<'_> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl<const WIDTH: usize, const HEIGHT: usize, const PIXELS: usize> OriginDimensions
    for RectBuffer<WIDTH, HEIGHT, PIXELS>
{
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}
