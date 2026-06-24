#![no_std]
#![no_main]
#![cfg_attr(feature = "const-parse", allow(long_running_const_eval))]

use core::{convert::Infallible, fmt::Write};

use device_envoy_esp::init_and_start;
use embassy_executor::Spawner;
use embedded_graphics::{
    Drawable,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point},
    text::{Baseline, Text},
};
use esp_backtrace as _;
use esp_hal::time::{Duration, Instant};
// Feature `const-parse`: parse pirouette.bvh at compile time via const fn (~8 s).
// Default (no feature): include a pre-generated snapshot; regenerate with `just generate-ballet`.

const BALLET_DOF: usize = 132;
const BALLET_FRAME_COUNT: usize = 592;

// ── const-parse path ─────────────────────────────────────────────────────────
//
// Three compile-time products, each independently named so they can be
// inspected or reused:
//   RAW_BALLET_FRAMES          — flat f32 values straight from the BVH file.
//   BALLET_CHANNEL_IS_POSITION — which channels are position vs rotation.
//   BALLET_FRAMES              — normalized [0, 1] Linkage parameters.
//
// Normalization ranges are Linkage Blaze parameter-encoding policy, not BVH
// facts. They live in BvhNormalizePolicy::LINKAGE_BLAZE inside bvh_parse.rs.
// Values within ±0.01 of 0.5 after normalization snap to exactly 0.5,
// matching the runtime behavior of linkage-blaze-mocap's `snap_centered_default`.
//
// todo0000 article: Consider making BvhNormalizePolicy an explicit caller
// argument so the const parser is clearly separated from Linkage Blaze's
// parameter-range policy when used in other projects.

#[cfg(feature = "const-parse")]
const BVH_BYTES: &[u8] = include_bytes!("../../linkage-blaze-mocap/samples/pirouette.bvh");

#[cfg(feature = "const-parse")]
#[allow(long_running_const_eval)]
const RAW_BALLET_FRAMES: [[f32; BALLET_DOF]; BALLET_FRAME_COUNT] =
    linkage_blaze_ballet::bvh_parse::parse_bvh_motion_section::<BALLET_DOF, BALLET_FRAME_COUNT>(
        BVH_BYTES,
    );

#[cfg(feature = "const-parse")]
const BALLET_CHANNEL_IS_POSITION: [bool; BALLET_DOF] =
    linkage_blaze_ballet::bvh_parse::parse_bvh_channel_is_position::<BALLET_DOF>(BVH_BYTES);

#[cfg(feature = "const-parse")]
#[allow(long_running_const_eval)]
const BALLET_FRAMES: [[f32; BALLET_DOF]; BALLET_FRAME_COUNT] =
    linkage_blaze_ballet::bvh_parse::normalize_bvh_motion::<BALLET_DOF, BALLET_FRAME_COUNT>(
        RAW_BALLET_FRAMES,
        BALLET_CHANNEL_IS_POSITION,
        linkage_blaze_ballet::bvh_parse::BvhNormalizePolicy::LINKAGE_BLAZE,
    );

// ── pre-generated path ───────────────────────────────────────────────────────

#[cfg(not(feature = "const-parse"))]
include!("ballet_frames_precomputed.rs");

use linkage_blaze_ballet::ballet_render::{
    BACKGROUND, BALLET, FIGURE_COLOR, FIGURE_STROKE_PX, TEXT, draw_filled_circle, draw_segment,
    pose_to_point,
};
use linkage_blaze_core::DrawItem;
use linkage_blaze_cyd::{Cyd, CydDisplayConfig, CydStatic, PixelBufferFull};
use log::info;

// todo000 I'm not happy with all this noise.

// todo000 this seems unmotivated.
const SOURCE_FPS_X10: u32 = 1200;

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    Cyd(linkage_blaze_cyd::CydError),
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    info!("Starting CYD ballet loop");

    static CYD_STATIC: CydStatic<PixelBufferFull> = Cyd::new_static();
    let mut cyd = Cyd::new_display_only(
        &CYD_STATIC,
        p.SPI2,
        p.GPIO14,
        p.GPIO13,
        p.GPIO12,
        p.GPIO15,
        p.GPIO2,
        p.GPIO4,
        p.GPIO21,
        // todo000 are there 4 orientations?
        CydDisplayConfig::PORTRAIT,
    )?;
    let background565 = Cyd::rgb565(BACKGROUND);
    let text565 = Cyd::rgb565(TEXT);
    // todo000 agent, remember to never delete my todo's.
    info!("CYD display initialized");

    let linkage = BALLET.view();
    let mut last_frame_duration = None;
    loop {
        info!("starting ballet cycle");
        // todo000 We don't expect BALLET_DOF to be a free-floating constant.
        for (frame_index, params) in BALLET_FRAMES.iter().enumerate() {
            let started = Instant::now();
            // todo000 pull this out of the loops?
            let mut cyd_frame = cyd.full_frame_mut();
            cyd_frame.clear(background565);
            for draw_item in &mut linkage.draw_items(params) {
                match draw_item {
                    // todo understand pose_to_point
                    DrawItem::Stroke(stroke) => {
                        draw_segment(
                            &mut cyd_frame,
                            pose_to_point(stroke.start()),
                            pose_to_point(stroke.end()),
                            FIGURE_COLOR,
                            FIGURE_STROKE_PX,
                        );
                    }
                    // todo00 Disk or filled_circle?
                    DrawItem::Disk(disk) => {
                        draw_filled_circle(
                            &mut cyd_frame,
                            pose_to_point(disk.pose()),
                            disk.radius(),
                            FIGURE_COLOR,
                        );
                    }
                    DrawItem::Sphere(sphere) => {
                        draw_filled_circle(
                            &mut cyd_frame,
                            pose_to_point(sphere.pose()),
                            sphere.radius(),
                            FIGURE_COLOR,
                        );
                    }
                }
            }

            // todo000 review this
            draw_status(&mut cyd_frame, text565, frame_index, last_frame_duration);
            cyd_frame.flush()?;
            last_frame_duration = Some(Instant::now() - started);
        }
    }
}
// todo000 still need to review other files in the project.

fn draw_status<D>(
    draw_target: &mut D,
    text565: Rgb565,
    frame_index: usize,
    last_frame_duration: Option<Duration>,
) where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    let mut status = heapless::String::<64>::new();
    if let Some(last_frame_duration) = last_frame_duration {
        let elapsed_ms = last_frame_duration.as_millis().max(1);
        let fps_x10 = (10_000 / elapsed_ms) as u32;
        let slomo_x10 = if fps_x10 == 0 {
            0
        } else {
            (SOURCE_FPS_X10 * 10 + fps_x10 / 2) / fps_x10
        };
        Write::write_fmt(
            &mut status,
            format_args!(
                "{}/{}  fps {}.{}  slow {}.{}x",
                frame_index + 1,
                BALLET_FRAME_COUNT,
                fps_x10 / 10,
                fps_x10 % 10,
                slomo_x10 / 10,
                slomo_x10 % 10
            ),
        )
        .ok();
    } else {
        Write::write_fmt(
            &mut status,
            format_args!(
                "{}/{}  fps --.-  slow --.-x",
                frame_index + 1,
                BALLET_FRAME_COUNT
            ),
        )
        .ok();
    }

    Text::with_baseline(
        status.as_str(),
        Point::new(0, 0),
        MonoTextStyle::new(&FONT_6X10, text565),
        Baseline::Top,
    )
    .draw(draw_target)
    .ok();
}
