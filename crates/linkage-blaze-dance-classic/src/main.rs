#![no_std]
#![no_main]

// todo000 wifi status is missing.
// todo000 we need to use color and/or size to tell hours from minutes
// todo000 we need some wasm preview

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
    DrawItemIter, LinkageFixed, NegXProjection, PixelTarget, Pose, Projection, Rgb888, WebColors,
    linkage, linkage_fixed, to_point,
};
use linkage_blaze_cyd::{Cyd, CydDisplayConfig, CydStatic, PixelBuffer};
use log::info;

// ── Palette ──────────────────────────────────────────────────────────────────

const BACKGROUND: Rgb888 = Rgb888::CSS_MIDNIGHT_BLUE; // deep night blue (25, 25, 112)
const FIGURE: Rgb888 = Rgb888::CSS_WHEAT; // warm pale bone-like tan (245, 222, 179)
const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE; // muted cool text (176, 196, 222)
const PLACARD_FILL: Rgb888 = Rgb888::new(25, 60, 70); // dark teal sign face

// ── Screen / tile layout ─────────────────────────────────────────────────────
const DISPLAY_WIDTH: usize = 240; // portrait CYD screen width
const DISPLAY_HEIGHT: usize = 320; // portrait CYD screen height
const TEXT_BAND_HEIGHT: usize = 34;
const WIFI_TEXT_TOP_LEFT: Point = Point::new(8, 12);
const TIME_TEXT_TOP_LEFT: Point = Point::new(166, 12);

const BODY_TOP: usize = TEXT_BAND_HEIGHT; // first row of the dance area (286 px tall)

// 3×80 = 240 covers display width exactly; 3×96 = 288 covers body height (286) with 2 px clip on the last row.
const TILE_COLUMNS: usize = 3;
const TILE_ROWS: usize = 3;
const TILE_WIDTH: usize = 80;
const TILE_HEIGHT: usize = 96;

// The shared pixel buffer must hold the largest frame: a dance tile or the full-width text band.
const TILE_PIXELS: usize = TILE_WIDTH * TILE_HEIGHT;
const TEXT_BAND_PIXELS: usize = DISPLAY_WIDTH * TEXT_BAND_HEIGHT;
const WORKSPACE_PIXELS: usize = if TILE_PIXELS > TEXT_BAND_PIXELS {
    TILE_PIXELS
} else {
    TEXT_BAND_PIXELS
};

// ── Digit glyph metrics (shared across draw_digit / draw_number_centered) ────

const DIGIT_W: i32 = 3; // 3×5 pixel cell width
const DIGIT_H: i32 = 5; // 3×5 pixel cell height
const DIGIT_SCALE: i32 = 2; // 3×5 cells become 6×10 px glyphs
const DIGIT_GAP: i32 = 2; // gap between the two digits of a placard

// ── Projection ───────────────────────────────────────────────────────────────

