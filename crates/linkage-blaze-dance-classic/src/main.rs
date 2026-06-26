#![no_std]
#![no_main]

// todo000 wifi status is missing. (may no longer apply)
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
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Primitive, Size},
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use esp_backtrace as _;
use linkage_blaze_core::{
    LinkageFixed, MarkError, NegXProjection, Pose, Projection, Rgb888, WebColors, linkage,
    linkage_fixed, to_point,
};
use linkage_blaze_cyd::{
    Cyd, CydError, CydStatic, Orientation, TranslatedDrawTarget,
    tiling::{TileGrid, max_u32, max_usize},
};
use log::info;
use time::OffsetDateTime;

// ── Palette ──────────────────────────────────────────────────────────────────

const BACKGROUND: Rgb888 = Rgb888::CSS_MIDNIGHT_BLUE; // deep night blue (25, 25, 112)
const FIGURE: Rgb888 = Rgb888::CSS_WHEAT; // warm pale bone-like tan (245, 222, 179)
const FOREGROUND: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE; // muted cool text (176, 196, 222)
const PLACARD_FILL: Rgb888 = Rgb888::new(25, 60, 70); // dark teal sign face

// ── Screen / tile layout ─────────────────────────────────────────────────────
const ORIENTATION: Orientation = Orientation::Portrait;

const WIFI_STATUS_SIZE: Size = Size::new(166, 22);
const WIFI_STATUS_POINT: Point = Point::new(0, 0);

const TIME_POINT: Point = Point::new(WIFI_STATUS_SIZE.width as i32, WIFI_STATUS_POINT.y as i32);
const TIME_SIZE: Size = Size::new(
    ORIENTATION.width() - TIME_POINT.x as u32,
    WIFI_STATUS_SIZE.height,
);

const BELOW_WIFI_TIME: u32 = max_u32(
    WIFI_STATUS_POINT.y as u32 + WIFI_STATUS_SIZE.height,
    TIME_POINT.y as u32 + TIME_SIZE.height,
);
const FIGURE_TILES: TileGrid = TileGrid::new(
    Point::new(0, BELOW_WIFI_TIME as i32),
    Size::new(ORIENTATION.width(), ORIENTATION.height() - BELOW_WIFI_TIME),
    3,
    3,
);

// ── Projection ───────────────────────────────────────────────────────────────

//todo000 review projections.
const PROJECTION: NegXProjection = NegXProjection {
    center_x: 120.0,
    baseline_y: 300.0,
    scale: 1.25,
};

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

