//! The generic "skeleton clock" example: a motion-captured figure whose limbs
//! act as clock hands, with hour/minute placards hanging from its hands.
//!
//! This is the device-agnostic core — modeled on device-envoy's
//! `conway_with_led2d_ir_kepler`. It is generic over a [`CydSurface`] display
//! and a [`ClockSync`] time source, so the same code runs on a real esp32 CYD
//! and (later) a WASM-simulated one. Platform shims construct the concrete
//! devices, handle WiFi/clock setup, and then call [`skeleton_clock`].

use core::convert::Infallible;

use device_envoy_core::clock_sync::{ClockSync, h12_m_s};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_6X10, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Primitive, Size},
    primitives::{Circle, Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use linkage_blaze_core::{
    CameraProjection, LinkageFixed, MarkError, Pose, Projection, Rgb888, WebColors, linkage,
    linkage_fixed, to_point,
};
use log::info;
use time::OffsetDateTime;

use linkage_blaze_cyd_core::{
    Cyd, CydFrame, Orientation, TranslatedDrawTarget,
    tiling::{TileGrid, max_u32},
};

// ── Palette ──────────────────────────────────────────────────────────────────

/// Device default background color the platform shim should construct its `Cyd`
/// with (also used to clear every frame).
pub const BACKGROUND: Rgb888 = Rgb888::CSS_MIDNIGHT_BLUE; // deep night blue (25, 25, 112)
const FIGURE: Rgb888 = Rgb888::CSS_WHEAT; // warm pale bone-like tan (245, 222, 179)
/// Device default foreground/text color the platform shim should construct its
/// `Cyd` with.
pub const FOREGROUND: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE; // muted cool text (176, 196, 222)
const PLACARD_FILL: Rgb888 = Rgb888::CSS_WHEAT; // light sign face
const PLACARD_TEXT: Rgb888 = BACKGROUND; // dark text on the light sign face
const DIAL_FILL: Rgb888 = Rgb888::new(13, 49, 67); // dark blue-green dial disk
const DIAL_STROKE: Rgb888 = Rgb888::CSS_DARK_SLATE_GRAY; // muted teal-gray dial edge

// ── Screen / tile layout ─────────────────────────────────────────────────────

/// Screen orientation this example's layout assumes; the platform shim MUST
/// construct its `Cyd` with this orientation so the layout constants match.
pub const ORIENTATION: Orientation = Orientation::Portrait;

/// Font for the top WiFi/time texts; every platform shim MUST construct its
/// `Cyd` with this font so the simulator and the real device match (and so the
/// time band's character-width math below stays correct). Double-height (10×20).
pub const TOP_FONT: MonoFont<'static> = FONT_10X20;

/// Region (size) of the WiFi-status band; the shim draws WiFi messages here.
/// Sized for the double-height top font (10×20): "WiFi: OK" is 8×10 = 80 px wide
/// and the band is 44 px tall, leaving the rest of the row for the time text.
pub const WIFI_STATUS_SIZE: Size = Size::new(96, 44);
/// Top-left of the WiFi-status band.
pub const WIFI_STATUS_POINT: Point = Point::new(0, 0);

const TIME_POINT: Point = Point::new(WIFI_STATUS_SIZE.width as i32, WIFI_STATUS_POINT.y);
const TIME_SIZE: Size = Size::new(
    ORIENTATION.width() - TIME_POINT.x as u32,
    WIFI_STATUS_SIZE.height,
);

const BELOW_WIFI_TIME: u32 = max_u32(
    WIFI_STATUS_POINT.y as u32 + WIFI_STATUS_SIZE.height,
    TIME_POINT.y as u32 + TIME_SIZE.height,
);

/// The 3×3 grid of tiles the figure is rendered into, below the status band.
/// The shim uses this to size its shared pixel buffer.
pub const FIGURE_TILES: TileGrid = TileGrid::new(
    Point::new(0, BELOW_WIFI_TIME as i32),
    Size::new(ORIENTATION.width(), ORIENTATION.height() - BELOW_WIFI_TIME),
    3,
    3,
);

// ── Projection ───────────────────────────────────────────────────────────────

//todo000 review projections.
const PROJECTION: CameraProjection = CameraProjection::neg_x_ortho(120.0, 300.0, 1.25);

// ── Linkage constants ─────────────────────────────────────────────────────────

// Load the raw motion-capture linkage.
const LINKAGE0: LinkageFixed<132, 6, 600> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");

// Add the drawing style.
const LINKAGE1: LinkageFixed<132, 6, 600> = LinkageFixed::<0, 0, 3>::start()
    .pen_width(3.5)
    .pen_color(FIGURE)
    .combine(LINKAGE0);

// Keep only the three clock-driven parameters, then optimize the fixed linkage.
const LINKAGE: LinkageFixed<3, 6, 400> = LINKAGE1
    .freeze_param_name::<131>("l_shin_yrotation", 57.6)
    .retain_param_names(&["head_yrotation", "l_shldr_zrotation", "r_shldr_zrotation"])
    .compact::<400>();

// ── Errors ────────────────────────────────────────────────────────────────────

/// Error from the generic skeleton-clock loop, generic over the surface's flush
/// error `F`.
#[derive(Debug)]
pub enum SkeletonClockError<F> {
    /// Flushing a frame to the display failed.
    Flush(F),
    /// A required figure mark was not found.
    Mark(MarkError),
}

impl<F> From<MarkError> for SkeletonClockError<F> {
    fn from(error: MarkError) -> Self {
        Self::Mark(error)
    }
}

// ── Generic entry point ────────────────────────────────────────────────────────

/// Run the skeleton-clock render loop forever, driven by `clock_sync` ticks and
/// drawn onto `cyd`.
pub async fn skeleton_clock<S, C>(
    cyd: &mut S,
    clock_sync: &C,
) -> Result<Infallible, SkeletonClockError<S::Error>>
where
    S: Cyd,
    C: ClockSync,
{
    let linkage_view = LINKAGE.view();

    loop {
        let tick = clock_sync.wait_for_tick().await;
        let local_time = &tick.local_time;
        info!("tick {}", text_24h(local_time));

        let params = linkage_params(local_time);
        let time_text = text_12h(local_time);

        cyd.frame_mut(TIME_SIZE)
            .write_text(&time_text)
            .flush_at(TIME_POINT)
            .await
            .map_err(SkeletonClockError::Flush)?;

        // Shared linkage rendering path, tiled for CYD.

        for tile in FIGURE_TILES.tiles() {
            let mut tile_frame = cyd.frame_mut(tile.size);

            // Skeleton-clock-specific background overlay.
            {
                // todo000 understand TranslatedDrawTarget
                let mut target = TranslatedDrawTarget::new(&mut tile_frame, tile.top_left);
                draw_dial(&mut target);
            }

            let mut draw_items = linkage_view.draw_items(&params);
            for draw_item in &mut draw_items {
                draw_item
                    .project(&PROJECTION)
                    // todo00 really understand draw_offset
                    .draw_offset(&mut tile_frame, tile.top_left);
            }

            // todo000 explain that after we go through all the items we inspect the poses of the marks.
            // Skeleton-clock-specific foreground overlay: placards hang from hand marks.
            let right_hand_pose = draw_items.pose_by_mark_name("rMid2")?;
            let left_hand_pose = draw_items.pose_by_mark_name("lMid2")?;
            let (hour_12, minute, _) = h12_m_s(local_time);
            let mut target = TranslatedDrawTarget::new(&mut tile_frame, tile.top_left);
            draw_hanging_placard(
                &mut target,
                pose_to_point(left_hand_pose),
                hour_12 as u32,
                "H",
            );
            draw_hanging_placard(
                &mut target,
                pose_to_point(right_hand_pose),
                minute as u32,
                "M",
            );

            tile_frame
                .flush_at(tile.top_left)
                .await
                .map_err(SkeletonClockError::Flush)?;
        }
    }
}

// ── Clock time ────────────────────────────────────────────────────────────────

/// Format a 12-hour clock string with AM/PM. The hour is space-padded to two
/// characters (e.g. " 5:04:32 PM" or "12:04:32 PM") and the whole string is
/// right-justified within the time band (144 px ÷ 10 px/char = 14 chars) so it
/// sits against the band's right edge.
fn text_12h(local_time: &OffsetDateTime) -> heapless::String<16> {
    let (hour_12, minute, second) = h12_m_s(local_time);
    let suffix = if local_time.hour() % 24 < 12 {
        "AM"
    } else {
        "PM"
    };
    // Build the time, then right-justify the whole thing to the band width so it
    // hugs the right edge (leading spaces push it over; the hour stays padded).
    let mut inner = heapless::String::<16>::new();
    core::fmt::write(
        &mut inner,
        format_args!("{hour_12:>2}:{minute:02}:{second:02} {suffix}"),
    )
    .expect("clock string fits in 16 bytes");
    let mut text = heapless::String::new();
    core::fmt::write(&mut text, format_args!("{inner:>14}"))
        .expect("clock string fits in 16 bytes");
    text
}

/// Format a 24-hour `HH:MM:SS` clock string.
fn text_24h(local_time: &OffsetDateTime) -> heapless::String<9> {
    let mut text = heapless::String::new();
    core::fmt::write(
        &mut text,
        format_args!(
            "{:02}:{:02}:{:02}",
            local_time.hour(),
            local_time.minute(),
            local_time.second()
        ),
    )
    .expect("clock string fits in 9 bytes");
    text
}

// todo000 seems overly complex.
fn linkage_params(local_time: &OffsetDateTime) -> [f32; 3] {
    const CLOCK_HAND_PARAM_TURN: f32 = 0.25;
    const EYES_FORWARD_PARAM: f32 = 0.5;
    const RIGHT_ARM_12_PARAM: f32 = 0.4375;
    const LEFT_ARM_12_PARAM: f32 = 0.5625;
    let second_phase = local_time.second() as f32 / 60.0;
    let minute_phase = (local_time.minute() as f32 + second_phase) / 60.0;
    let hour_phase = ((local_time.hour() % 12) as f32 + minute_phase) / 12.0;
    let signed_hour_phase = signed_clock_phase(hour_phase);
    [
        wrap_unit(EYES_FORWARD_PARAM + second_phase * CLOCK_HAND_PARAM_TURN),
        wrap_unit(RIGHT_ARM_12_PARAM + minute_phase * CLOCK_HAND_PARAM_TURN),
        wrap_unit(LEFT_ARM_12_PARAM + signed_hour_phase * CLOCK_HAND_PARAM_TURN),
    ]
}

fn signed_clock_phase(phase: f32) -> f32 {
    if phase > 0.5 { phase - 1.0 } else { phase }
}

fn wrap_unit(value: f32) -> f32 {
    let mut value = value;
    while value >= 1.0 {
        value -= 1.0;
    }
    while value < 0.0 {
        value += 1.0;
    }
    value
}

// ── Skeleton-clock-specific overlay drawing ──────────────────────────────────

// All overlay drawing happens against a `DrawTarget` whose coordinates are in
// physical-screen space; a `TranslatedDrawTarget` subtracts the tile origin so
// these functions never need to know they are rendering into a tile.

/// Draw a short string centered (both axes) on `center`.
fn draw_centered_text<D>(
    target: &mut D,
    text: &str,
    center: Point,
    font: &'static MonoFont<'static>,
    color: Rgb565,
) where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    let text_style = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Middle)
        .build();
    Text::with_text_style(text, center, MonoTextStyle::new(font, color), text_style)
        .draw(target)
        .expect("drawing to an Infallible target cannot fail");
}

