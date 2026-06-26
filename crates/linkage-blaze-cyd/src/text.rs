//! Convenience text rendering for [`CydFrame`].
//!
//! This mirrors the device-envoy `led2d` text helpers: the device owns a single
//! default style (background, foreground, font) and [`CydFrame::write_text`]
//! drops a line of text into a frame using that default, without repeating the
//! [`Text`] / [`MonoTextStyle`] / [`Baseline`] boilerplate each time. Combined
//! with per-region frames (see [`Cyd::frame_mut`]), this lets each status or time
//! message own its own area and be drawn in one call.
//!
//! There is intentionally exactly one convenience method. For a different font,
//! color, alignment, or baseline, draw with embedded-graphics directly against
//! the frame — that is the escape hatch.

use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_9X15_BOLD},
    prelude::Point,
    text::{Baseline, Text},
};

use crate::CydFrame;

/// Font apps typically pass to [`Cyd::new_display_only`] as the device default.
pub const DEFAULT_FONT: MonoFont<'static> = FONT_9X15_BOLD;

impl CydFrame<'_> {
    /// Draw `text` at the frame's top-left (0, 0) using the device default font
    /// and foreground color.
    ///
    /// For any other font, color, alignment, or baseline, draw with
    /// embedded-graphics directly against this frame.
    pub fn write_text(&mut self, text: &str) -> &mut Self {
        Text::with_baseline(
            text,
            Point::new(0, 0),
            MonoTextStyle::new(self.font, self.foreground565),
            Baseline::Top,
        )
        .draw(self)
        .expect("drawing text to an Infallible CYD frame cannot fail");
        self
    }
}