// ── Binary entry point ────────────────────────────────────────────────────────

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    DeviceEnvoy(Error),
    Core(CoreError),
    Cyd(CydError),
    Mark(MarkError),
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

    // The shared pixel buffer must hold the largest frame: a dance tile or the full-width text band.
    const BUFFER_PIXEL_COUNT: usize = max_usize(
        (WIFI_STATUS_SIZE.width * WIFI_STATUS_SIZE.height) as usize,
        FIGURE_TILES.max_tile_pixel_count(),
    );
    static CYD_STATIC: CydStatic<BUFFER_PIXEL_COUNT> = Cyd::new_static();
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
        ORIENTATION,
        BACKGROUND,
        FOREGROUND,
        &FONT_6X10,
    )?;
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

    // One frame owns the Wi-Fi status band: it is reused across every connect
    // event and for the final "WiFi OK". The async closure borrows it directly
    // (no `RefCell`), and dropping it afterward hands `cyd` back to the clock loop.
    let mut wifi_status_frame = cyd.frame_mut(WIFI_STATUS_SIZE);
    let stack = wifi_auto
        .connect(
            &mut force_portal_button,
            async |wifi_auto_event| -> Result<(), CydError> {
                let message = match wifi_auto_event {
                    WifiAutoEvent::CaptivePortalReady => "setup CydDance",
                    WifiAutoEvent::Connecting { .. } => "connecting",
                    WifiAutoEvent::ConnectionFailed => "connect failed",
                };
                info!("WiFi: {message}");
                // Draw the Wi-Fi status into the top band, leaving the time slot
                // blank, until the clock loop below takes over the band.
                //todo0000 fix this
                wifi_status_frame.clear(cyd.background_565());
                wifi_status_frame.write_text(message, WIFI_STATUS_POINT);
                wifi_status_frame.flush()?;
                Ok(())
            },
        )
        .await?;

    wifi_status_frame.clear(cyd.background_565());
    wifi_status_frame.write_text("WiFi OK", WIFI_STATUS_POINT);
    wifi_status_frame.flush()?;
    drop(wifi_status_frame);

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

    // todo000 should shared/generic code with wasm start here?

    let linkage_view = LINKAGE.view();

    loop {
        let tick = clock_sync.wait_for_tick().await;
        // todo could add ClockTime functionality to ClockSyncTick
        let clock = ClockTime::new(&tick.local_time);
        info!("tick {}", clock.text_24h());

        let params = clock.linkage_params();
        let time_text = clock.text_12h();

        //todo0000 there is not point in the write_text method. It also starts at the upper left and respects "\n" like device-envoy.
        let mut time_frame = cyd.frame_mut(TIME_SIZE);
        time_frame.write_text(time_text.as_str(), Point::new(0, 0));
        //todo0000 this is where point goes. I was confused before.
        //todo000 let's call this flush and not have a flush w/o a point.
        time_frame.flush_at(TIME_POINT)?;

        // Shared linkage rendering path, tiled for CYD.

        for tile in FIGURE_TILES.tiles() {
            let mut tile_frame = cyd.frame_mut(tile.size);

            // Dance-specific background overlay.
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
            // Dance-specific foreground overlay: placards hang from hand marks.
            let right_hand_pose = draw_items.pose_by_mark_name("rMid2")?;
            let left_hand_pose = draw_items.pose_by_mark_name("lMid2")?;
            let mut target = TranslatedDrawTarget::new(&mut tile_frame, tile.top_left);
            draw_hanging_placard(
                &mut target,
                pose_to_point(left_hand_pose),
                clock.hour_12() as u32,
            );
            draw_hanging_placard(
                &mut target,
                pose_to_point(right_hand_pose),
                clock.minute() as u32,
            );

            tile_frame.flush_at(tile.top_left)?;
        }
    }
}

// ── Clock time ────────────────────────────────────────────────────────────────

/// App-local wall-clock time. Collects the time-derived behavior the dance
/// needs: 12-hour display, formatted strings, and the normalized linkage
/// parameters. No alloc.
struct ClockTime {
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl ClockTime {
    fn new(local_time: &OffsetDateTime) -> Self {
        Self {
            hours: local_time.hour(),
            minutes: local_time.minute(),
            seconds: local_time.second(),
        }
    }

    /// 12 for midnight/noon, 1–11 otherwise.
    fn hour_12(&self) -> u8 {
        match self.hours % 12 {
            0 => 12,
            other => other,
        }
    }

    fn minute(&self) -> u8 {
        self.minutes
    }

    /// Format a 12-hour clock string with AM/PM, right-justified to 11
    /// characters (e.g. " 5:04:32 PM" or "12:04:32 PM").
    fn text_12h(&self) -> heapless::String<16> {
        let suffix = if self.hours % 24 < 12 { "AM" } else { "PM" };
        let mut text = heapless::String::new();
        core::fmt::write(
            &mut text,
            format_args!(
                "{:>2}:{:02}:{:02} {suffix}",
                self.hour_12(),
                self.minutes,
                self.seconds
            ),
        )
        .expect("clock string fits in 16 bytes");
        text
    }

    /// Format a 24-hour `HH:MM:SS` clock string.
    fn text_24h(&self) -> heapless::String<9> {
        let mut text = heapless::String::new();
        core::fmt::write(
            &mut text,
            format_args!("{:02}:{:02}:{:02}", self.hours, self.minutes, self.seconds),
        )
        .expect("clock string fits in 9 bytes");
        text
    }

    // todo000 seems overly complex.
    fn linkage_params(&self) -> [f32; 3] {
        const CLOCK_HAND_PARAM_TURN: f32 = 0.25;
        const EYES_FORWARD_PARAM: f32 = 0.5;
        const RIGHT_ARM_12_PARAM: f32 = 0.4375;
        const LEFT_ARM_12_PARAM: f32 = 0.5625;
        let second_phase = self.seconds as f32 / 60.0;
        let minute_phase = (self.minutes as f32 + second_phase) / 60.0;
        let hour_phase = ((self.hours % 12) as f32 + minute_phase) / 12.0;
        let signed_hour_phase = signed_clock_phase(hour_phase);
        [
            wrap_unit(EYES_FORWARD_PARAM + second_phase * CLOCK_HAND_PARAM_TURN),
            wrap_unit(RIGHT_ARM_12_PARAM + minute_phase * CLOCK_HAND_PARAM_TURN),
            wrap_unit(LEFT_ARM_12_PARAM + signed_hour_phase * CLOCK_HAND_PARAM_TURN),
        ]
    }
}

// todo000 move these into impl ClockTime as static methods?
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

// ── Dance-specific overlay drawing ───────────────────────────────────────────

// All overlay drawing happens against a `DrawTarget` whose coordinates are in
// physical-screen space; a `TranslatedDrawTarget` subtracts the tile origin so
// these functions never need to know they are rendering into a tile.

/// Draw a short number string centered (both axes) on `center`.
fn draw_centered_number<D>(target: &mut D, text: &str, center: Point, color: Rgb565)
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    let text_style = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Middle)
        .build();
    Text::with_text_style(
        text,
        center,
        MonoTextStyle::new(&FONT_6X10, color),
        text_style,
    )
    .draw(target)
    .expect("drawing to an Infallible target cannot fail");
}

