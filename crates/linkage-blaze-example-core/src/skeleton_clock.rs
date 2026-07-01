//! The generic "skeleton clock" example: a motion-captured figure whose limbs
//! act as clock hands, with hour/minute placards hanging from its hands.
// todo000 the esp32 may not have a reset button for wifi

use core::array::from_fn;
use core::convert::Infallible;

use device_envoy_core::clock_sync::{ClockSync, h12_m_s};
use embedded_graphics::{
    Drawable,
    mono_font::{MonoFont, MonoTextStyle, ascii::FONT_7X13, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Size},
    primitives::Rectangle,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use linkage_blaze_core::{
    DrawItem2d, LinkageFixed, LinkageView, MarkError, Projection, Rgb888, linkage,
    linkage_fixed,
};
use log::info;
use time::OffsetDateTime;

use linkage_blaze_cyd_core::{
    Cyd, CydFrame, Image565, Image565Mask, Orientation, tga565, tga565_magenta_mask,
    tiling::{TileGrid, max_u32},
};

use crate::infallible::InfallibleResultExt;

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
const LINKAGE: LinkageView<3, 6> = LINKAGE1
    // turn the left foot out jauntily.
    .freeze_param_name::<131>("l_shin_yrotation", 57.6)
    .retain_param_names(&["head_yrotation", "l_shldr_zrotation", "r_shldr_zrotation"])
    .compact::<385>()
    .view();

// ── Projection ───────────────────────────────────────────────────────────────

const PROJECTION: Projection = Projection::front_orthographic(
    /* target origin */ Point::new(139, 306),
    /* scale */ 1.35,
);

// ── Background bitmap ──────────────────────────────────────────────────────────

/// Clock-face background bitmap, loaded at compile time.
//todo0000 we need all these numbers?
//todo0000 is tga565! good?
const BACKGROUND_BITMAP: Image565<239, 319, { 239 * 319 }> =
    tga565!("../assets/clock_back.small.tga", 239, 319);

const HOURS_SIGN: Image565Mask<45, 73, { 45 * 73 }, { (45 * 73 + 7) / 8 }> =
    tga565_magenta_mask!("../assets/hours.small.tga", 45, 73);
const HOURS_SIGN_ANCHOR_X: f32 = 22.0;
const HOURS_SIGN_VALUE_CENTER: Point = Point::new(22, 50);

const MINUTE_SIGN: Image565Mask<45, 77, { 45 * 77 }, { (45 * 77 + 7) / 8 }> =
    tga565_magenta_mask!("../assets/minute.small.tga", 45, 77);
const MINUTE_SIGN_ANCHOR_X: f32 = 22.0;
const MINUTE_SIGN_VALUE_CENTER: Point = Point::new(22, 56);

// ── Screen / tile layout ─────────────────────────────────────────────────────

pub const ORIENTATION: Orientation = Orientation::Portrait;
pub const TOP_FONT: MonoFont<'static> = FONT_7X13;
pub const WIFI_STATUS_REGION: Rectangle = Rectangle::new(Point::new(6, 6), Size::new(155, 14));
const TIME_REGION: Rectangle = Rectangle::new(
    Point::new(
        WIFI_STATUS_REGION.top_left.x + WIFI_STATUS_REGION.size.width as i32,
        WIFI_STATUS_REGION.top_left.y,
    ),
    Size::new(
        ORIENTATION.width() - WIFI_STATUS_REGION.size.width,
        WIFI_STATUS_REGION.size.height,
    ),
);

// The figure starts below the top-level display. We will tile to save memory.
const FIGURE_Y: u32 = max_u32(
    WIFI_STATUS_REGION.top_left.y as u32 + WIFI_STATUS_REGION.size.height,
    TIME_REGION.top_left.y as u32 + TIME_REGION.size.height,
);
pub const FIGURE_TILE_GRID: TileGrid = TileGrid::new(
    Point::new(0, FIGURE_Y as i32),
    Size::new(ORIENTATION.width(), ORIENTATION.height() - FIGURE_Y),
    3,
    3,
);
// ── Main function ────────────────────────────────────────────────────────

/// Run the skeleton-clock render loop forever, driven by `clock_sync` ticks and
/// drawn onto `cyd`.
pub async fn skeleton_clock<CydDevice, ClockSyncDevice>(
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
        let (hour_12, minute, _) = h12_m_s(local_time);
        info!("tick {}", text_24h(local_time));

        // Write the digital time.
        cyd.frame_mut(TIME_REGION)
            .write_text(&text_12h(local_time))
            .flush()
            .await
            .map_err(Error::Flush)?;

        // Convert the time into normalized angles for the figure's
        // the head (seconds), right arm (minutes) and left arm (hours).
        let params = linkage_params(local_time);

        // Create an iterator that will list every 3D item and its pose.
        let mut draw_items_3d = LINKAGE.draw_items_3d(&params);

        // // Iterate 3d items, project to 2D, and collect 2D items and poses.
        let mut projected_items = heapless::Vec::<_, { LINKAGE.draw_item_3d_count() }>::new();
        for draw_item_3d in draw_items_3d.by_ref() {
            projected_items
                .push(draw_item_3d.project(&PROJECTION))
                .map_err(Error::VecOverflow)?;
        }

        // Using the exhausted iterator, find the position of the middle of the left hand.
        let (hours_anchor_x, hours_anchor_y) =
            mark_lookup(draw_items_3d.pose_by_mark_name("lMid2"))?.project(&PROJECTION);
        // Find the position of the middle of the right hand.
        let (minute_anchor_x, minute_anchor_y) =
            mark_lookup(draw_items_3d.pose_by_mark_name("rMid2"))?.project(&PROJECTION);

        // Figure out where to draw the hour and minute placards.
        let hours_top_left = Point::new(
            (hours_anchor_x - HOURS_SIGN_ANCHOR_X) as i32,
            hours_anchor_y as i32,
        );
        let minute_top_left = Point::new(
            (minute_anchor_x - MINUTE_SIGN_ANCHOR_X) as i32,
            minute_anchor_y as i32,
        );

        // On each tile-backed frame ...
        // (Can't use a `for` loop and Iterator because each yielded frame
        // borrows the CYD's reusable pixel buff. This is the "lending
        // iterator" patten.)
        let mut tiles = cyd.tiles(FIGURE_TILE_GRID);
        while let Some(mut tile) = tiles.next() {
            BACKGROUND_BITMAP.draw(&mut tile).unwrap_never();

            // Draw the projected items from the linkage.
            for projected_item in &projected_items {
                projected_item.draw(&mut tile);
            }

            // Draw the hour sign and number
            HOURS_SIGN.at(hours_top_left).draw(&mut tile).unwrap_never();
            draw_centered_sign_value(
                &mut tile,
                hours_top_left,
                HOURS_SIGN_VALUE_CENTER,
                hour_12 as u32,
            );

            // Draw the minute sign and number.
            MINUTE_SIGN
                .at(minute_top_left)
                .draw(&mut tile)
                .unwrap_never();
            draw_centered_sign_value(
                &mut tile,
                minute_top_left,
                MINUTE_SIGN_VALUE_CENTER,
                minute as u32,
            );

            tile.flush().await.map_err(Error::Flush)?;
        }
    }
}

