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

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Size},
};
use linkage_blaze_core::PixelTarget;

use crate::TouchInputEvent;

/// A CYD display: hands out cleared, region-sized frames and reads calibrated touch.
pub trait Cyd {
    /// Error returned when flushing a frame or reading touch fails.
    type Error;

    /// The per-region frame type this device produces.
    type Frame<'a>: CydFrame<Error = Self::Error>
    where
        Self: 'a;

    /// Oriented screen size for the configured orientation.
    fn screen_size(&self) -> Size;

    /// Borrow a frame of `size`, cleared to the device background color.
    fn frame_mut(&mut self, size: Size) -> Self::Frame<'_>;

    /// Borrow a full-screen frame, cleared to the device background color.
    fn full_frame_mut(&mut self) -> Self::Frame<'_> {
        self.frame_mut(self.screen_size())
    }

    /// Read the next calibrated, screen-space touch event, if any.
    ///
    /// Returns `Ok(None)` when there is no pending touch (including devices
    /// constructed without touch). Errors only on a hardware/read failure.
    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, Self::Error>;
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

    /// Flush the frame's pixels to the panel at `top_left` (screen coordinates).
    fn flush_at(&mut self, top_left: Point) -> Result<(), <Self as CydFrame>::Error>;
}
