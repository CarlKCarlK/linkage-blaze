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
    Pixel,
    pixelcolor::Rgb565,
    prelude::{Dimensions, DrawTarget, Point, Size},
    primitives::Rectangle,
};
use linkage_blaze_core::{PixelTarget, Rgb888};

use crate::{
    TouchInputEvent,
    tiling::{Region, TileGrid},
    translated::TranslatedDrawTarget,
};

/// A CYD display: hands out cleared, region-sized frames and reads calibrated touch.
pub trait Cyd {
    /// Error returned when flushing a frame or reading touch fails.
    type Error;

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

    /// Borrow a frame covering `region`, cleared to the device background color.
    ///
    /// The frame remembers its `region`, so [`CydFrame::flush`] presents it at
    /// the region's top-left with no separate position argument.
    fn frame_mut(&mut self, region: Region) -> Self::Frame<'_>;

    /// Borrow a full-screen frame, cleared to the device background color.
    fn full_frame_mut(&mut self) -> Self::Frame<'_> {
        self.frame_mut(Region::new(Point::zero(), self.screen_size()))
    }

    /// Read the next calibrated, screen-space touch event, if any.
    ///
    /// Returns `Ok(None)` when there is no pending touch (including devices
    /// constructed without touch). Errors only on a hardware/read failure.
    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, Self::Error>;

    /// Drive `grid` as a sequence of low-memory tiles.
    ///
    /// The returned [`Tiles`] is a lending/streaming iterator (it does not
    /// implement [`Iterator`], because each [`Tile`] borrows the device's single
    /// reusable frame buffer). Each tile draws in physical-screen coordinates,
    /// knows its own position, and is presented with [`Tile::flush`]:
    ///
    /// ```rust,no_run
    /// # use linkage_blaze_cyd_core::{Cyd, tiling::TileGrid};
    /// # async fn draw<C: Cyd>(cyd: &mut C, grid: TileGrid) -> Result<(), C::Error> {
    /// let mut tiles = cyd.tiles(grid);
    /// while let Some(mut tile) = tiles.next() {
    ///     // draw into `tile` in screen coordinates...
    ///     tile.flush().await?;
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
/// [`Iterator`]: each [`Tile`] borrows the device's single reusable frame
/// buffer, so only one tile can be live at a time. Iterate with a
/// `while let Some(mut tile) = tiles.next()` loop.
pub struct Tiles<'a, C: Cyd> {
    cyd: &'a mut C,
    grid: TileGrid,
    column: usize,
    row: usize,
}

impl<C: Cyd> Tiles<'_, C> {
    /// Borrow the next tile, cleared to the device background color, or `None`
    /// once every tile has been yielded.
    ///
    /// Tiles are visited in row-major order (each row left-to-right), skipping
    /// any `(column, row)` that falls entirely outside the region.
    // This is a lending iterator: each `Tile` borrows the device's single
    // reusable frame buffer, so it cannot implement `Iterator` (whose `next`
    // returns an item that outlives the `&mut self` borrow). The `next` name is
    // the intended call shape, so allow the trait-shape lint here.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Tile<'_, C>> {
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
                return Some(Tile {
                    frame: self.cyd.frame_mut(region),
                    region,
                });
            }
        }
    }
}

/// A single tile's frame, drawn in physical-screen coordinates.
///
/// Yielded by [`Tiles::next`]. Drawing commands use physical-screen coordinates
/// (the tile's [`top_left`](Self::top_left) is subtracted before writing into
/// the tile-local buffer), and [`flush`](Self::flush) presents the tile at that
/// same position.
pub struct Tile<'a, C: Cyd + 'a> {
    frame: C::Frame<'a>,
    region: Region,
}

impl<'a, C: Cyd + 'a> Tile<'a, C> {
    /// This tile's region (top-left and size) in physical-screen coordinates.
    #[must_use]
    pub fn region(&self) -> Region {
        self.region
    }

    /// This tile's top-left corner in physical-screen coordinates.
    #[must_use]
    pub fn top_left(&self) -> Point {
        self.region.top_left
    }

    /// This tile's size in pixels.
    #[must_use]
    pub fn size(&self) -> Size {
        self.region.size
    }

    /// Present this tile's pixels at its [`top_left`](Self::top_left).
    pub async fn flush(&mut self) -> Result<(), C::Error> {
        self.frame.flush().await
    }
}

impl<'a, C: Cyd + 'a> Dimensions for Tile<'a, C> {
    fn bounding_box(&self) -> Rectangle {
        let bounding_box = self.frame.bounding_box();
        Rectangle::new(
            bounding_box.top_left + self.region.top_left,
            bounding_box.size,
        )
    }
}

impl<'a, C: Cyd + 'a> DrawTarget for Tile<'a, C> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        TranslatedDrawTarget::new(&mut self.frame, self.region.top_left).draw_iter(pixels)
    }
}

impl<'a, C: Cyd + 'a> PixelTarget for Tile<'a, C> {
    fn width(&self) -> usize {
        self.region.top_left.x as usize + self.frame.width()
    }

    fn height(&self) -> usize {
        self.region.top_left.y as usize + self.frame.height()
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        TranslatedDrawTarget::new(&mut self.frame, self.region.top_left).put_pixel(x, y, color);
    }

    fn put_pixel_565(&mut self, x: usize, y: usize, rgb565: u16) {
        TranslatedDrawTarget::new(&mut self.frame, self.region.top_left)
            .put_pixel_565(x, y, rgb565);
    }
}

/// A single in-progress frame: a `Rgb565` draw target that can be flushed.
///
/// Also a [`PixelTarget`] so projected linkage draw items can render into it.
pub trait CydFrame: DrawTarget<Color = Rgb565, Error = Infallible> + PixelTarget {
    /// Error returned when flushing this frame to the panel.
    type Error;

    /// Draw `text` at the frame's top-left using the device default font and
    /// foreground color. Returns `&mut Self` for chaining.
    fn write_text(&mut self, text: &str) -> &mut Self;

    /// Present the frame's pixels at its region's top-left (screen coordinates).
    ///
    /// The frame was created over a [`Region`] by [`Cyd::frame_mut`], so it
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
