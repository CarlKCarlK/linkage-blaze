//! The generic "skeleton clock" example: a motion-captured figure whose limbs
//! act as clock hands, with hour/minute placards hanging from its hands.

use core::convert::Infallible;

use device_envoy_core::clock_sync::{ClockSync, h12_m_s};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_7X13, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Size},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use linkage_blaze_core::{
    LinkageFixed, MarkError, PixelTarget, Pose, Projection, Rgb888, linkage, linkage_fixed,
    to_point,
};
use log::info;
use time::OffsetDateTime;

use linkage_blaze_cyd_core::{
    Cyd, CydFrame, Image565, Image565Mask, Orientation, TranslatedDrawTarget, tga565,
    tga565_magenta_mask,
    tiling::{TileGrid, max_u32},
};

// ── Palette ──────────────────────────────────────────────────────────────────

pub const BACKGROUND: Rgb888 = Rgb888::new(13, 13, 11); // near-black warm charcoal (13, 13, 11)
const FIGURE: Rgb888 = Rgb888::new(255, 214, 123); // warm pale gold (255, 214, 123)
pub const FOREGROUND: Rgb888 = Rgb888::new(255, 214, 123); // warm pale gold (255, 214, 123)
const PLACARD_TEXT: Rgb888 = BACKGROUND; // dark text on the light sign face

// ── Linkage ────────────────────────────────────────────────────────────

// Load the motion-capture linkage converted *.bvh -> *.lb.rs.
const LINKAGE0: LinkageFixed<132, 6, 600> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");

// Prepend a linkage drawing style.
const LINKAGE1: LinkageFixed<132, 6, 600> = LinkageFixed::<0, 0, 3>::start()
    .pen_width(3.5)
    .pen_color(FIGURE)
    .combine(LINKAGE0);

// Keep only the three clock-driven parameters, then optimize the fixed linkage.
const LINKAGE: LinkageFixed<3, 6, 400> = LINKAGE1
    // turn the left foot out jauntily.
    .freeze_param_name::<131>("l_shin_yrotation", 57.6)
    .retain_param_names(&["head_yrotation", "l_shldr_zrotation", "r_shldr_zrotation"])
    .compact::<400>();

// ── Projection ───────────────────────────────────────────────────────────────

//todo000 review projections.

// Keep only the
const PROJECTION: Projection = Projection::front_ortho(141.0, 306.0, 1.35);

// ── Background bitmap ──────────────────────────────────────────────────────────

/// Clock-face background bitmap, loaded at compile time.
//todo0000 we need all these numbers?
//todo0000 is tga565! good?
const CLOCK_BACK_BITMAP: Image565<239, 319, { 239 * 319 }> =
    tga565!("../assets/clock_back.small.tga", 239, 319);
const CLOCK_BACK_POINT: Point = Point::new(0, 0);

const HOURS_SIGN: Image565Mask<45, 73, { 45 * 73 }, { (45 * 73 + 7) / 8 }> =
    tga565_magenta_mask!("../assets/hours.small.tga", 45, 73);
const HOURS_SIGN_ANCHOR_X: i32 = 22;
const HOURS_SIGN_VALUE_CENTER: Point = Point::new(22, 50);

const MINUTE_SIGN: Image565Mask<45, 77, { 45 * 77 }, { (45 * 77 + 7) / 8 }> =
    tga565_magenta_mask!("../assets/minute.small.tga", 45, 77);
const MINUTE_SIGN_ANCHOR_X: i32 = 22;
const MINUTE_SIGN_VALUE_CENTER: Point = Point::new(22, 56);

// ── Screen / tile layout ─────────────────────────────────────────────────────

pub const ORIENTATION: Orientation = Orientation::Portrait;
pub const TOP_FONT: MonoFont<'static> = FONT_7X13;
pub const WIFI_STATUS_SIZE: Size = Size::new(155, 14);
pub const WIFI_STATUS_POINT: Point = Point::new(6, 6);
const TIME_SIZE: Size = Size::new(
    ORIENTATION.width() - WIFI_STATUS_SIZE.width,
    WIFI_STATUS_SIZE.height,
);
const TIME_POINT: Point = Point::new(
    WIFI_STATUS_POINT.x + WIFI_STATUS_SIZE.width as i32,
    WIFI_STATUS_POINT.y,
);

const FIGURE_Y: u32 = max_u32(
    WIFI_STATUS_POINT.y as u32 + WIFI_STATUS_SIZE.height,
    TIME_POINT.y as u32 + TIME_SIZE.height,
);
pub const FIGURE_TILES: TileGrid = TileGrid::new(
    Point::new(0, FIGURE_Y as i32),
    Size::new(ORIENTATION.width(), ORIENTATION.height() - FIGURE_Y),
    3,
    3,
);

