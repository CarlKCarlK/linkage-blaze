//! The generic "ballet" example: free-runs a motion-captured pirouette across
//! the full screen, with an fps / slow-motion status line.

use core::{
    convert::Infallible,
    fmt::{self, Write},
};

use device_envoy_core::cyd::{
    CydDisplay,
    display::{CydFrame, Image565Fixed, Orientation, tga565},
};
use embassy_time::{Duration, Instant};
use embedded_graphics::mono_font::{MonoFont, ascii::FONT_6X10};
use linkage_blaze_core::{
    LinkageFixed, LinkageView, Point, Projection, Rgb888, bvh_motion, bvh_parse::BvhMotion,
    linkage, linkage_fixed,
};
use linkage_blaze_cyd_3d::DrawItem3dExt;

// ── Screen policy ─────────────────────────────────────────────────────────────

// todo000 are there 4 orientations?
pub const ORIENTATION: Orientation = Orientation::Portrait;
pub const TOP_FONT: MonoFont<'static> = FONT_6X10;

// ── Palette ──────────────────────────────────────────────────────────────────

// Default colors.
pub const BACKGROUND: Rgb888 = Rgb888::new(13, 13, 11); // near-black warm charcoal
pub const FOREGROUND: Rgb888 = Rgb888::new(255, 214, 123); // warm pale gold

// The linkage (skeleton) previously converted from BVH to lb.rs format.
const LINKAGE0: LinkageFixed<{ MOTION.dof() }, 6, 538> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");
const LINKAGE: LinkageView<{ MOTION.dof() }, 6> = LinkageFixed::<0, 0, 3>::start()
    .pen_color(FOREGROUND)
    .pen_width(3.2)
    .combine::<{ MOTION.dof() }, 6, 538, { MOTION.dof() }, 6, 540>(LINKAGE0)
    .view();

// The motion capture data, read at compile time from BVH and stored in the binary.
#[allow(long_running_const_eval)]
// This can take ~8 seconds to compile and will generate a warning.
const MOTION: BvhMotion<132, 592> = bvh_motion!("../../linkage-blaze-mocap/samples/pirouette.bvh");
const MOTION_FPS: f32 = 120.0; // the mocap was captured at 120fps, so we can run it at that speed.

// A background bitmap read at compile time and stored in the binary.
const BACKGROUND_BITMAP: Image565Fixed<240, 320, { 240 * 320 }> =
    tga565!("../assets/ballet_background.tga", 240, 320);

// How we convert 3D points in the linkage to 2D points in a frame.
const PROJECTION: Projection = Projection::front_orthographic(
    /*target origin*/ Point::new(84, 275),
    /* scale */ 1.4,
);

// ── Generic entry point ────────────────────────────────────────────────────────

/// Run the ballet example forever on a [`Cyd`] implementation (for example `CydEsp` or `CydWasm`).
pub async fn ballet<CydDisplayDevice>(
    display: &mut CydDisplayDevice,
) -> Result<Infallible, Error<CydDisplayDevice::Error>>
where
    CydDisplayDevice: CydDisplay,
{
    let mut last_sample_duration: Option<Duration> = None;

    // Loop the motion control samples forever.
    loop {
        for (sample_index, params) in MOTION.samples().enumerate() {
            let started = Instant::now();

            // Create a frame to draw into. It uses preallocated memory.
            let mut cyd_frame = display.full_frame_mut();

            // Draw the background bitmap into the frame via bulk copy.
            // .draw(...) works too, but is slower.
            BACKGROUND_BITMAP.copy_to(&mut cyd_frame)?;

            // Apply the mocap params to the linkage and draw everything to the frame.
            for draw_item_3d in LINKAGE.draw_items_3d(&params) {
                draw_item_3d.project(&PROJECTION).draw(&mut cyd_frame);
            }

            // Create a status line and write it to the frame.
            let status = status_text(sample_index, last_sample_duration)?;

            // Send the frame to the display.
            cyd_frame
                .write_text(&status)
                .flush()
                .await
                .map_err(Error::Flush)?;

            last_sample_duration = Some(sample_duration(started));
        }
    }
}

#[cfg(not(test))]
fn sample_duration(started: Instant) -> Duration {
    Instant::now() - started
}

