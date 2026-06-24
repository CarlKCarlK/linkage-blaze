#![no_std]
#![no_main]

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

use linkage_blaze_core::{
    LinkageFixed, NegXProjection, PixelSurface, Rgb888, WebColors, bvh_motion, bvh_parse::BvhMotion,
    linkage, linkage_fixed, render_draw_items,
};
use linkage_blaze_cyd::{Cyd, CydDisplayConfig, CydStatic, PixelBufferFull};
use log::info;

// todo00 audit the existing numeric color backlog and add approximate color-name comments.
// todo000 every numeric color should have a comment telling what it is. (and named colors are better)
const BACKGROUND: Rgb888 = Rgb888::new(10, 28, 36); // very dark blue-green
const FIGURE_COLOR: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;

// todo000 these could be OK, but there are a lot of them. Can't some be done via math?
const BALLET_CENTER_X: i32 = 84;
const BALLET_BASELINE_Y: i32 = 300;
const BALLET_SCALE: f32 = 1.575;

#[allow(long_running_const_eval)]
const MOTION: BvhMotion<132, 592> = bvh_motion!("../../linkage-blaze-mocap/samples/pirouette.bvh");
const LINKAGE_INNER: LinkageFixed<{ MOTION.dof() }, 6, 538> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");
const LINKAGE: LinkageFixed<{ MOTION.dof() }, 6, 540> = LinkageFixed::<0, 0, 3>::start()
    .pen_color(FIGURE_COLOR)
    .pen_width(3.2)
    .combine(LINKAGE_INNER);

const BALLET_PROJECTION: NegXProjection = NegXProjection {
    center_x: BALLET_CENTER_X as f32,
    baseline_y: BALLET_BASELINE_Y as f32,
    scale: BALLET_SCALE,
};


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

    let linkage = LINKAGE.view();
    let mut params = [0.0f32; MOTION.dof()];
    let mut last_frame_duration = None;
    loop {
        info!("starting ballet cycle");
        for frame_index in 0..MOTION.frame_count() {
            MOTION.frame_into(frame_index, &mut params);
            let started = Instant::now();
            let mut cyd_frame = cyd.full_frame_mut();
            cyd_frame.clear(background565);
            // todo000 proj is too short
            // todo000 a free-floating function?
            // todo000 understand the inputs.
            render_draw_items(
                &BALLET_PROJECTION,
                &mut PixelSurface::new(&mut cyd_frame),
                linkage.draw_items(&params),
            );

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
        let elapsed_secs = last_frame_duration.as_micros() as f32 * 1e-6_f32;
        let fps = (1.0_f32 / elapsed_secs).max(0.1);
        let slomo = 120.0_f32 / fps;
        Write::write_fmt(
            &mut status,
            format_args!(
                "{}/{}  fps {:.1}  slow {:.1}x",
                frame_index + 1,
                MOTION.frame_count(),
                fps,
                slomo,
            ),
        )
        .ok();
    } else {
        Write::write_fmt(
            &mut status,
            format_args!(
                "{}/{}  fps --.-  slow --.-x",
                frame_index + 1,
                MOTION.frame_count()
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
