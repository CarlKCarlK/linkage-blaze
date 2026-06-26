//! Device-agnostic abstraction of a tiled CYD-style display.
//!
//! Modeled on device-envoy's device traits (for example `Led2d`): the generic
//! example logic talks only to these traits, so the same code drives a real
//! esp32 CYD panel and (in the future) a WASM-simulated one.
//!
//! A [`CydSurface`] hands out per-region [frames](CydFrameOps); each frame is an
//! [`embedded_graphics`] [`DrawTarget`] that starts cleared to the device
//! background, can have a line of default-style text written into it, and is
//! flushed to a screen position. This mirrors the inherent `Cyd::frame_mut` /
//! `CydFrame` API on the concrete esp implementation.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Size},
};
use linkage_blaze_core::PixelTarget;

/// A tiled display surface that hands out cleared, region-sized frames.
pub trait CydSurface {
    /// Error returned when flushing a frame to the panel.
    type FlushError;

    /// The per-region frame type this surface produces.
    type Frame<'a>: CydFrameOps<FlushError = Self::FlushError>
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
}

/// A single in-progress frame: a `Rgb565` draw target that can be flushed.
///
/// Also a [`PixelTarget`] so projected linkage draw items can render into it.
pub trait CydFrameOps: DrawTarget<Color = Rgb565, Error = Infallible> + PixelTarget {
    /// Error returned when flushing this frame to the panel.
    type FlushError;

    /// Draw `text` at the frame's top-left using the device default font and
    /// foreground color. Returns `&mut Self` for chaining.
    fn write_text(&mut self, text: &str) -> &mut Self;

    /// Flush the frame's pixels to the panel at `top_left` (screen coordinates).
    fn flush_at(&mut self, top_left: Point) -> Result<(), Self::FlushError>;
}
