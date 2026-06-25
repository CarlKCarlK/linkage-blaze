//! A coordinate-translating [`DrawTarget`] adapter for tiled rendering.

use embedded_graphics::{
    Pixel,
    prelude::{Dimensions, DrawTarget, Point},
    primitives::Rectangle,
};

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
pub struct TranslatedDrawTarget<'a, D> {
    target: &'a mut D,
    origin: Point,
}

impl<'a, D> TranslatedDrawTarget<'a, D> {
    pub fn new(target: &'a mut D, origin: Point) -> Self {
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