//todo000 review projections.
const PROJECTION: NegXProjection = NegXProjection {
    center_x: 120.0,
    baseline_y: 300.0,
    scale: 1.25,
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

// ── TileRect ─────────────────────────────────────────────────────────────────

struct TileRect {
    top_left: Point,
    width: usize,
    height: usize,
}

impl TileRect {
    fn new(tile_column: usize, tile_row: usize) -> Option<Self> {
        let left = tile_column * TILE_WIDTH;
        let top = BODY_TOP + tile_row * TILE_HEIGHT;

        if left >= DISPLAY_WIDTH || top >= DISPLAY_HEIGHT {
            return None;
        }

        Some(Self {
            top_left: Point::new(left as i32, top as i32),
            width: TILE_WIDTH.min(DISPLAY_WIDTH - left),
            height: TILE_HEIGHT.min(DISPLAY_HEIGHT - top),
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

// ── Dance-specific overlay drawing ───────────────────────────────────────────

/// Wraps a tile buffer and its screen-space origin for 2D overlay drawing.
///
/// All coordinate arguments to drawing methods are in screen coordinates.
/// The struct subtracts the tile origin to produce tile-local buffer coordinates.
struct DanceOverlay<'a, T: PixelTarget> {
    target: &'a mut T,
    tile_origin: Point,
}

impl<'a, T: PixelTarget> DanceOverlay<'a, T> {
    fn new(target: &'a mut T, tile_origin: Point) -> Self {
        Self {
            target,
            tile_origin,
        }
    }

    fn put_pixel(&mut self, x: i32, y: i32, color: Rgb888) {
        let bx = x - self.tile_origin.x;
        let by = y - self.tile_origin.y;
        if bx < 0 || by < 0 {
            return;
        }
        self.target.put_pixel(bx as usize, by as usize, color);
    }

    fn draw_digit(&mut self, digit: u32, at: Point, color: Rgb888, scale: i32) {
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
                            self.put_pixel(
                                at.x + col * scale + scale_x,
                                at.y + row * scale + scale_y,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }

    fn fill_rect(&mut self, left: i32, top: i32, width: i32, height: i32, color: Rgb888) {
        for dy in 0..height {
            for dx in 0..width {
                self.put_pixel(left + dx, top + dy, color);
            }
        }
    }

    fn draw_rect_border(
        &mut self,
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
                    self.put_pixel(left + dx, top + dy, color);
                }
            }
        }
    }

    fn draw_segment(&mut self, start: Point, end: Point, color: Rgb888, thickness: i32) {
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
            for dy in brush_low..=brush_high {
                for dx in brush_low..=brush_high {
                    self.put_pixel(current_x + dx, current_y + dy, color);
                }
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

    fn draw_number_centered(&mut self, number: u32, center: Point, color: Rgb888, scale: i32) {
        let glyph_w = DIGIT_W * scale;
        let glyph_h = DIGIT_H * scale;
        let digit_count = if number >= 10 { 2 } else { 1 };
        let total_w = digit_count * glyph_w + (digit_count - 1) * DIGIT_GAP;
        let left = center.x - total_w / 2;
        let top = center.y - glyph_h / 2;
        if digit_count == 2 {
            self.draw_digit(number / 10, Point::new(left, top), color, scale);
            self.draw_digit(
                number % 10,
                Point::new(left + glyph_w + DIGIT_GAP, top),
                color,
                scale,
            );
        } else {
            self.draw_digit(number, Point::new(left, top), color, scale);
        }
    }

    fn draw_dial(&mut self) {
        const DIAL_COLOR: Rgb888 = Rgb888::CSS_DARK_SLATE_GRAY; // muted teal-gray (47, 79, 79)
        const DIAL_SCALE: i32 = 2;
        const DIAL_CENTER_SCREEN: Point = Point::new(120, 178);
        const DIAL_RADIUS_X: i32 = 100;
        const DIAL_RADIUS_Y: i32 = 118;

        let center_x = DIAL_CENTER_SCREEN.x;
        let center_y = DIAL_CENTER_SCREEN.y;
        let top = Point::new(center_x, center_y - DIAL_RADIUS_Y);
        let bottom = Point::new(center_x, center_y + DIAL_RADIUS_Y);
        let right = Point::new(center_x + DIAL_RADIUS_X, center_y);
        let left = Point::new(center_x - DIAL_RADIUS_X, center_y);
        self.draw_number_centered(12, top, DIAL_COLOR, DIAL_SCALE);
        self.draw_number_centered(3, right, DIAL_COLOR, DIAL_SCALE);
        self.draw_number_centered(6, bottom, DIAL_COLOR, DIAL_SCALE);
        self.draw_number_centered(9, left, DIAL_COLOR, DIAL_SCALE);
    }

    /// Draw a hanging number sign anchored at `anchor` (a hand mark in dance
    /// coordinates).  The sign is a fixed size and always shows two digits.  It
    /// hangs straight down via a short vertical hook from the hand, then a triangle
    /// splays out to the sign's two top corners.  `number` is shown modulo 100.
    fn draw_hanging_placard(&mut self, anchor: Point, number: u32) {
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
        self.draw_segment(anchor, apex, PLACARD_BORDER, HANGER_PX);
        self.draw_segment(
            apex,
            Point::new(card_left, card_top),
            PLACARD_BORDER,
            HANGER_PX,
        );
        self.draw_segment(
            apex,
            Point::new(card_right, card_top),
            PLACARD_BORDER,
            HANGER_PX,
        );

        self.fill_rect(card_left, card_top, PLACARD_W, PLACARD_H, PLACARD_FILL);
        self.draw_rect_border(
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
        self.draw_digit(
            value / 10,
            Point::new(text_left, text_top),
            PLACARD_TEXT,
            DIGIT_SCALE,
        );
        self.draw_digit(
            value % 10,
            Point::new(text_left + glyph_w + DIGIT_GAP, text_top),
            PLACARD_TEXT,
            DIGIT_SCALE,
        );
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
    Core(CoreError),
    Cyd(linkage_blaze_cyd::CydError),
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

    static CYD_STATIC: CydStatic<PixelBuffer<WORKSPACE_PIXELS>> = CydStatic::new();
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

        let mut cyd_tile =
            cyd.frame_mut(Size::new(DISPLAY_WIDTH as u32, TEXT_BAND_HEIGHT as u32));
        cyd_tile.clear(background565);
        Text::with_baseline(
            "WiFi OK",
            WIFI_TEXT_TOP_LEFT,
            MonoTextStyle::new(&FONT_6X10, text565),
            Baseline::Top,
        )
        .draw(&mut cyd_tile)
        .expect("drawing to an Infallible frame cannot fail");
        Text::with_baseline(
            time_text.as_str(),
            TIME_TEXT_TOP_LEFT,
            MonoTextStyle::new(&FONT_9X15_BOLD, text565),
            Baseline::Top,
        )
        .draw(&mut cyd_tile)
        .expect("drawing to an Infallible frame cannot fail");
        cyd_tile.flush()?;

        let hour_display = if hours % 12 == 0 { 12 } else { hours % 12 };

        for tile_row in 0..TILE_ROWS {
            for tile_column in 0..TILE_COLUMNS {
                let Some(tile) = TileRect::new(tile_column, tile_row) else {
                    continue;
                };
                let mut cyd_tile =
                    cyd.frame_mut(Size::new(tile.width as u32, tile.height as u32));
                cyd_tile.clear(background565);

                // Dance-specific background overlay.
                DanceOverlay::new(&mut cyd_tile, tile.top_left).draw_dial();

                // Shared linkage rendering path, identical to the ballet app.
                let linkage_view = LINKAGE.view();
                let mut iter: DrawItemIter<3, 6> = linkage_view.draw_items(&params);
                for draw_item in &mut iter {
                    draw_item
                        .project(&PROJECTION)
                        .draw_offset(&mut cyd_tile, tile.top_left);
                }

                // Dance-specific foreground overlay: placards hang from hand marks.
                let right_hand_pose = iter
                    .marked_pose("rMid2")
                    .expect("rMid2 mark missing from LINKAGE");
                let left_hand_pose = iter
                    .marked_pose("lMid2")
                    .expect("lMid2 mark missing from LINKAGE");
                let mut overlay = DanceOverlay::new(&mut cyd_tile, tile.top_left);
                overlay.draw_hanging_placard(pose_to_point(left_hand_pose), hour_display as u32);
                overlay.draw_hanging_placard(pose_to_point(right_hand_pose), minutes as u32);

                cyd_tile.flush_at(tile.top_left)?;
            }
        }
    }
}