#[cfg(test)]
fn sample_duration(_started: Instant) -> Duration {
    Duration::from_millis(10)
}

fn status_text(
    sample_index: usize,
    last_sample_duration: Option<Duration>,
) -> Result<heapless::String<64>, StatusTextError> {
    let mut status_text = heapless::String::<64>::new();

    let Some(last_sample_duration) = last_sample_duration else {
        // return the empty string
        return Ok(status_text);
    };

    let elapsed_secs = last_sample_duration.as_micros() as f32 * 1e-6_f32;
    let fps = elapsed_secs.recip();
    let slomo = MOTION_FPS / fps;

    write!(
        &mut status_text,
        " #{:03}/{:03}  |  {:>4.1} fps  |  slomo {:>4.1}x",
        sample_index + 1,
        MOTION.sample_count(),
        fps,
        slomo,
    )?;
    Ok(status_text)
}

#[cfg(test)]
mod tests {
    use device_envoy_core::cyd::Cyd as _;
    use device_envoy_core::memory::{
        MemoryCyd, MemoryCydError, assert_framebuffer_matches_expected_png,
    };
    use embedded_graphics::geometry::Point;
    use embedded_graphics::primitives::Rectangle;
    use futures_executor::block_on;

    use super::{BACKGROUND, Error, FOREGROUND, ORIENTATION, TOP_FONT, ballet};

    const SMOKE_TEST_FRAME_BUDGET: usize = 5;

    #[test]
    fn ballet_runs_bounded_frames_and_flushes_within_screen_bounds() {
        let mut memory_cyd = MemoryCyd::new(ORIENTATION.size(), BACKGROUND, FOREGROUND, &TOP_FONT);
        memory_cyd.set_frame_budget(SMOKE_TEST_FRAME_BUDGET);

        let ballet_result = {
            let (mut display, _touch) = memory_cyd.parts();
            block_on(ballet(&mut display))
        };

        let ballet_error =
            ballet_result.expect_err("the free-running loop should stop at the frame budget");
        assert!(matches!(
            ballet_error,
            Error::Flush(MemoryCydError::OutOfFrames)
        ));
        assert_eq!(memory_cyd.flush_count(), SMOKE_TEST_FRAME_BUDGET);
        assert_eq!(
            memory_cyd.last_flush_rectangle(),
            Some(Rectangle::new(Point::zero(), ORIENTATION.size()))
        );
    }

    #[test]
    fn ballet_renders_expected_frame() {
        const GOLDEN_TEST_FRAME_BUDGET: usize = 225;

        let mut memory_cyd = MemoryCyd::new(ORIENTATION.size(), BACKGROUND, FOREGROUND, &TOP_FONT);
        memory_cyd.set_frame_budget(GOLDEN_TEST_FRAME_BUDGET);

        let ballet_result = {
            let (mut display, _touch) = memory_cyd.parts();
            block_on(ballet(&mut display))
        };
        ballet_result.expect_err("the free-running loop should stop at the frame budget");
        assert_eq!(memory_cyd.flush_count(), GOLDEN_TEST_FRAME_BUDGET);

        assert_framebuffer_matches_expected_png(
            &memory_cyd,
            env!("CARGO_MANIFEST_DIR"),
            "ballet.png",
        )
        .expect("rendered frame should match the golden image");
    }
}

#[derive(Debug, derive_more::From)]
pub struct StatusTextError(pub fmt::Error);

// todo0000 review this.
/// Error from the generic ballet loop, generic over the surface's flush error `F`.
///
/// Our own error types ([`StatusTextError`], [`device_envoy_core::Error`]) get a derived
/// `From`, so they propagate with a plain `?`. The device's flush error `F` is
/// the one exception: a blanket `From<F>` would be greedy enough to collide
/// with those concrete `From`s (Rust can't rule out a future
/// `F: CydIoError == device_envoy_core::Error`), so flush is converted explicitly with
/// `.map_err(Error::Flush)` at the call site.
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// Formatting the status line failed.
    StatusText(StatusTextError),
    /// A device-envoy-core operation failed (for example, the background bitmap's
    /// dimensions didn't match the frame's).
    Core(device_envoy_core::Error),
    /// Flushing a frame to the display failed.
    #[from(ignore)]
    Flush(F),
}