// todo000 this is hard to see. Make it more visible or remove it
/// Draw a soft clock face behind the figure plus the 12 / 3 / 6 / 9 markers.
fn draw_dial<D>(target: &mut D)
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    const DIAL_CENTER_SCREEN: Point = Point::new(120, 178);
    const DIAL_RADIUS: i32 = 112;
    const DIAL_NUMBER_RADIUS_X: i32 = 96;
    const DIAL_NUMBER_RADIUS_Y: i32 = 102;

    let dial_fill = Rgb565::from(DIAL_FILL);
    let dial_stroke = Rgb565::from(DIAL_STROKE);
    let dial_text = Rgb565::from(FIGURE);

    Circle::new(
        Point::new(
            DIAL_CENTER_SCREEN.x - DIAL_RADIUS,
            DIAL_CENTER_SCREEN.y - DIAL_RADIUS,
        ),
        (DIAL_RADIUS * 2) as u32,
    )
    .into_styled(
        PrimitiveStyleBuilder::new()
            .fill_color(dial_fill)
            .stroke_color(dial_stroke)
            .stroke_width(2)
            .build(),
    )
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");

    // Cardinal ticks help the pale numerals read as a clock face, even on the
    // small CYD screen.
    let tick_style = PrimitiveStyle::with_stroke(dial_stroke, 3);
    Line::new(
        Point::new(
            DIAL_CENTER_SCREEN.x,
            DIAL_CENTER_SCREEN.y - DIAL_RADIUS + 10,
        ),
        Point::new(
            DIAL_CENTER_SCREEN.x,
            DIAL_CENTER_SCREEN.y - DIAL_RADIUS + 28,
        ),
    )
    .into_styled(tick_style)
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");
    Line::new(
        Point::new(
            DIAL_CENTER_SCREEN.x + DIAL_RADIUS - 28,
            DIAL_CENTER_SCREEN.y,
        ),
        Point::new(
            DIAL_CENTER_SCREEN.x + DIAL_RADIUS - 10,
            DIAL_CENTER_SCREEN.y,
        ),
    )
    .into_styled(tick_style)
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");
    Line::new(
        Point::new(
            DIAL_CENTER_SCREEN.x,
            DIAL_CENTER_SCREEN.y + DIAL_RADIUS - 28,
        ),
        Point::new(
            DIAL_CENTER_SCREEN.x,
            DIAL_CENTER_SCREEN.y + DIAL_RADIUS - 10,
        ),
    )
    .into_styled(tick_style)
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");
    Line::new(
        Point::new(
            DIAL_CENTER_SCREEN.x - DIAL_RADIUS + 10,
            DIAL_CENTER_SCREEN.y,
        ),
        Point::new(
            DIAL_CENTER_SCREEN.x - DIAL_RADIUS + 28,
            DIAL_CENTER_SCREEN.y,
        ),
    )
    .into_styled(tick_style)
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");

    draw_centered_text(
        target,
        "12",
        Point::new(
            DIAL_CENTER_SCREEN.x,
            DIAL_CENTER_SCREEN.y - DIAL_NUMBER_RADIUS_Y,
        ),
        &FONT_10X20,
        dial_text,
    );
    draw_centered_text(
        target,
        "3",
        Point::new(
            DIAL_CENTER_SCREEN.x + DIAL_NUMBER_RADIUS_X,
            DIAL_CENTER_SCREEN.y,
        ),
        &FONT_10X20,
        dial_text,
    );
    draw_centered_text(
        target,
        "6",
        Point::new(
            DIAL_CENTER_SCREEN.x,
            DIAL_CENTER_SCREEN.y + DIAL_NUMBER_RADIUS_Y,
        ),
        &FONT_10X20,
        dial_text,
    );
    draw_centered_text(
        target,
        "9",
        Point::new(
            DIAL_CENTER_SCREEN.x - DIAL_NUMBER_RADIUS_X,
            DIAL_CENTER_SCREEN.y,
        ),
        &FONT_10X20,
        dial_text,
    );
}