/// Draw the clock dial's 12 / 3 / 6 / 9 hour markers around the figure.
fn draw_dial<D>(target: &mut D)
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    const DIAL_COLOR: Rgb888 = Rgb888::CSS_DARK_SLATE_GRAY; // muted teal-gray (47, 79, 79)
    const DIAL_CENTER_SCREEN: Point = Point::new(120, 178);
    const DIAL_RADIUS_X: i32 = 100;
    const DIAL_RADIUS_Y: i32 = 118;

    let dial565 = Cyd::rgb565(DIAL_COLOR);
    let center_x = DIAL_CENTER_SCREEN.x;
    let center_y = DIAL_CENTER_SCREEN.y;
    draw_centered_number(
        target,
        "12",
        Point::new(center_x, center_y - DIAL_RADIUS_Y),
        dial565,
    );
    draw_centered_number(
        target,
        "3",
        Point::new(center_x + DIAL_RADIUS_X, center_y),
        dial565,
    );
    draw_centered_number(
        target,
        "6",
        Point::new(center_x, center_y + DIAL_RADIUS_Y),
        dial565,
    );
    draw_centered_number(
        target,
        "9",
        Point::new(center_x - DIAL_RADIUS_X, center_y),
        dial565,
    );
}

/// Draw a hanging number sign anchored at `anchor` (a hand mark in dance
/// coordinates).  The sign is a fixed size and always shows two digits.  It
/// hangs straight down via a short vertical hook from the hand, then a triangle
/// splays out to the sign's two top corners.  `number` is shown modulo 100.
fn draw_hanging_placard<D>(target: &mut D, anchor: Point, number: u32)
where
    D: DrawTarget<Color = Rgb565, Error = Infallible>,
{
    const PLACARD_W: i32 = 34;
    const PLACARD_H: i32 = 20;
    const PLACARD_BORDER_PX: i32 = 2;
    const HANGER_PX: i32 = 2;
    const HANGER_HOOK: i32 = 7;
    const HANGER_TRIANGLE: i32 = 22;

    let placard_border = Cyd::rgb565(FIGURE);
    let placard_text = Cyd::rgb565(FIGURE);
    let placard_fill = Cyd::rgb565(PLACARD_FILL);

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

    // Fill plus an inside-aligned stroke reproduces the original 2 px inner border.
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

    let mut value_text = heapless::String::<4>::new();
    core::fmt::write(&mut value_text, format_args!("{:02}", number % 100))
        .expect("two-digit placard value fits in 4 bytes");
    let card_center = Point::new(card_left + PLACARD_W / 2, card_top + PLACARD_H / 2);
    draw_centered_number(target, &value_text, card_center, placard_text);
}

// todo0000 shouldn't be needed.
fn pose_to_point(pose: Pose) -> Point {
    to_point(PROJECTION.project_pos(pose))
}
