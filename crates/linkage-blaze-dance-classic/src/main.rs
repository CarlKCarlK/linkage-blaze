#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_esp::{
    Error,
    button::{ButtonEsp, PressedTo},
    clock_sync::{ClockSync as _, ClockSyncEsp, ClockSyncStaticEsp, CoreError, ONE_SECOND},
    flash_block::FlashBlockEsp,
    init_and_start,
    wifi_auto::{
        WifiAuto as _, WifiAutoEsp, WifiAutoEvent,
        fields::{TimezoneField, TimezoneFieldStatic},
    },
};
use embassy_executor::Spawner;
use embedded_graphics::{
    Drawable,
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_6X10, FONT_9X15_BOLD},
    },
    prelude::{Point, Size},
    text::{Baseline, Text},
};
use esp_backtrace as _;
use linkage_blaze_core::{
    DrawItemIter, LinkageFixed, NegXProjection, PixelSurface, PixelTarget, Pose, Projection,
    Rgb888, WebColors, linkage, linkage_fixed, render_draw_items, to_point,
};
use linkage_blaze_cyd::{
    Cyd, CydDisplayConfig, CydStatic, PixelBuffer, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use log::info;

// ── Palette ──────────────────────────────────────────────────────────────────

const BACKGROUND: Rgb888 = Rgb888::CSS_MIDNIGHT_BLUE; // deep night blue (25, 25, 112)
const FIGURE: Rgb888 = Rgb888::CSS_WHEAT; // warm pale bone-like tan (245, 222, 179)
const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE; // muted cool text (176, 196, 222)
const PLACARD_FILL: Rgb888 = Rgb888::new(25, 60, 70); // dark teal sign face

// ── Screen / tile layout ─────────────────────────────────────────────────────
const TEXT_BAND_HEIGHT: i32 = 34;
const WIFI_TEXT_TOP_LEFT: Point = Point::new(8, 12);
const TIME_TEXT_TOP_LEFT: Point = Point::new(166, 12);

// Dance figure is rendered into a virtual 375×500 canvas offset from (0,0).
const TOP_LEFT: Point = Point::new(-68, -170);
const TILE_COLUMNS: usize = 3;
const TILE_ROWS: usize = 4;
const TILE_WIDTH: usize = 125;
const TILE_HEIGHT: usize = 125;
const TILE_PIXELS: usize = TILE_WIDTH * TILE_HEIGHT;

// ── Digit glyph metrics (shared across draw_digit / draw_number_centered) ────

const DIGIT_W: i32 = 3; // 3×5 pixel cell width
const DIGIT_H: i32 = 5; // 3×5 pixel cell height
const DIGIT_SCALE: i32 = 2; // 3×5 cells become 6×10 px glyphs
const DIGIT_GAP: i32 = 2; // gap between the two digits of a placard

// ── Projection ───────────────────────────────────────────────────────────────

//todo000 review projections.
const PROJECTION: NegXProjection = NegXProjection {
    center_x: 207.0,
    baseline_y: 480.0,
    scale: 1.35,
};

// ── Linkage constants ─────────────────────────────────────────────────────────

const LINKAGE_INNER: LinkageFixed<132, 6, 600> = LinkageFixed::<0, 0, 3>::start()
    .pen_width(3.5)
    .pen_color(FIGURE)
    .combine(linkage_fixed!(
        "../../linkage-blaze-mocap/samples/pirouette.lb.rs",
        132,
        6,
        600
    ));

// todo000 I thought we killed _at_default
// todo000 can we kill or reduce the optimization steps?
const LINKAGE: LinkageFixed<3, 6, 400> = LINKAGE_INNER
    .freeze_param_name::<131>("l_shin_yrotation", 57.6)
    .freeze_param_name_at_default::<130>("abdomen_xrotation")
    .retain_param_names(&["head_yrotation", "l_shldr_zrotation", "r_shldr_zrotation"])
    .strip_fixed_noops::<400>()
    .merge_adjacent_fixed::<400>()
    .strip_fixed_noops::<400>();

// ── TileFlush ────────────────────────────────────────────────────────────────

// todo000 seems overly complex.
struct TileFlush {
    top_left: Point,
    origin: Point,
    width: usize,
    height: usize,
}

impl TileFlush {
    fn new(tile_origin: Point, tile_width: usize, tile_height: usize) -> Option<Self> {
        let tile_top_left = Point::new(TOP_LEFT.x + tile_origin.x, TOP_LEFT.y + tile_origin.y);
        let tile_bottom_right = Point::new(
            tile_top_left.x + tile_width as i32,
            tile_top_left.y + tile_height as i32,
        );
        let visible_left = tile_top_left.x.max(0);
        let visible_top = tile_top_left.y.max(TEXT_BAND_HEIGHT);
        let visible_right = tile_bottom_right.x.min(SCREEN_WIDTH as i32);
        let visible_bottom = tile_bottom_right.y.min(SCREEN_HEIGHT as i32);

        if visible_left >= visible_right || visible_top >= visible_bottom {
            return None;
        }

        Some(Self {
            top_left: Point::new(visible_left, visible_top),
            origin: Point::new(
                tile_origin.x + visible_left - tile_top_left.x,
                tile_origin.y + visible_top - tile_top_left.y,
            ),
            width: (visible_right - visible_left) as usize,
            height: (visible_bottom - visible_top) as usize,
        })
    }
}

// ── Time / param helpers ─────────────────────────────────────────────────────

/// Format a 12-hour clock string with AM/PM, right-justified to 11 characters
/// (e.g. " 5:04:32 PM" or "12:04:32 PM"). No alloc.
fn format_clock_12h(hours: u8, minutes: u8, seconds: u8) -> heapless::String<16> {
    let hour12 = match hours % 12 {
        0 => 12,
        other => other,
    };
    let suffix = if hours % 24 < 12 { "AM" } else { "PM" };
    let mut text = heapless::String::new();
    core::fmt::write(
        &mut text,
        format_args!("{hour12:>2}:{minutes:02}:{seconds:02} {suffix}"),
    )
    .expect("clock string fits in 16 bytes");
    text
}

// todo000 seems overly complex.
fn dance_params(hours: u8, minutes: u8, seconds: u8) -> [f32; 3] {
    const CLOCK_HAND_PARAM_TURN: f32 = 0.25;
    const EYES_FORWARD_PARAM: f32 = 0.5;
    const RIGHT_ARM_12_PARAM: f32 = 0.4375;
    const LEFT_ARM_12_PARAM: f32 = 0.5625;
    let second_phase = seconds as f32 / 60.0;
    let minute_phase = (minutes as f32 + second_phase) / 60.0;
    let hour_phase = ((hours % 12) as f32 + minute_phase) / 12.0;
    let signed_hour_phase = signed_phase_from_twelve(hour_phase);
    [
        wrap_param(EYES_FORWARD_PARAM + second_phase * CLOCK_HAND_PARAM_TURN),
        wrap_param(RIGHT_ARM_12_PARAM + minute_phase * CLOCK_HAND_PARAM_TURN),
        wrap_param(LEFT_ARM_12_PARAM + signed_hour_phase * CLOCK_HAND_PARAM_TURN),
    ]
}

fn signed_phase_from_twelve(phase: f32) -> f32 {
    if phase > 0.5 { phase - 1.0 } else { phase }
}

fn wrap_param(value: f32) -> f32 {
    let mut value = value;
    while value >= 1.0 {
        value -= 1.0;
    }
    while value < 0.0 {
        value += 1.0;
    }
    value
}

// ── Render ───────────────────────────────────────────────────────────────────

fn render_tile<T: PixelTarget>(
    surface: &mut PixelSurface<'_, T>,
    params: &[f32; 3],
    hours: u8,
    minutes: u8,
) {
    // Faint dial marks first, so the figure and placards draw over them.
    draw_dial(surface);

    let dance_view = LINKAGE.view();
    let mut iter: DrawItemIter<3, 6> = dance_view.draw_items(params);
    render_draw_items(&PROJECTION, surface, &mut iter);

    // Hanging placards use the hand marks only as anchor points; they hang
    // straight down in screen coordinates and do not inherit hand rotation.
    let hour_display = if hours % 12 == 0 { 12 } else { hours % 12 };
    let right_hand_pose = iter
        .marked_pose("rMid2")
        .expect("rMid2 mark missing from LINKAGE");
    let left_hand_pose = iter
        .marked_pose("lMid2")
        .expect("lMid2 mark missing from LINKAGE");
    draw_hanging_placard(surface, pose_to_point(left_hand_pose), hour_display as u32);
    draw_hanging_placard(surface, pose_to_point(right_hand_pose), minutes as u32);
}

fn draw_dial<T: PixelTarget>(surface: &mut PixelSurface<'_, T>) {
    const DIAL_COLOR: Rgb888 = Rgb888::CSS_DARK_SLATE_GRAY; // muted teal-gray (47, 79, 79)
    const DIAL_SCALE: i32 = 2;
    const DIAL_CENTER_SCREEN: Point = Point::new(120, 178);
    const DIAL_RADIUS_X: i32 = 100;
    const DIAL_RADIUS_Y: i32 = 118;

    // Convert screen-space dial center to dance space (offset by TOP_LEFT).
    let center_x = DIAL_CENTER_SCREEN.x - TOP_LEFT.x;
    let center_y = DIAL_CENTER_SCREEN.y - TOP_LEFT.y;
    let top = Point::new(center_x, center_y - DIAL_RADIUS_Y);
    let bottom = Point::new(center_x, center_y + DIAL_RADIUS_Y);
    let right = Point::new(center_x + DIAL_RADIUS_X, center_y);
    let left = Point::new(center_x - DIAL_RADIUS_X, center_y);
    draw_number_centered(surface, 12, top, DIAL_COLOR, DIAL_SCALE);
    draw_number_centered(surface, 3, right, DIAL_COLOR, DIAL_SCALE);
    draw_number_centered(surface, 6, bottom, DIAL_COLOR, DIAL_SCALE);
    draw_number_centered(surface, 9, left, DIAL_COLOR, DIAL_SCALE);
}

/// Draw a hanging number sign anchored at `anchor` (a hand mark in screen
/// coordinates).  The sign is a fixed size and always shows two digits.  It
/// hangs straight down via a short vertical hook from the hand, then a triangle
/// splays out to the sign's two top corners.  `number` is shown modulo 100.
fn draw_hanging_placard<T: PixelTarget>(
    surface: &mut PixelSurface<'_, T>,
    anchor: Point,
    number: u32,
) {
    const PLACARD_BORDER: Rgb888 = FIGURE;
    const PLACARD_TEXT: Rgb888 = FIGURE;
    const PLACARD_W: i32 = 34;
    const PLACARD_H: i32 = 20;
    const PLACARD_BORDER_PX: i32 = 2;
    const HANGER_PX: i32 = 2;
    const HANGER_HOOK: i32 = 7;
    const HANGER_TRIANGLE: i32 = 22;

    let card_left = anchor.x - PLACARD_W / 2;
    let card_top = anchor.y + HANGER_HOOK + HANGER_TRIANGLE;
    let card_right = card_left + PLACARD_W;

    let apex = Point::new(anchor.x, anchor.y + HANGER_HOOK);
    draw_segment(surface, anchor, apex, PLACARD_BORDER, HANGER_PX);
    draw_segment(
        surface,
        apex,
        Point::new(card_left, card_top),
        PLACARD_BORDER,
        HANGER_PX,
    );
    draw_segment(
        surface,
        apex,
        Point::new(card_right, card_top),
        PLACARD_BORDER,
        HANGER_PX,
    );

    fill_rect(
        surface,
        card_left,
        card_top,
        PLACARD_W,
        PLACARD_H,
        PLACARD_FILL,
    );
    draw_rect_border(
        surface,
        card_left,
        card_top,
        PLACARD_W,
        PLACARD_H,
        PLACARD_BORDER_PX,
        PLACARD_BORDER,
    );

    let glyph_w = DIGIT_W * DIGIT_SCALE;
    let glyph_h = DIGIT_H * DIGIT_SCALE;
    let total_w = 2 * glyph_w + DIGIT_GAP;
    let text_left = card_left + (PLACARD_W - total_w) / 2;
    let text_top = card_top + (PLACARD_H - glyph_h) / 2;
    let value = number % 100;
    draw_digit(
        surface,
        value / 10,
        Point::new(text_left, text_top),
        PLACARD_TEXT,
        DIGIT_SCALE,
    );
    draw_digit(
        surface,
        value % 10,
        Point::new(text_left + glyph_w + DIGIT_GAP, text_top),
        PLACARD_TEXT,
        DIGIT_SCALE,
    );
}

/// Draw a 1- or 2-digit number centered on `center`, at the given pixel scale.
fn draw_number_centered<T: PixelTarget>(
    surface: &mut PixelSurface<'_, T>,
    number: u32,
    center: Point,
    color: Rgb888,
    scale: i32,
) {
    let glyph_w = DIGIT_W * scale;
    let glyph_h = DIGIT_H * scale;
    let digit_count = if number >= 10 { 2 } else { 1 };
    let total_w = digit_count * glyph_w + (digit_count - 1) * DIGIT_GAP;
    let left = center.x - total_w / 2;
    let top = center.y - glyph_h / 2;
    if digit_count == 2 {
        draw_digit(surface, number / 10, Point::new(left, top), color, scale);
        draw_digit(
            surface,
            number % 10,
            Point::new(left + glyph_w + DIGIT_GAP, top),
            color,
            scale,
        );
    } else {
        draw_digit(surface, number, Point::new(left, top), color, scale);
    }
}

fn draw_digit<T: PixelTarget>(
    surface: &mut PixelSurface<'_, T>,
    digit: u32,
    origin: Point,
    color: Rgb888,
    scale: i32,
) {
    // Each digit is 3×5 pixels, encoded as 15 bits (row-major, top-to-bottom, left-to-right).
    #[rustfmt::skip]
    const DIGIT_BITMAPS: [u16; 10] = [
        0b111_101_101_101_111, // 0
        0b010_110_010_010_111, // 1
        0b111_001_111_100_111, // 2
        0b111_001_111_001_111, // 3
        0b101_101_111_001_001, // 4
        0b111_100_111_001_111, // 5
        0b111_100_111_101_111, // 6
        0b111_001_001_001_001, // 7
        0b111_101_111_101_111, // 8
        0b111_101_111_001_111, // 9
    ];

    let bits = DIGIT_BITMAPS[(digit % 10) as usize];
    for row in 0..DIGIT_H {
        for col in 0..DIGIT_W {
            let bit = 14 - (row * DIGIT_W + col);
            if (bits >> bit) & 1 == 1 {
                for scale_y in 0..scale {
                    for scale_x in 0..scale {
                        surface.put_pixel(
                            origin.x + col * scale + scale_x,
                            origin.y + row * scale + scale_y,
                            color,
                        );
                    }
                }
            }
        }
    }
}

fn fill_rect<T: PixelTarget>(
    surface: &mut PixelSurface<'_, T>,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    color: Rgb888,
) {
    for dy in 0..height {
        for dx in 0..width {
            surface.put_pixel(left + dx, top + dy, color);
        }
    }
}

fn draw_rect_border<T: PixelTarget>(
    surface: &mut PixelSurface<'_, T>,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    thickness: i32,
    color: Rgb888,
) {
    for dy in 0..height {
        for dx in 0..width {
            let on_border = dx < thickness
                || dx >= width - thickness
                || dy < thickness
                || dy >= height - thickness;
            if on_border {
                surface.put_pixel(left + dx, top + dy, color);
            }
        }
    }
}

fn draw_segment<T: PixelTarget>(
    surface: &mut PixelSurface<'_, T>,
    start: Point,
    end: Point,
    color: Rgb888,
    thickness: i32,
) {
    // Brush spans `thickness` pixels, supporting both even and odd widths.
    let brush_low = -(thickness / 2);
    let brush_high = brush_low + thickness - 1;
    let mut current_x = start.x;
    let mut current_y = start.y;
    let delta_x = (end.x - start.x).abs();
    let delta_y = -(end.y - start.y).abs();
    let step_x = if start.x < end.x { 1 } else { -1 };
    let step_y = if start.y < end.y { 1 } else { -1 };
    let mut error = delta_x + delta_y;

    loop {
        let mut dy = brush_low;
        while dy <= brush_high {
            let mut dx = brush_low;
            while dx <= brush_high {
                surface.put_pixel(current_x + dx, current_y + dy, color);
                dx += 1;
            }
            dy += 1;
        }
        if current_x == end.x && current_y == end.y {
            break;
        }
        let doubled_error = error * 2;
        if doubled_error >= delta_y {
            error += delta_y;
            current_x += step_x;
        }
        if doubled_error <= delta_x {
            error += delta_x;
            current_y += step_y;
        }
    }
}

// todo0000 shouldn't be needed.
fn pose_to_point(pose: Pose) -> Point {
    to_point(PROJECTION.project_pos(pose))
}

// ── Binary entry point ────────────────────────────────────────────────────────

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    DeviceEnvoy(Error),
    Cyd(linkage_blaze_cyd::CydError),
}

