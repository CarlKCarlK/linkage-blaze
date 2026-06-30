//! The generic "clock" example: an analog clock face whose hands are driven by a
//! tiny [`linkage`](linkage_blaze_core::linkage), with a digital time read-out
//! above it.
//!
//! This mirrors [`skeleton_clock`](crate::skeleton_clock): the device-agnostic
//! game loop lives here (written against the [`Cyd`] and
//! [`ClockSync`] traits) while the esp32 binary
//! (`linkage-blaze-classic/examples/clock/main.rs`) only wires up the hardware
//! and hands a `&mut CydEsp` to [`clock`].

use core::convert::Infallible;

use device_envoy_core::clock_sync::{ClockSync, h12_m_s};
use embedded_graphics::{
    mono_font::{MonoFont, ascii::FONT_10X20},
    pixelcolor::{Rgb888, WebColors},
    prelude::{Point, Size},
};
use linkage_blaze_cyd_core::{Cyd, CydFrame, Orientation, tiling::Region};
use log::info;
use time::OffsetDateTime;

// ── Palette ──────────────────────────────────────────────────────────────────

/// The clock face is drawn on this background; the esp32 binary uses the same
/// color as the device per-frame clear color.
pub const BACKGROUND: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE; // pale parchment
pub const FOREGROUND: Rgb888 = Rgb888::CSS_NAVY; // dark blue clock text

// ── Screen / region layout ─────────────────────────────────────────────────────

pub const ORIENTATION: Orientation = Orientation::Landscape;
pub const TOP_FONT: MonoFont<'static> = FONT_10X20;

/// WiFi/status line, top-left.
pub const WIFI_STATUS_REGION: Region = Region::new(Point::new(6, 6), Size::new(180, 22));
/// Digital time read-out, below the status line.
pub const TIME_REGION: Region = Region::new(Point::new(6, 34), Size::new(220, 22));

// ── Main function ────────────────────────────────────────────────────────