// ── Main function ────────────────────────────────────────────────────────

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

            // Skeleton-clock-specific background overlay: blit the clock-face
            // bitmap. `tile_frame` is a `PixelTarget` in tile-local coordinates,
            // so a screen point maps to local by subtracting the tile origin;
            // pixels outside the tile are clipped.
            CLOCK_BACK_BITMAP.draw_at(
                &mut tile_frame,
                (
                    CLOCK_BACK_POINT.x - tile.top_left.x,
                    CLOCK_BACK_POINT.y - tile.top_left.y,
                ),
            );

            let mut draw_items = linkage_view.draw_items(&params);
            for draw_item in &mut draw_items {
                draw_item
                    .project(&PROJECTION)
                    // todo00 really understand draw_offset
                    // Shift the figure 2 px toward screen-left by drawing it
                    // relative to an origin nudged 2 px right (local = screen − origin).
                    .draw_offset(
                        &mut tile_frame,
                        Point::new(tile.top_left.x + 2, tile.top_left.y),
                    );
            }

            // todo000 explain that after we go through all the items we inspect the poses of the marks.
            // Skeleton-clock-specific foreground overlay: placards hang from hand marks.
            let right_hand_pose = draw_items.pose_by_mark_name("rMid2")?;
            let left_hand_pose = draw_items.pose_by_mark_name("lMid2")?;
            let (hour_12, minute, _) = h12_m_s(local_time);

            // Hours and minutes signs: each is a bitmap placard (hanger, body and
            // baked-in "H"/"M") anchored under a hand mark, with its two-digit
            // value overlaid. The bitmaps are drawn straight onto the tile (a
            // `PixelTarget`) in tile-local coordinates, the same way the clock-face
            // background is; the values go through a `TranslatedDrawTarget`.
            //
            // Each sign is drawn together with its own value before the next sign,
            // so when two signs overlap a sign occludes the lower sign *and its
            // number* as one unit (otherwise a lower sign's digits would float on
            // top of the upper sign's face). The minute sign is drawn last, so it
            // sits on top.
            let hours_anchor = pose_to_point(left_hand_pose);
            let hours_top_left = Point::new(hours_anchor.x - HOURS_SIGN_ANCHOR_X, hours_anchor.y);
            let minute_anchor = pose_to_point(right_hand_pose);
            let minute_top_left =
                Point::new(minute_anchor.x - MINUTE_SIGN_ANCHOR_X, minute_anchor.y);

            draw_sign(
                &mut tile_frame,
                tile.top_left,
                &HOURS_SIGN,
                hours_top_left,
                HOURS_SIGN_VALUE_CENTER,
                hour_12 as u32,
            );
            draw_sign(
                &mut tile_frame,
                tile.top_left,
                &MINUTE_SIGN,
                minute_top_left,
                MINUTE_SIGN_VALUE_CENTER,
                minute as u32,
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
/// characters (e.g. " 5:04:32 PM" or "12:04:32 PM") so the colon stays aligned,
/// but the string starts at the band's left edge with no leading spaces.
fn text_12h(local_time: &OffsetDateTime) -> heapless::String<24> {
    let (hour_12, minute, second) = h12_m_s(local_time);
    let suffix = if local_time.hour() % 24 < 12 {
        "AM"
    } else {
        "PM"
    };
    // The hour is space-padded to two characters (so " 5:04:32 PM" lines up with
    // "12:04:32 PM"), but the string starts at the left edge with no extra leading
    // spaces.
    let mut text = heapless::String::new();
    core::fmt::write(
        &mut text,
        format_args!("{hour_12:>2}:{minute:02}:{second:02} {suffix}"),
    )
    .expect("clock string fits in 24 bytes");
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

/// Blit a sign bitmap onto `frame` (a tile, `tile_top_left` is its screen
/// origin) and overlay its two-digit value as a single z-ordered unit, so that an
/// overlapping later sign occludes both an earlier sign's face and its number.
fn draw_sign<F, const W: usize, const H: usize, const N: usize, const M: usize>(
    frame: &mut F,
    tile_top_left: Point,
    sign: &Image565Mask<W, H, N, M>,
    sign_top_left: Point,
    value_center: Point,
    number: u32,
) where
    F: PixelTarget + DrawTarget<Color = Rgb565, Error = Infallible>,
{
    sign.draw_at(
        &mut *frame,
        (
            sign_top_left.x - tile_top_left.x,
            sign_top_left.y - tile_top_left.y,
        ),
    );
    let mut target = TranslatedDrawTarget::new(&mut *frame, tile_top_left);
    draw_centered_sign_value(&mut target, sign_top_left, value_center, number);
}

/// Overlay a two-digit value onto a blitted sign bitmap, centered in the open
/// area of the sign body above its baked-in label. `sign_top_left` is the screen
/// point where the bitmap's top-left was drawn, and `value_center` is the value's
/// center relative to that top-left (e.g. [`HOURS_SIGN_VALUE_CENTER`] or
/// [`MINUTE_SIGN_VALUE_CENTER`]).
fn draw_centered_sign_value<D>(
    target: &mut D,
    sign_top_left: Point,
    value_center: Point,
    number: u32,
) where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    let mut value_text = heapless::String::<4>::new();
    core::fmt::write(&mut value_text, format_args!("{:02}", number % 100))
        .expect("two-digit sign value fits in 4 bytes");
    draw_centered_text(
        target,
        &value_text,
        sign_top_left + value_center,
        &FONT_10X20,
        Rgb565::from(PLACARD_TEXT),
    );
}

// todo0000 shouldn't be needed.
fn pose_to_point(pose: Pose) -> Point {
    to_point(PROJECTION.project_pos(pose))
}

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
