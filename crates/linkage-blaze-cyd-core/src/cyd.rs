//! The opinionated CYD device trait.
//!
//! Modeled on device-envoy's opinionated device abstractions (for example
//! `WifiAuto`, which exposes the useful 95% path rather than raw wifi): [`Cyd`]
//! bundles the CYD's defining capabilities — tiled drawing and calibrated touch
//! — into one device. Generic example logic talks only to this trait, so the
//! same code drives the real esp32 `CydEsp` and a future WASM `CydWasm`.
//!
//! A [`Cyd`] hands out per-region [frames](CydFrame); each frame starts cleared
//! to the device background, can have a line of default-style text written into
//! it, and is flushed to a screen position. Touch reads return calibrated,
//! screen-space [`TouchInputEvent`]s (or `None` when there is no touch).

use core::{convert::Infallible, future::Future};

use embedded_graphics::{
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::{DrawTarget, Point, Size},
    primitives::Rectangle,
};
use linkage_blaze_core::{DrawItem3d, PixelTarget, Projection, Rgb888};

use crate::{ContiguousPixels, DrawItem2d, TouchInputEvent, tiling::TileGrid};

pub trait RegionPixels {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn raw_pixels(&self) -> &[u16];
}

/// Error type used by a CYD device or frame.
///
/// This marker lets downstream generic examples distinguish device/flush errors
/// from their own local errors when using `?`.
pub trait CydFlushError {}

/// Device/flush error for CYD implementations whose presentation path cannot fail.
#[derive(Debug)]
pub enum CydInfallibleError {}

impl CydFlushError for CydInfallibleError {}

/// A CYD display: hands out cleared, region-sized frames and reads calibrated touch.
pub trait Cyd {
    /// Error returned when flushing a frame or reading touch fails.
    type Error: CydFlushError;

    /// The per-region frame type this device produces.
    ///
    /// Its [`CydFrame::Error`] is pinned to this device's [`Cyd::Error`], so
    /// `frame.flush().await?` in generic code propagates a single
    /// `S::Error` (see [`ballet`](../../linkage_blaze_example_core/ballet/index.html)).
    type Frame<'a>: CydFrame<Error = Self::Error>
    where
        Self: 'a;

    /// Oriented screen size for the configured orientation.
    fn screen_size(&self) -> Size;

    /// The device default background color.
    fn background(&self) -> Rgb888;

    /// The device default foreground/text color.
    fn foreground(&self) -> Rgb888;

    /// The device default background color in the native `Rgb565` format.
    fn background_565(&self) -> Rgb565;

    /// The device default foreground/text color in the native `Rgb565` format.
    fn foreground_565(&self) -> Rgb565;

    /// Convert an `Rgb888` color to the device's native `Rgb565` format.
    fn to_rgb565(&self, color: Rgb888) -> Rgb565 {
        Rgb565::from(color)
    }

    /// Borrow a frame covering `region`, cleared to the device background color.
    ///
    /// Drawing commands are interpreted in screen coordinates:
    /// `tile_top_left` is subtracted before pixels are written into the
    /// frame-local buffer. Regular, non-tiled frames use `(0, 0)` and therefore
    /// draw in frame-local coordinates.
    fn frame_mut_with_tile_top_left(
        &mut self,
        region: Rectangle,
        tile_top_left: Point,
    ) -> Self::Frame<'_>;