/// Run the clock render loop forever, driven by `clock_sync` ticks and drawn
/// onto `cyd`.
pub async fn clock<CydDevice, ClockSyncDevice>(
    cyd: &mut CydDevice,
    clock_sync: &ClockSyncDevice,
) -> Result<Infallible, Error<CydDevice::Error>>
where
    CydDevice: Cyd,
    ClockSyncDevice: ClockSync,
{
    loop {
        // Wait for a tick and get the time.
        let tick = clock_sync.wait_for_tick().await;
        let local_time = &tick.local_time;
        let clock_time = ClockTime::from_time(local_time);
        info!("tick {}", clock_time.as_str());

        // Write the digital time.
        cyd.frame_mut(TIME_REGION)
            .write_text(clock_time.as_str())
            .flush()
            .await
            .map_err(Error::Flush)?;

        // TODO00000 Port the analog clock-hand rendering to the generic `Cyd`
        // trait. The original esp32 `CydClockDisplay::show_clock` (in the old
        // `examples/clock/display.rs`) is commented out below because it depends
        // on rendering methods that exist only on the concrete `CydEsp`, not on
        // the device-agnostic `Cyd`/`CydFrame` traits:
        //
        //   * `CydEsp::draw_primitives` / the `DrawPrimitive`, `Ellipse`,
        //     `LineSegment` batch-draw types,
        //   * `CydEsp::flush_at`, `CydEsp::fill_rectangle`, and the scaled-glyph
        //     `PixelBuffer`/`RegionView` workspace.
        //
        // It also uses a bespoke "model +X = up, +Y = left" projection
        // (`project_x`/`project_y`/`project_dir`) that the generic
        // `linkage_blaze_core::Projection` cannot yet express: `front_orthographic`
        // maps world Z (not world X) to screen Y, and there is no public
        // constructor for a custom rotation basis (see the `todo0000` on
        // `Projection`). Once a custom-rotation `Projection` constructor lands,
        // render the linkage the way `skeleton_clock` does: iterate
        // `CLOCK_HANDS.view().draw_items(&clock_time.params())`, `.project(...)`
        // each into a `ProjectedDrawItem`, and `.draw(&mut frame)` onto a frame
        // borrowed over the clock-face `Region` (every `CydFrame` is a
        // `PixelTarget`).
        /*
        const CLOCK_TOP_LEFT: Point = Point::new(80, 80);
        const CLOCK_CENTER_X: i32 = 80;
        const CLOCK_CENTER_Y: i32 = 80;
        const HAND_SCALE: f32 = 1.0;
        const CLOCK_HANDS: LinkageFixed<2, 2, 48> = linkage_fixed!("clock.lb.rs");

        // Map the time onto the linkage's two normalized params: the hands
        // (hour, derived from h/m/s) and the slowly spinning face.
        // let second = seconds as f32 / 60.0;
        // let minute = (minutes as f32 + second) / 60.0;
        // let hour = ((hours % 12) as f32 + minute) / 12.0;
        // let face_spin = (((seconds % 20) as f32) / 20.0 + 0.5) % 1.0;
        // let params = [hour, face_spin];
        for draw_item in CLOCK_HANDS.view().draw_items(&params) {
            let prim = match draw_item {
                DrawItem::Stroke(stroke) => {
                    let project = |pose: Pose| {
                        let position = pose.position();
                        clock_point(Point::new(
                            CLOCK_CENTER_X + project_x(position, HAND_SCALE),
                            CLOCK_CENTER_Y + project_y(position, HAND_SCALE),
                        ))
                    };
                    let start = project(stroke.start());
                    let end = project(stroke.end());
                    if start != end {
                        DrawPrimitive::LineSegment(LineSegment {
                            start,
                            end,
                            width: clock_width_pixels(stroke.width()),
                            color: CydEsp::rgb565(stroke.color()),
                        })
                    } else {
                        continue;
                    }
                }
                DrawItem::Disk(disk) => DrawPrimitive::Ellipse(disk_to_ellipse(disk)),
                DrawItem::Sphere(sphere) => DrawPrimitive::Ellipse(sphere_to_ellipse(sphere)),
            };
            // ... collect into a heapless::Vec<DrawPrimitive, 16> and
            // cyd.draw_primitives(CLOCK_BOUNDS, CydEsp::rgb565(BACKGROUND), &primitives)?;
        }

        // Convention: model +X = up, +Y = left.
        // screen_x = -model_y, screen_y = -model_x.
        // fn project_x(pos: Vec3, scale: f32) -> i32 { -(pos[1] * scale) as i32 }
        // fn project_y(pos: Vec3, scale: f32) -> i32 { -(pos[0] * scale) as i32 }
        // fn project_dir(world_x: f32, world_y: f32, r: f32) -> (f32, f32) {
        //     (-world_y * r, -world_x * r)
        // }
        */
    }
}

// ── Clock time ──────────────────────────────────────────────────────────────────

/// A formatted snapshot of the wall-clock time.
///
/// Once the analog clock-hand rendering is ported (see the `TODO00000` in
/// [`clock`]), this should also carry the raw hour/minute/second so it can drive
/// the linkage params.
pub struct ClockTime {
    text: heapless::String<16>,
}

impl ClockTime {
    /// Build a `ClockTime` from `local_time`, formatting a `12:59 PM`-style read-out.
    pub fn from_time(local_time: &OffsetDateTime) -> Self {
        let (hour_12, minute, _second) = h12_m_s(local_time);
        let meridiem = if local_time.hour() % 24 < 12 {
            "AM"
        } else {
            "PM"
        };
        let mut text = heapless::String::new();
        core::fmt::write(&mut text, format_args!("{hour_12}:{minute:02} {meridiem}"))
            .expect("clock string fits in 16 bytes");
        Self { text }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// Error from the generic clock loop, generic over the surface's flush error `F`.
///
/// Only flushing a frame can fail today; it is converted explicitly with
/// `.map_err(Error::Flush)` at the call site (the same flush-error convention as
/// [`skeleton_clock::Error`](crate::skeleton_clock::Error)). More variants will
/// appear once the analog clock-hand rendering is ported and can report linkage
/// errors.
#[derive(Debug)]
pub enum Error<F> {
    /// Flushing a frame to the display failed.
    Flush(F),
}
