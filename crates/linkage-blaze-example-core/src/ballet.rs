//! The generic "ballet" example: free-runs a motion-captured pirouette across
//! the full screen, with an fps / slow-motion status line.
//!
//! Like [`skeleton_clock`](crate::skeleton_clock), this is the device-agnostic
//! core: generic over a [`CydSurface`] so the same code drives a real esp32 CYD
//! and (later) a WASM-simulated one. The platform shim constructs the concrete
//! device and calls [`ballet`].

use core::{convert::Infallible, fmt::Write};

use embassy_time::{Duration, Instant};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point},
    text::{Baseline, Text},
};
use linkage_blaze_core::{
    LinkageFixed, Projection, Rgb888, WebColors, bvh_motion, bvh_parse::BvhMotion, linkage,
    linkage_fixed,
};

use linkage_blaze_cyd_core::{Cyd, CydFrame};

// todo00 audit the existing numeric color backlog and add approximate color-name comments.
// todo000 every numeric color should have a comment telling what it is. (and named colors are better)
/// Device default background color the platform shim should construct its `Cyd`
/// with (also used to clear every frame).
pub const BACKGROUND: Rgb888 = Rgb888::new(10, 28, 36); // very dark blue-green
pub const FOREGROUND: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;
const FIGURE: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;

// todo000 these could be OK, but there are a lot of them. Can't some be done via math?
const CENTER_X: i32 = 84;
const BASELINE_Y: i32 = 300;
const SCALE: f32 = 1.575;

#[allow(long_running_const_eval)]
// This can take ~8 seconds to compile and will generate a warning.
const MOTION: BvhMotion<132, 592> = bvh_motion!("../../linkage-blaze-mocap/samples/pirouette.bvh");
const LINKAGE0: LinkageFixed<{ MOTION.dof() }, 6, 538> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");
const LINKAGE: LinkageFixed<{ MOTION.dof() }, 6, 540> = LinkageFixed::<0, 0, 3>::start()
    .pen_color(FIGURE)
    .pen_width(3.2)
    .combine(LINKAGE0);

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
    let linkage = LINKAGE.view();
    let text565 = Rgb565::from(FOREGROUND);
    let mut last_sample_duration = None;
    loop {
        for (sample_index, params) in MOTION.samples().enumerate() {
            let started = Instant::now();
            let mut cyd_frame = cyd.full_frame_mut();
            for draw_item in linkage.draw_items(&params) {
                draw_item.project(&PROJECTION).draw(&mut cyd_frame);
            }

            // todo000 review this
            draw_status(&mut cyd_frame, text565, sample_index, last_sample_duration);
            // The frame boundary: immediate on the MCU, next-animation-frame on WASM.
            cyd_frame.flush().await?;
            last_sample_duration = Some(Instant::now() - started);
            // todo000 wasm is so fast, might want code to stop faster than 120fps.
        }
    }
}

fn draw_status<D>(
    draw_target: &mut D,
    text565: Rgb565,
    sample_index: usize,
    last_sample_duration: Option<Duration>,
) where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    let mut status = heapless::String::<64>::new();
    if let Some(last_sample_duration) = last_sample_duration {
        let elapsed_secs = last_sample_duration.as_micros() as f32 * 1e-6_f32;
        let fps = (1.0_f32 / elapsed_secs).max(0.1);
        let slomo = 120.0_f32 / fps;
        Write::write_fmt(
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
        Write::write_fmt(
            &mut status,
            format_args!(
                "{}/{}  fps --.-  slow --.-x",
                sample_index + 1,
                MOTION.sample_count()
            ),
        )
        .expect("status text fits in 64 bytes");
    }

    Text::with_baseline(
        status.as_str(),
        Point::new(0, 0),
        MonoTextStyle::new(&FONT_6X10, text565),
        Baseline::Top,
    )
    .draw(draw_target)
    .expect("drawing to an Infallible target cannot fail");
}
