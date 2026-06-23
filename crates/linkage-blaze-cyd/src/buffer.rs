use core::convert::Infallible;

use embedded_graphics::{
    Pixel,
    pixelcolor::{IntoStorage, Rgb565},
    prelude::{DrawTarget, OriginDimensions, Size},
};
use static_cell::StaticCell;

pub trait RectPixels {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn raw_pixels(&self) -> &[u16];
}

pub struct RectBuffer<const WIDTH: usize, const HEIGHT: usize, const PIXELS: usize> {
    pixels: [u16; PIXELS],
}

pub struct RectWorkspace<const PIXELS: usize> {
    pixels: [u16; PIXELS],
}

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

    pub fn fill(&mut self, color: Rgb565) {
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
        self.fill(color);
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

impl<const PIXELS: usize> RectWorkspace<PIXELS> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: [0; PIXELS],
        }
    }

    pub fn init_static(storage: &'static StaticCell<Self>) -> &'static mut RectWorkspace<PIXELS> {
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

impl<const PIXELS: usize> Default for RectWorkspace<PIXELS> {
    fn default() -> Self {
        Self::new()
    }
}

impl RectView<'_> {
    pub fn fill(&mut self, color: Rgb565) {
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
        self.fill(color);
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