    /// Borrow a frame covering `region`, cleared to the device background color.
    ///
    /// The frame remembers its `region`, so [`CydFrame::flush`] presents it at
    /// the region's top-left with no separate position argument.
    fn frame_mut(&mut self, region: Rectangle) -> Self::Frame<'_> {
        self.frame_mut_with_tile_top_left(region, Point::zero())
    }

    /// Borrow a full-screen frame, cleared to the device background color.
    fn full_frame_mut(&mut self) -> Self::Frame<'_> {
        self.frame_mut(Rectangle::new(Point::zero(), self.screen_size()))
    }

    /// Read the next calibrated, screen-space touch event, if any.
    ///
    /// Returns `Ok(None)` when there is no pending touch (including devices
    /// constructed without touch). Errors only on a hardware/read failure.
    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, Self::Error>;

    /// Fill `rectangle` immediately in physical-screen coordinates.
    ///
    /// Unlike [`CydFrame::fill`], this is a device-level operation rather than a
    /// frame-local buffered draw. Implementations clip to the physical screen and
    /// treat an empty intersection as a no-op.
    fn fill_rectangle(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), Self::Error>;

    /// Fill `rectangle` immediately from row-major native-color pixels.
    ///
    /// Empty rectangles are a no-op.
    fn fill_contiguous<I>(&mut self, rectangle: Rectangle, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Rgb565>;

    /// Present a native-color region buffer at `top_left`.
    fn flush_at(&mut self, buffer: &impl RegionPixels, top_left: Point) -> Result<(), Self::Error> {
        let rectangle = Rectangle::new(
            top_left,
            Size::new(buffer.width() as u32, buffer.height() as u32),
        );
        self.fill_contiguous(
            rectangle,
            buffer
                .raw_pixels()
                .iter()
                .copied()
                .map(|pixel| Rgb565::from(RawU16::new(pixel))),
        )
    }

    /// Draw projected draw items immediately inside `bounds`.
    fn draw_items_2d<const PRIMITIVE_COUNT: usize>(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        items: &[DrawItem2d],
    ) -> Result<(), Self::Error> {
        let primitive_pixels =
            self.prepare_draw_items_2d::<PRIMITIVE_COUNT>(bounds, background, items);
        self.fill_contiguous(primitive_pixels.bounds(), primitive_pixels.iter())
    }

    /// Compile projected draw items for indexed pixel lookups inside `bounds`.
    fn prepare_draw_items_2d<const PRIMITIVE_COUNT: usize>(
        &self,
        bounds: Rectangle,
        background: Rgb565,
        items: &[DrawItem2d],
    ) -> ContiguousPixels<PRIMITIVE_COUNT> {
        let bounds = bounds.intersection(&Rectangle::new(Point::zero(), self.screen_size()));
        ContiguousPixels::from_draw_items_2d(bounds, background, items.iter().copied())
    }

    /// Project and draw 3D draw items immediately inside `bounds`.
    fn draw_items_3d<const PRIMITIVE_COUNT: usize, I>(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        items: I,
        projection: &Projection,
    ) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = DrawItem3d>,
    {
        let primitive_pixels =
            self.prepare_draw_items_3d::<PRIMITIVE_COUNT, _>(bounds, background, items, projection);
        self.fill_contiguous(primitive_pixels.bounds(), primitive_pixels.iter())
    }

    /// Compile 3D draw items for indexed pixel lookups inside `bounds`.
    fn prepare_draw_items_3d<const PRIMITIVE_COUNT: usize, I>(
        &self,
        bounds: Rectangle,
        background: Rgb565,
        items: I,
        projection: &Projection,
    ) -> ContiguousPixels<PRIMITIVE_COUNT>
    where
        I: IntoIterator<Item = DrawItem3d>,
    {
        let bounds = bounds.intersection(&Rectangle::new(Point::zero(), self.screen_size()));
        ContiguousPixels::from_draw_items_3d(bounds, background, items, projection)
    }

    /// Clear the whole screen to the device default background color.
    ///
    /// New frames already start cleared to this color. This is for immediately
    /// returning the physical screen to the default background between frame
    /// workflows.
    fn clear(&mut self) -> Result<(), Self::Error> {
        self.fill(self.background_565())
    }

    /// Fill the whole screen with an explicit color.
    fn fill(&mut self, color: Rgb565) -> Result<(), Self::Error> {
        self.fill_rectangle(Rectangle::new(Point::zero(), self.screen_size()), color)
    }

    /// Drive `grid` as a sequence of low-memory tiles.
    ///
    /// The returned [`Tiles`] is a lending/streaming iterator (it does not
    /// implement [`Iterator`], because each yielded frame borrows the device's
    /// single reusable frame buffer). Each yielded frame draws in screen
    /// coordinates via each frame's non-zero [`CydFrame::tile_top_left`], and is
    /// presented with [`CydFrame::flush`]:
    ///
    /// ```rust,no_run
    /// # use linkage_blaze_cyd_core::{Cyd, tiling::TileGrid};
    /// # async fn draw<C: Cyd>(cyd: &mut C, grid: TileGrid) -> Result<(), C::Error> {
    /// let mut tiles = cyd.tiles(grid);
    /// while let Some(mut frame) = tiles.next() {
    ///     // draw into `frame` in screen coordinates...
    ///     frame.flush().await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn tiles(&mut self, grid: TileGrid) -> Tiles<'_, Self>
    where
        Self: Sized,
    {
        Tiles {
            cyd: self,
            grid,
            column: 0,
            row: 0,
        }
    }
}

