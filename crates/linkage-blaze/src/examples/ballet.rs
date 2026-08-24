//! The generic "ballet" example: free-runs a motion-captured pirouette across
//! the full screen, with an fps / slow-motion status line.

use core::{
    convert::Infallible,
    fmt::{self, Write},
};

use crate::bvh::Motion as BvhMotion;
use crate::render::Projection;
use crate::{Error as LinkageError, LinkageFixed, Rgb888, linkage_file};
use device_envoy_core::{
    Error as CoreError,
    button::Button,
    cyd::{
        CydDisplay,
        display::{CydFrame, Image565Fixed, Orientation, tga},
    },
};
use embassy_time::{Duration, Instant};
use embedded_graphics::mono_font::{MonoFont, ascii::FONT_6X10};
use embedded_graphics::prelude::Point;

// ── Screen policy ─────────────────────────────────────────────────────────────

// The CYD display supports landscape, portrait, and the inverted form of each.
pub const ORIENTATION: Orientation = Orientation::Portrait;
pub const TOP_FONT: MonoFont<'static> = FONT_6X10;

// ── Palette ──────────────────────────────────────────────────────────────────

// Default colors.
pub const BACKGROUND_COLOR: Rgb888 = Rgb888::new(13, 13, 11); // near-black warm charcoal
pub const FOREGROUND_COLOR: Rgb888 = Rgb888::new(255, 214, 123); // warm pale gold

// The linkage (skeleton) previously converted from BVH to lb.rs format.
linkage_file! {
    pirouette {
        file: "../assets/mocap/pirouette.lb.rs",
    }
}
const STYLE: LinkageFixed<0, 0, 3> = LinkageFixed::start()
    .pen_color(FOREGROUND_COLOR)
    .pen_width(3.2);
const LINKAGE: LinkageFixed<
    { pirouette::DOF },
    { pirouette::MARKS },
    { STYLE.step_count() + pirouette::STEP_COUNT - 1 },
> = STYLE.combine(pirouette::view());

// The motion capture data, read at compile time from BVH and stored in the binary.
#[allow(long_running_const_eval)]
// This can take ~8 seconds to compile.
const MOTION: BvhMotion<{ pirouette::DOF }, 592> =
    crate::bvh::motion!("../assets/mocap/pirouette.bvh");
const MOTION_FPS: f32 = 120.0; // the mocap was captured at 120fps, so we can run it at that speed.

// A background_bitmap read at compile time and stored in the binary.
const BACKGROUND_BITMAP: Image565Fixed<240, 320, { 240 * 320 }> =
    tga!("../assets/ballet_background.tga").to_565();

// How we convert 3D points in the linkage to 2D points in a frame.
const PROJECTION: Projection = Projection::front_orthographic(
    Point::new(84, 275), // target origin
    1.4,                 // scale
);

// ── Generic entry point ────────────────────────────────────────────────────────

/// Run the ballet example forever on a [`CydDisplay`] implementation.
pub async fn run<CydDisplayDevice>(
    display: &mut CydDisplayDevice,
    button: &impl Button,
) -> Result<Infallible, Error<CydDisplayDevice::Error>>
where
    CydDisplayDevice: CydDisplay,
{
    let mut last_sample_duration: Option<Duration> = None;
    let mut boot_was_pressed = false;

    // Loop the motion control samples forever.
    loop {
        for (sample_index, params) in MOTION.samples().enumerate() {
            let boot_is_pressed = button.is_pressed();
            if boot_is_pressed && !boot_was_pressed {
                boot_was_pressed = true;
                break;
            }
            boot_was_pressed = boot_is_pressed;
            let started = Instant::now();

            // Create a frame to draw into. It uses preallocated memory.
            let mut cyd_frame = display.full_frame_mut();

            // Draw the background_bitmap into the frame via bulk copy.
            // .draw(...) works too, but is slower.
            BACKGROUND_BITMAP.copy_to(&mut cyd_frame)?;

            // Apply the mocap params to the linkage and draw everything to the frame.
            for draw_item_3d in LINKAGE.view().draw_items_3d(&params)? {
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

#[derive(Debug, derive_more::From)]
pub struct StatusTextError(pub fmt::Error);

/// Errors from the generic ballet loop.
#[derive(Debug, derive_more::From)]
pub enum Error<FlushError> {
    /// A runtime linkage parameter was invalid.
    Linkage(LinkageError),
    /// Formatting the status line failed.
    StatusText(StatusTextError),
    /// A device-envoy-core operation failed (for example, the background_bitmap's
    /// dimensions didn't match the frame's).
    Core(CoreError),
    /// Flushing a frame to the display failed.
    #[from(ignore)]
    Flush(FlushError),
}

#[cfg(not(test))]
fn sample_duration(started: Instant) -> Duration {
    Instant::now() - started
}

#[cfg(test)]
fn sample_duration(_started: Instant) -> Duration {
    Duration::from_millis(1)
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
    use device_envoy_core::{
        button::Button,
        memory::{CydMemory, assert_framebuffer_matches_expected_png},
    };
    use futures_executor::block_on;

    use super::{BACKGROUND_COLOR, FOREGROUND_COLOR, ORIENTATION, TOP_FONT, run};

    fn render_ballet(memory_cyd: &mut CydMemory, button: &impl Button) {
        let mut display = memory_cyd.display();
        block_on(run(&mut display, button))
            .expect_err("the free-running loop should stop at the frame budget");
    }

    #[test]
    fn boot_restarts_the_motion_sequence_at_the_initial_frame() {
        let mut baseline = CydMemory::new(
            ORIENTATION.size(),
            BACKGROUND_COLOR,
            FOREGROUND_COLOR,
            &TOP_FONT,
        );
        baseline.set_frame_budget(2);
        let baseline_button = baseline.button_memory();
        render_ballet(&mut baseline, &baseline_button);

        let mut restarted = CydMemory::new(
            ORIENTATION.size(),
            BACKGROUND_COLOR,
            FOREGROUND_COLOR,
            &TOP_FONT,
        );
        restarted.set_frame_budget(4);
        let mut restarted_button = restarted.button_memory();
        restarted_button.set_pressed_for_frame(2, true);
        render_ballet(&mut restarted, &restarted_button);

        for position_y in 0..ORIENTATION.height() as usize {
            for position_x in 0..ORIENTATION.width() as usize {
                assert_eq!(
                    restarted.pixel(position_x, position_y),
                    baseline.pixel(position_x, position_y),
                    "restarted frame differs at ({position_x}, {position_y})",
                );
            }
        }
    }

    #[test]
    fn ballet_renders_expected_frame() {
        const GOLDEN_TEST_FRAME_BUDGET: usize = 225;

        let mut memory_cyd = CydMemory::new(
            ORIENTATION.size(),
            BACKGROUND_COLOR,
            FOREGROUND_COLOR,
            &TOP_FONT,
        );
        memory_cyd.set_frame_budget(GOLDEN_TEST_FRAME_BUDGET);
        let memory_button = memory_cyd.button_memory();

        let ballet_error = {
            let mut display = memory_cyd.display();
            block_on(run(&mut display, &memory_button))
        }
        .expect_err("the free-running loop should stop at the frame budget");
        drop(ballet_error);

        assert_framebuffer_matches_expected_png(
            &memory_cyd,
            env!("CARGO_MANIFEST_DIR"),
            "ballet.png",
        )
        .expect("rendered frame should match the golden image");
    }
}