impl From<CoreError> for MainError {
    fn from(error: CoreError) -> Self {
        MainError::DeviceEnvoy(error.into())
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD dance with WiFi");

    static CYD_STATIC: CydStatic<PixelBuffer<TILE_PIXELS>> = CydStatic::new();
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
        CydDisplayConfig::PORTRAIT,
    )?;
    cyd.clear(Cyd::rgb565(BACKGROUND))?;
    info!("CYD display initialized");

    let [wifi_auto_flash_block, timezone_flash_block] = FlashBlockEsp::new_array::<2>(p.FLASH)?;

    static TIMEZONE_FIELD_STATIC: TimezoneFieldStatic = TimezoneField::new_static();
    let timezone_field = TimezoneField::new(&TIMEZONE_FIELD_STATIC, timezone_flash_block);
    let mut force_portal_button = ButtonEsp::new(p.GPIO0, PressedTo::Ground);

    let wifi_auto = WifiAutoEsp::new(
        p.WIFI,
        wifi_auto_flash_block,
        "CydDance",
        [timezone_field],
        spawner,
    )?;
    // todo000 verify WiFi status events are clearly visible in the serial log now
    // that the display no longer shows status during the setup/connect phase.
    let stack = wifi_auto
        .connect(&mut force_portal_button, |wifi_auto_event| async move {
            let wifi_mode = match wifi_auto_event {
                WifiAutoEvent::CaptivePortalReady => "setup CydDance",
                WifiAutoEvent::Connecting { .. } => "connecting",
                WifiAutoEvent::ConnectionFailed => "connect failed",
            };
            info!("WiFi: {wifi_mode}");
            Ok(())
        })
        .await?;
    info!("WiFi connected");

