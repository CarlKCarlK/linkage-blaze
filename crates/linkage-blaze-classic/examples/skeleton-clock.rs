#![no_std]
#![no_main]

// todo000 we need to use color and/or size to tell hours from minutes
// todo000 we need some wasm preview

use core::convert::Infallible;

use device_envoy_esp::{
    Error,
    button::{ButtonEsp, PressedTo},
    clock_sync::{ClockSyncEsp, ClockSyncStaticEsp, CoreError, ONE_SECOND},
    flash_block::FlashBlockEsp,
    init_and_start,
    wifi_auto::{
        WifiAuto as _, WifiAutoEsp, WifiAutoEvent,
        fields::{TimezoneField, TimezoneFieldStatic},
    },
};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_cyd::{CydEsp, CydError, CydStaticEsp, tiling::max_usize};
use linkage_blaze_example_core::skeleton_clock::{
    BACKGROUND, FIGURE_TILES, FOREGROUND, ORIENTATION, SkeletonClockError, TOP_FONT,
    WIFI_STATUS_POINT, WIFI_STATUS_SIZE, skeleton_clock,
};
use log::info;

// ── Binary entry point ────────────────────────────────────────────────────────

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    DeviceEnvoy(Error),
    Core(CoreError),
    CydEsp(CydError),
    SkeletonClock(SkeletonClockError<CydError>),
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD skeleton-clock with WiFi");

    // The shared pixel buffer must hold the largest frame: a skeleton-clock tile
    // or a wi-fi or time message.
    const BUFFER_PIXEL_COUNT: usize = max_usize(
        (WIFI_STATUS_SIZE.width * WIFI_STATUS_SIZE.height) as usize,
        FIGURE_TILES.max_tile_pixel_count(),
    );
    static CYD_STATIC: CydStaticEsp<BUFFER_PIXEL_COUNT> = CydEsp::new_static();
    let mut cyd = CydEsp::new_display_only(
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
        &TOP_FONT,
    )?;
    info!("CYD display initialized");

    let [wifi_auto_flash_block, timezone_flash_block] = FlashBlockEsp::new_array::<2>(p.FLASH)?;

    static TIMEZONE_FIELD_STATIC: TimezoneFieldStatic = TimezoneField::new_static();
    let timezone_field = TimezoneField::new(&TIMEZONE_FIELD_STATIC, timezone_flash_block);
    let mut force_portal_button = ButtonEsp::new(p.GPIO0, PressedTo::Ground);

    let wifi_auto = WifiAutoEsp::new(
        p.WIFI,
        wifi_auto_flash_block,
        "SkelClock",
        [timezone_field],
        spawner,
    )?;

    let mut wifi_status_frame = cyd.frame_mut(WIFI_STATUS_SIZE);
    let stack = wifi_auto
        .connect(
            &mut force_portal_button,
            async |wifi_auto_event| -> Result<(), CydError> {
                let message = match wifi_auto_event {
                    WifiAutoEvent::CaptivePortalReady => "WiFi: setup SkelClock",
                    WifiAutoEvent::Connecting { .. } => "WiFi: connecting",
                    WifiAutoEvent::ConnectionFailed => "WiFi: connect failed",
                };
                wifi_status_frame
                    .clear()
                    .write_text(message)
                    .flush_at(WIFI_STATUS_POINT)?;
                info!("WiFi: {message}");
                Ok(())
            },
        )
        .await?;

    wifi_status_frame
        .clear()
        .write_text("WiFi: OK")
        .flush_at(WIFI_STATUS_POINT)?;
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
    info!("clock sync ready; entering skeleton-clock loop");

    // Hand off to the device-agnostic render loop.
    Ok(skeleton_clock(&mut cyd, &clock_sync).await?)
}
