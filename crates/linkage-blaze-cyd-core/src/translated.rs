//! A coordinate-translating [`DrawTarget`] adapter for tiled rendering.

use embedded_graphics::{
    Pixel,
    prelude::{Dimensions, DrawTarget, Point},
    primitives::Rectangle,
};
use linkage_blaze_core::{PixelTarget, Rgb888};

/// Wraps a [`DrawTarget`] and subtracts a fixed origin from every drawing command.
///
/// Drawing commands are issued in a parent (for example physical-screen)
/// coordinate space; the adapter subtracts `origin` to produce the wrapped
/// target's local coordinates. This pairs with tiled rendering: draw in screen
/// coordinates, then flush the tile at the same `origin`.
///
/// ```text
/// screen coordinate → TranslatedDrawTarget subtracts origin → tile-local coordinate
/// ```
pub(crate) struct TranslatedDrawTarget<'a, D> {
    target: &'a mut D,
    origin: Point,
}

impl<'a, D> TranslatedDrawTarget<'a, D> {
    pub(crate) fn new(target: &'a mut D, origin: Point) -> Self {
        Self { target, origin }
    }
}

impl<D: DrawTarget> Dimensions for TranslatedDrawTarget<'_, D> {
    fn bounding_box(&self) -> Rectangle {
        let bounding_box = self.target.bounding_box();
        Rectangle::new(bounding_box.top_left + self.origin, bounding_box.size)
    }
}

impl<D: DrawTarget> DrawTarget for TranslatedDrawTarget<'_, D> {
    type Color = D::Color;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let origin = self.origin;
        self.target.draw_iter(
            pixels
                .into_iter()
                .map(|Pixel(point, color)| Pixel(point - origin, color)),
        )
    }
}

impl<D: PixelTarget> PixelTarget for TranslatedDrawTarget<'_, D> {
    fn width(&self) -> usize {
        screen_extent(self.origin.x, self.target.width())
    }

    fn height(&self) -> usize {
        screen_extent(self.origin.y, self.target.height())
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        let Some(local_x) = local_coordinate(x, self.origin.x) else {
            return;
        };
        let Some(local_y) = local_coordinate(y, self.origin.y) else {
            return;
        };
        if local_x < self.target.width() && local_y < self.target.height() {
            self.target.put_pixel(local_x, local_y, color);
        }
    }

    fn put_pixel_565(&mut self, x: usize, y: usize, rgb565: u16) {
        let Some(local_x) = local_coordinate(x, self.origin.x) else {
            return;
        };
        let Some(local_y) = local_coordinate(y, self.origin.y) else {
            return;
        };
        if local_x < self.target.width() && local_y < self.target.height() {
            self.target.put_pixel_565(local_x, local_y, rgb565);
        }
    }
}

fn screen_extent(origin: i32, local_extent: usize) -> usize {
    usize::try_from(origin)
        .expect("translated PixelTarget origins must be non-negative")
        .checked_add(local_extent)
        .expect("translated PixelTarget screen extent must fit in usize")
}

fn local_coordinate(screen_coordinate: usize, origin: i32) -> Option<usize> {
    let origin = usize::try_from(origin).ok()?;
    screen_coordinate.checked_sub(origin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::pixelcolor::RgbColor;

    struct PixelBuffer {
        pixels: [[Rgb888; 3]; 2],
        raw_pixels: [[u16; 3]; 2],
    }

    impl PixelBuffer {
        const fn new() -> Self {
            Self {
                pixels: [[Rgb888::BLACK; 3]; 2],
                raw_pixels: [[0; 3]; 2],
            }
        }
    }

    impl PixelTarget for PixelBuffer {
        fn width(&self) -> usize {
            3
        }

        fn height(&self) -> usize {
            2
        }

        fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
            self.pixels[y][x] = color;
        }

        fn put_pixel_565(&mut self, x: usize, y: usize, rgb565: u16) {
            self.raw_pixels[y][x] = rgb565;
        }
    }

    #[test]
    fn pixel_target_subtracts_origin_for_screen_space_writes() {
        let mut pixel_buffer = PixelBuffer::new();
        let mut translated = TranslatedDrawTarget::new(&mut pixel_buffer, Point::new(5, 7));

        assert_eq!(translated.width(), 8);
        assert_eq!(translated.height(), 9);

        translated.put_pixel(6, 8, Rgb888::RED);
        translated.put_pixel_565(7, 7, 0xffff); // white

        assert_eq!(pixel_buffer.pixels[1][1], Rgb888::RED);
        assert_eq!(pixel_buffer.raw_pixels[0][2], 0xffff); // white
    }

    #[test]
    fn pixel_target_clips_writes_before_and_after_tile() {
        let mut pixel_buffer = PixelBuffer::new();
        let mut translated = TranslatedDrawTarget::new(&mut pixel_buffer, Point::new(5, 7));

        translated.put_pixel(4, 7, Rgb888::RED);
        translated.put_pixel(8, 7, Rgb888::RED);
        translated.put_pixel(5, 9, Rgb888::RED);

        assert_eq!(pixel_buffer.pixels, [[Rgb888::BLACK; 3]; 2]);
    }
}