    let timezone_offset_minutes = timezone_field
        .offset_minutes()?
        .ok_or(Error::MissingCustomWifiAutoField)?;

    static CLOCK_SYNC_STATIC: ClockSyncStaticEsp = ClockSyncEsp::new_static();
    let clock_sync = ClockSyncEsp::new(
        &CLOCK_SYNC_STATIC,
        stack,
        timezone_offset_minutes,
        Some(ONE_SECOND),
        spawner,
    )?;
    info!("clock sync ready; entering dance loop");

    let background565 = Cyd::rgb565(BACKGROUND);
    let text565 = Cyd::rgb565(TEXT);

    loop {
        let tick = clock_sync.wait_for_tick().await;
        let local_time = tick.local_time;
        let hours = local_time.hour();
        let minutes = local_time.minute();
        let seconds = local_time.second();
        info!("tick {:02}:{:02}:{:02}", hours, minutes, seconds);

        let params = dance_params(hours, minutes, seconds);
        let time_text = format_clock_12h(hours, minutes, seconds);

        cyd.draw_frame(
            Size::new(SCREEN_WIDTH as u32, TEXT_BAND_HEIGHT as u32),
            Point::new(0, 0),
            |frame| {
                frame.clear(background565);
                Text::with_baseline(
                    "WiFi OK",
                    WIFI_TEXT_TOP_LEFT,
                    MonoTextStyle::new(&FONT_6X10, text565),
                    Baseline::Top,
                )
                .draw(frame)
                .expect("drawing to an Infallible frame cannot fail");
                Text::with_baseline(
                    time_text.as_str(),
                    TIME_TEXT_TOP_LEFT,
                    MonoTextStyle::new(&FONT_9X15_BOLD, text565),
                    Baseline::Top,
                )
                .draw(frame)
                .expect("drawing to an Infallible frame cannot fail");
            },
        )?;

        for tile_row in 0..TILE_ROWS {
            for tile_column in 0..TILE_COLUMNS {
                let tile_origin = Point::new(
                    (tile_column * TILE_WIDTH) as i32,
                    (tile_row * TILE_HEIGHT) as i32,
                );
                let Some(tile_flush) = TileFlush::new(tile_origin, TILE_WIDTH, TILE_HEIGHT) else {
                    continue;
                };
                cyd.draw_frame(
                    Size::new(tile_flush.width as u32, tile_flush.height as u32),
                    tile_flush.top_left,
                    |frame| {
                        frame.clear(background565);
                        render_tile(
                            &mut PixelSurface {
                                target: frame,
                                tile_origin: tile_flush.origin,
                            },
                            &params,
                            hours,
                            minutes,
                        );
                    },
                )?;
            }
        }
    }
}