/// A lending/streaming iterator over a [`TileGrid`]'s tiles.
///
/// Created by [`Cyd::tiles`]. This deliberately does *not* implement
/// [`Iterator`]: each yielded frame borrows the device's single reusable frame
/// buffer, so only one frame can be live at a time. Iterate with a
/// `while let Some(mut frame) = tiles.next()` loop.
pub struct Tiles<'a, C: Cyd> {
    cyd: &'a mut C,
    grid: TileGrid,
    column: usize,
    row: usize,
}

impl<C: Cyd> Tiles<'_, C> {
    /// Borrow the next tile-backed frame, cleared to the device background
    /// color, or `None` once every tile has been yielded.
    ///
    /// Tiles are visited in row-major order (each row left-to-right), skipping
    /// any `(column, row)` that falls entirely outside the region.
    // This is a lending iterator: each yielded frame borrows the device's single
    // reusable frame buffer, so it cannot implement `Iterator` (whose `next`
    // returns an item that outlives the `&mut self` borrow). The `next` name is
    // the intended call shape, so allow the trait-shape lint here.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<C::Frame<'_>> {
        let (columns, rows) = (self.grid.columns(), self.grid.rows());
        loop {
            if self.row >= rows {
                return None;
            }
            let region = self.grid.tile(self.column, self.row);
            self.column += 1;
            if self.column >= columns {
                self.column = 0;
                self.row += 1;
            }
            if let Some(region) = region {
                let tile_top_left = region.top_left;
                return Some(self.cyd.frame_mut_with_tile_top_left(region, tile_top_left));
            }
        }
    }
}

/// A single in-progress frame: a `Rgb565` draw target that can be flushed.
///
/// Also a [`PixelTarget`] so projected linkage draw items can render into it.
pub trait CydFrame: DrawTarget<Color = Rgb565, Error = Infallible> + PixelTarget {
    /// Error returned when flushing this frame to the panel.
    type Error;

    /// This frame's tile top-left in screen coordinates.
    ///
    /// This point is subtracted from input drawing commands before pixels reach
    /// this frame's local backing buffer. Regular, non-tiled frames use `(0, 0)`.
    #[must_use]
    fn tile_top_left(&self) -> Point {
        Point::zero()
    }

    /// This frame's region (top-left and size) in physical-screen coordinates.
    fn region(&self) -> Rectangle;

    /// Fill this frame with the device default background color.
    fn clear(&mut self) -> &mut Self;

    /// Fill this frame with an explicit color.
    fn fill(&mut self, color: Rgb565) -> &mut Self;

    /// Draw `text` at the frame's top-left using the device default font and
    /// foreground color. Returns `&mut Self` for chaining.
    fn write_text(&mut self, text: &str) -> &mut Self;

    /// Bulk-copy a full-frame, row-major RGB565 buffer into this frame.
    ///
    /// This is the fast path for a full-screen background: a single
    /// `copy_from_slice` instead of the per-pixel [`DrawTarget`] path (on the
    /// esp32 the per-pixel path makes the ballet loop ~1/3 slower). `src` must
    /// hold exactly one entry per frame pixel — i.e. the source image's
    /// dimensions must match the frame's. A mismatch returns
    /// [`CopySizeError`] rather than panicking or silently corrupting the
    /// buffer.
    fn copy_from_565(&mut self, src: &[u16]) -> Result<(), CopySizeError>;

    /// Present the frame's pixels at its region's top-left (screen coordinates).
    ///
    /// The frame was created over a [`Rectangle`] by [`Cyd::frame_mut`], so it
    /// already knows where it lives and needs no position argument.
    ///
    /// The returned future is the render loop's frame boundary. On the MCU it
    /// flushes over SPI and resolves immediately; on WASM it awaits the next
    /// browser animation frame, blits to the canvas, then resolves — so a
    /// platform-neutral `loop { draw; flush().await?; }` paces itself to
    /// each device's natural present point without inverting into a state
    /// machine.
    fn flush(&mut self) -> impl Future<Output = Result<(), <Self as CydFrame>::Error>>;
}

