//! The generic "ballet" example: free-runs a motion-captured pirouette across
//! the full screen, with an fps / slow-motion status line.
//!
//! Like [`skeleton_clock`](crate::skeleton_clock), this is the device-agnostic
//! core: generic over a [`CydSurface`] so the same code drives a real esp32 CYD
//! and (later) a WASM-simulated one. The platform shim constructs the concrete
//! device and calls [`ballet`].

use core::convert::Infallible;

use embassy_time::{Duration, Instant};
use embedded_graphics::Drawable;
use linkage_blaze_core::{
    LinkageFixed, LinkageView, Projection, Rgb888, bvh_motion, bvh_parse::BvhMotion, linkage,
    linkage_fixed,
};

use linkage_blaze_cyd_core::{Cyd, CydFrame, Image565, tga565};

/// Device default background color the platform shim should construct its `Cyd`
/// with (also used to clear every frame).
pub const BACKGROUND: Rgb888 = Rgb888::new(13, 13, 11); // near-black warm charcoal
pub const FOREGROUND: Rgb888 = Rgb888::new(255, 214, 123); // warm pale gold
const FIGURE: Rgb888 = FOREGROUND;

const BACKGROUND_BITMAP: Image565<240, 320, { 240 * 320 }> =
    tga565!("../assets/ballet_background.tga", 240, 320);

// todo000 these could be OK, but there are a lot of them. Can't some be done via math?
const CENTER_X: i32 = 84;
const BASELINE_Y: i32 = 275;
const SCALE: f32 = 1.4;

#[allow(long_running_const_eval)]
// This can take ~8 seconds to compile and will generate a warning.
const MOTION: BvhMotion<132, 592> = bvh_motion!("../../linkage-blaze-mocap/samples/pirouette.bvh");
const LINKAGE0: LinkageFixed<{ MOTION.dof() }, 6, 538> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");
const LINKAGE: LinkageView<{ MOTION.dof() }, 6> = LinkageFixed::<0, 0, 3>::start()
    .pen_color(FIGURE)
    .pen_width(3.2)
    .combine::<{ MOTION.dof() }, 6, 538, { MOTION.dof() }, 6, 540>(LINKAGE0)
    .view();

// todo000000 do .view() here

// todo000 still to understand projections.
const PROJECTION: Projection =
    Projection::front_orthographic(CENTER_X as f32, BASELINE_Y as f32, SCALE);

// ── Generic entry point ────────────────────────────────────────────────────────

/// Run the ballet render loop forever, drawn onto `cyd`.
pub async fn ballet<CydDevice>(cyd: &mut CydDevice) -> Result<Infallible, CydDevice::Error>
where
    CydDevice: Cyd,
{
    let mut last_sample_duration: Option<Duration> = None;
    loop {
        for (sample_index, params) in MOTION.samples().enumerate() {
            let started = Instant::now();
            let mut cyd_frame = cyd.full_frame_mut();
            BACKGROUND_BITMAP
                .draw(&mut cyd_frame)
                .expect("drawing the background bitmap into a CYD frame cannot fail");
            for draw_item in LINKAGE.draw_items(&params) {
                draw_item.project(&PROJECTION).draw(&mut cyd_frame);
            }

            // todo000 review this
            let mut status = heapless::String::<64>::new();
            if let Some(last_sample_duration) = last_sample_duration {
                let elapsed_secs = last_sample_duration.as_micros() as f32 * 1e-6_f32;
                let fps = (1.0_f32 / elapsed_secs).max(0.1);
                let slomo = 120.0_f32 / fps;
                core::fmt::write(
                    &mut status,
                    format_args!(
                        "{}/{}  fps {:.1}  slow {:.1}x",
                        sample_index + 1,
                        MOTION.sample_count(),
                        fps,
                        slomo,
                    ),
                )
                .expect("status text fits in 64 bytes");
            } else {
                core::fmt::write(
                    &mut status,
                    format_args!(
                        "{}/{}  fps --.-  slow --.-x",
                        sample_index + 1,
                        MOTION.sample_count()
                    ),
                )
                .expect("status text fits in 64 bytes");
            }
            cyd_frame.write_text(&status);
            // The frame boundary: immediate on the MCU, next-animation-frame on WASM.
            cyd_frame.flush().await?;
            last_sample_duration = Some(Instant::now() - started);
            // todo000 wasm is so fast, might want code to stop faster than 120fps.
        }
    }
}