/// Draw a hanging number sign anchored at `anchor` (a hand mark in skeleton-clock
/// coordinates). The sign is fixed size and shows a two-digit value plus a
/// small label (`"H"` or `"M"`) so the hanging hour and minute are distinguishable.
fn draw_hanging_placard<D>(target: &mut D, anchor: Point, number: u32, label: &str)
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    const PLACARD_W: i32 = 38;
    const PLACARD_H: i32 = 28;
    const PLACARD_BORDER_PX: i32 = 2;
    const HANGER_PX: i32 = 2;
    const HANGER_HOOK: i32 = 7;
    const HANGER_TRIANGLE: i32 = 18;

    let placard_border = Rgb565::from(FIGURE);
    let placard_text = Rgb565::from(PLACARD_TEXT);
    let placard_fill = Rgb565::from(PLACARD_FILL);

    let card_left = anchor.x - PLACARD_W / 2;
    let card_top = anchor.y + HANGER_HOOK + HANGER_TRIANGLE;
    let card_right = card_left + PLACARD_W;
    let apex = Point::new(anchor.x, anchor.y + HANGER_HOOK);

    let hanger_style = PrimitiveStyle::with_stroke(placard_border, HANGER_PX as u32);
    Line::new(anchor, apex)
        .into_styled(hanger_style)
        .draw(target)
        .expect("drawing to an Infallible target cannot fail");
    Line::new(apex, Point::new(card_left, card_top))
        .into_styled(hanger_style)
        .draw(target)
        .expect("drawing to an Infallible target cannot fail");
    Line::new(apex, Point::new(card_right, card_top))
        .into_styled(hanger_style)
        .draw(target)
        .expect("drawing to an Infallible target cannot fail");

    let card_style = PrimitiveStyleBuilder::new()
        .fill_color(placard_fill)
        .stroke_color(placard_border)
        .stroke_width(PLACARD_BORDER_PX as u32)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();
    Rectangle::new(
        Point::new(card_left, card_top),
        Size::new(PLACARD_W as u32, PLACARD_H as u32),
    )
    .into_styled(card_style)
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");

    // A center divider makes the small H/M label read as part of the sign
    // rather than as a third digit.
    Line::new(
        Point::new(card_left + 4, card_top + 18),
        Point::new(card_right - 4, card_top + 18),
    )
    .into_styled(PrimitiveStyle::with_stroke(placard_text, 1))
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");

    let mut value_text = heapless::String::<4>::new();
    core::fmt::write(&mut value_text, format_args!("{:02}", number % 100))
        .expect("two-digit placard value fits in 4 bytes");

    draw_centered_text(
        target,
        &value_text,
        Point::new(card_left + PLACARD_W / 2, card_top + 10),
        &FONT_10X20,
        placard_text,
    );
    draw_centered_text(
        target,
        label,
        Point::new(card_left + PLACARD_W / 2, card_top + 23),
        &FONT_6X10,
        placard_text,
    );
}

// todo0000 shouldn't be needed.
fn pose_to_point(pose: Pose) -> Point {
    to_point(PROJECTION.project_pos(pose))
}