/// Returned by [`CydFrame::copy_from_565`] when the source buffer's length
/// does not equal the frame's pixel count — i.e. the image's dimensions differ
/// from the frame's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopySizeError {
    /// Number of pixels supplied by the source image.
    pub src_len: usize,
    /// Number of pixels the destination frame holds.
    pub frame_len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::{Pixel, prelude::OriginDimensions};

    struct TestCyd;

    struct TestFrame {
        region: Rectangle,
        tile_top_left: Point,
    }

    impl Cyd for TestCyd {
        type Error = CydInfallibleError;
        type Frame<'a> = TestFrame;

        fn screen_size(&self) -> Size {
            Size::new(320, 240)
        }

        fn background(&self) -> Rgb888 {
            Rgb888::CSS_BLACK
        }

        fn foreground(&self) -> Rgb888 {
            Rgb888::CSS_WHITE
        }

        fn background_565(&self) -> Rgb565 {
            self.to_rgb565(self.background())
        }

        fn foreground_565(&self) -> Rgb565 {
            self.to_rgb565(self.foreground())
        }

        fn frame_mut_with_tile_top_left(
            &mut self,
            region: Rectangle,
            tile_top_left: Point,
        ) -> TestFrame {
            TestFrame {
                region,
                tile_top_left,
            }
        }

        fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, CydInfallibleError> {
            Ok(None)
        }

        fn fill_rectangle(
            &mut self,
            _rectangle: Rectangle,
            _color: Rgb565,
        ) -> Result<(), CydInfallibleError> {
            Ok(())
        }

        fn fill_contiguous<I>(
            &mut self,
            _rectangle: Rectangle,
            _pixels: I,
        ) -> Result<(), CydInfallibleError>
        where
            I: IntoIterator<Item = Rgb565>,
        {
            Ok(())
        }
    }

    impl DrawTarget for TestFrame {
        type Color = Rgb565;
        type Error = Infallible;

        fn draw_iter<I>(&mut self, _pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            Ok(())
        }
    }

    impl OriginDimensions for TestFrame {
        fn size(&self) -> Size {
            self.region.size
        }
    }

    impl PixelTarget for TestFrame {
        fn width(&self) -> usize {
            self.region.size.width as usize
        }

        fn height(&self) -> usize {
            self.region.size.height as usize
        }

        fn put_pixel(&mut self, _x: usize, _y: usize, _color: linkage_blaze_core::Rgb888) {}
    }

    impl CydFrame for TestFrame {
        type Error = CydInfallibleError;

        fn tile_top_left(&self) -> Point {
            self.tile_top_left
        }

        fn region(&self) -> Rectangle {
            self.region
        }

        fn clear(&mut self) -> &mut Self {
            self
        }

        fn fill(&mut self, _color: Rgb565) -> &mut Self {
            self
        }

        fn write_text(&mut self, _text: &str) -> &mut Self {
            self
        }

        fn copy_from_565(&mut self, _src: &[u16]) -> Result<(), CopySizeError> {
            Ok(())
        }

        async fn flush(&mut self) -> Result<(), CydInfallibleError> {
            Ok(())
        }
    }

    #[test]
    fn tiled_frames_use_screen_tile_top_left() {
        let mut cyd = TestCyd;
        let grid = TileGrid::new(Point::new(10, 20), Size::new(8, 6), 2, 2);
        let mut tiles = cyd.tiles(grid);

        let first = tiles.next().expect("first tile exists");
        assert_eq!(
            first.region(),
            Rectangle::new(Point::new(10, 20), Size::new(4, 3))
        );
        assert_eq!(first.tile_top_left(), Point::new(10, 20));
        drop(first);

        let second = tiles.next().expect("second tile exists");
        assert_eq!(
            second.region(),
            Rectangle::new(Point::new(14, 20), Size::new(4, 3))
        );
        assert_eq!(second.tile_top_left(), Point::new(14, 20));
        drop(second);

        let third = tiles.next().expect("third tile exists");
        assert_eq!(
            third.region(),
            Rectangle::new(Point::new(10, 23), Size::new(4, 3))
        );
        assert_eq!(third.tile_top_left(), Point::new(10, 23));
    }
}
