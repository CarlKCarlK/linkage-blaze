//! A display-calibration test pattern for measuring the panel's gamma.
//!
//! [`gamma_ramp`] paints a 4-column × 16-row grid of flat color patches onto any
//! [`Cyd`] (real device or WASM simulator). The columns are gray, red, green,
//! and blue; the rows step a channel from 0 (top) to 255 (bottom) in equal
//! 8-bit increments. Each patch is authored in `Rgb888` and converted to the
//! device's native `Rgb565` exactly the way real content is, so what you measure
//! is the true end-to-end response.
//!
//! To find the panel's transfer function: run this on the device and on the
//! WASM simulator (whose `Rgb565`→RGBA expansion is *linear*), sample each patch
//! with a color picker, and compare. The simulator readings are the intended
//! (sRGB) values; the device readings are what the panel actually emits. Fitting
//! `device = intended ^ gamma` per patch both recovers the current gamma and
//! reveals whether the curve is a clean power law.
//!
//! Patch geometry is fixed at a 240×320 portrait screen (`COLS × CELL_WIDTH` by
//! `ROWS × CELL_HEIGHT`); row `r` (0 = top) uses 8-bit level
//! `r * 255 / (ROWS - 1)`.

use core::convert::Infallible;

use embedded_graphics::{
    pixelcolor::{Rgb565, Rgb888},
    prelude::{Point, Size},
};
use linkage_blaze_cyd_core::{Cyd, CydFrame, tiling::Region};

/// Number of color columns: gray, red, green, blue.
pub const COLS: usize = 4;
/// Number of brightness rows (0 at top to full at bottom).
pub const ROWS: usize = 16;
/// Patch width in pixels (`COLS * CELL_WIDTH == 240`).
pub const CELL_WIDTH: u32 = 60;
/// Patch height in pixels (`ROWS * CELL_HEIGHT == 320`).
pub const CELL_HEIGHT: u32 = 20;
/// Pixel count of one patch; size a device draw buffer to at least this.
pub const CELL_PIXELS: usize = (CELL_WIDTH * CELL_HEIGHT) as usize;

/// The `Rgb888` color for a patch in `column` (0 gray, 1 red, 2 green, 3 blue)
/// at brightness `level`.
fn cell_color(column: usize, level: u8) -> Rgb888 {
    match column {
        0 => Rgb888::new(level, level, level),
        1 => Rgb888::new(level, 0, 0),
        2 => Rgb888::new(0, level, 0),
        _ => Rgb888::new(0, 0, level),
    }
}

/// Paint the gamma test pattern, then idle forever.
///
/// Expects a 240×320 portrait screen. Returns only if a frame flush fails; on
/// success it never returns (the pattern is static).
pub async fn gamma_ramp<S>(cyd: &mut S) -> Result<Infallible, S::Error>
where
    S: Cyd,
{
    let size = cyd.screen_size();
    assert!(
        size.width == COLS as u32 * CELL_WIDTH && size.height == ROWS as u32 * CELL_HEIGHT,
        "gamma_ramp expects a 240x320 portrait screen"
    );

    let mut row = 0;
    while row < ROWS {
        // Equal 8-bit steps from 0 (top) to 255 (bottom).
        let level = (row * 255 / (ROWS - 1)) as u8;
        let mut column = 0;
        while column < COLS {
            let color = Rgb565::from(cell_color(column, level));
            let region = Region::new(
                Point::new(
                    column as i32 * CELL_WIDTH as i32,
                    row as i32 * CELL_HEIGHT as i32,
                ),
                Size::new(CELL_WIDTH, CELL_HEIGHT),
            );
            let mut frame = cyd.frame_mut(region);
            frame.fill(color);
            frame.flush().await?;
            column += 1;
        }
        row += 1;
    }

    // The pattern is static; park here so the example keeps the screen up.
    loop {
        core::future::pending::<()>().await;
    }
}