/// Draw the skeleton-clock screen *before* the time is known: the status line
/// reads `WiFi: --` / `--:--:-- --`, and the clock-face background is shown with
/// no figure or placards. Call this as early as possible (right after the display
/// is initialized) so the user sees the framed clock immediately; the per-tick
/// [`skeleton_clock`] loop then overwrites the WiFi text, time and figure as they
/// become available.
pub async fn skeleton_clock_splash<CydDevice>(
    cyd: &mut CydDevice,
) -> Result<(), Error<CydDevice::Error>>
where
    CydDevice: Cyd,
{
    cyd.frame_mut(WIFI_STATUS_REGION)
        .write_text("WiFi: --")
        .flush()
        .await
        .map_err(Error::Flush)?;

    cyd.frame_mut(TIME_REGION)
        .write_text("--:--:-- --")
        .flush()
        .await
        .map_err(Error::Flush)?;

    let mut tiles = cyd.tiles(FIGURE_TILE_GRID);
    while let Some(mut frame) = tiles.next() {
        BACKGROUND_BITMAP.draw(&mut frame).unwrap_never();
        frame.flush().await.map_err(Error::Flush)?;
    }

    Ok(())
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

fn linkage_params(local_time: &OffsetDateTime) -> [f32; 3] {
    // Parameters are positional and depend on the order in the original `.lb.rs`.

    // Index of each clock hand's param: the head shows seconds, the right arm
    // minutes, the left arm hours.
    const SECOND_INDEX: usize = 0;
    const MINUTE_INDEX: usize = 1;
    const HOUR_INDEX: usize = 2;

    // Each param's range spans this many full turns, read straight from the linkage,
    // so one clock turn maps to 1 / span of the normalized param.
    const SECOND_SPAN_TURNS: f32 = param_span_turns(SECOND_INDEX);
    const MINUTE_SPAN_TURNS: f32 = param_span_turns(MINUTE_INDEX);
    const HOUR_SPAN_TURNS: f32 = param_span_turns(HOUR_INDEX);

    // Check that everything is as expected.
    const _: () = {
        assert_param_name(SECOND_INDEX, "head_yrotation");
        assert_param_name(MINUTE_INDEX, "r_shldr_zrotation");
        assert_param_name(HOUR_INDEX, "l_shldr_zrotation");
        assert!(SECOND_SPAN_TURNS == 4.0);
        assert!(MINUTE_SPAN_TURNS == 4.0);
        assert!(HOUR_SPAN_TURNS == 4.0);
    };

    // Calibration: what param value in 0..1 represents 12:00:00?
    const HEAD_AT_12_PARAM: f32 = 0.5;
    const RIGHT_ARM_AT_12_PARAM: f32 = 0.4375;
    const LEFT_ARM_AT_12_PARAM: f32 = 0.5625;

    // Find the fraction of a turn for each hand.
    let seconds_turn = local_time.second() as f32 / 60.0;
    let minutes_turn = (local_time.minute() as f32 + seconds_turn) / 60.0;
    let hours_turn = ((local_time.hour() % 12) as f32 + minutes_turn) / 12.0;

    // Set each 0.0 to 1.0 parameter in the correct order.
    from_fn(|index| match index {
        SECOND_INDEX => wrap_unit(HEAD_AT_12_PARAM + seconds_turn / SECOND_SPAN_TURNS),
        MINUTE_INDEX => wrap_unit(RIGHT_ARM_AT_12_PARAM + minutes_turn / MINUTE_SPAN_TURNS),
        HOUR_INDEX => wrap_unit(LEFT_ARM_AT_12_PARAM + hours_turn / HOUR_SPAN_TURNS),
        _ => unreachable!(),
    })
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

/// Compile-time assert that `LINKAGE`'s param `index` has the expected name.
const fn assert_param_name(index: usize, name: &str) {
    assert!(str_eq(LINKAGE.param(index).name(), name));
}

/// The span of `LINKAGE`'s param `index`, in full turns (1 turn = 360°), read from
/// the linkage's stored range.
const fn param_span_turns(index: usize) -> f32 {
    use core::f32::consts::TAU;
    let (low, high) = LINKAGE.scan_param_range(index);
    (high - low) / TAU
}

/// Const string equality, for the compile-time param-order assert in `linkage_params`.
const fn str_eq(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut i = 0;
    while i < left.len() {
        if left[i] != right[i] {
            return false;
        }
        i += 1;
    }
    true
}

// ── Skeleton-clock-specific overlay drawing ──────────────────────────────────

// All overlay drawing happens against a `DrawTarget` whose coordinates are in
// figure-region space; tiled frames from `Cyd::tiles` subtract the shared
// figure-region tile top-left so these functions never need to know they are
// rendering into a tile.

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
        .unwrap_never();
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

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, derive_more::From)]
pub struct MarkLookupError(pub MarkError);

fn mark_lookup<T>(result: Result<T, MarkError>) -> Result<T, MarkLookupError> {
    Ok(result?)
}

/// Error from the generic skeleton-clock loop, generic over the surface's flush
/// error `F`.
///
/// Our own [`MarkLookupError`] gets a derived `From`, so it propagates with a
/// plain `?`. The device's flush error `F` and the overflow value are converted
/// explicitly with `.map_err(...)` at the call site: a blanket `From<F>` would
/// be greedy enough to collide with that concrete `From` under coherence.
#[derive(Debug, derive_more::From)]
pub enum Error<F> {
    /// Flushing a frame to the display failed.
    #[from(ignore)]
    Flush(F),
    /// A required figure mark was not found.
    Mark(MarkLookupError),
    /// The projected-items scratch buffer was smaller than the linkage draw-item count.
    #[from(ignore)]
    VecOverflow(DrawItem2d),
}
