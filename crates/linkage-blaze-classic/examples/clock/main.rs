#![no_std]
#![no_main]

// todo000 can't we allocate the largest buffer and then use it for smaller things?
// todo000 get wifi portal and drawing work at the same time.
// TODO00000 the analog clock-hand rendering is commented out in the generic
// `clock` module; see the `TODO00000` there for what it needs to be ported.

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
use linkage_blaze_cyd::{CydError, CydEsp, CydStaticEsp};
use linkage_blaze_example_core::clock::{
    self, BACKGROUND, FOREGROUND, ORIENTATION, TIME_REGION, TOP_FONT, WIFI_STATUS_REGION, clock,
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
    Clock(clock::Error<CydError>),
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD clock with WiFi");

    // The shared pixel buffer must hold the largest frame we draw: the digital
    // time read-out or the wi-fi status line.
    const BUFFER_PIXEL_COUNT: usize =
        linkage_blaze_cyd::tiling::max_usize(WIFI_STATUS_REGION.pixel_count(), TIME_REGION.pixel_count());
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
        "CydClock",
        [timezone_field],
        spawner,
    )?;

    // A `RefCell` so the `FnMut` connect callback can capture the frame by shared
    // reference and mutate it through interior mutability on each event.
    let wifi_status_frame = core::cell::RefCell::new(cyd.frame_mut(WIFI_STATUS_REGION));
    let stack = wifi_auto
        .connect(
            &mut force_portal_button,
            async |wifi_auto_event| -> Result<(), Error> {
                let message = match wifi_auto_event {
                    WifiAutoEvent::CaptivePortalReady => "WiFi setup",
                    WifiAutoEvent::Connecting { .. } => "WiFi ...",
                    WifiAutoEvent::ConnectionFailed => "WiFi fail",
                };
                if let Err(error) = wifi_status_frame.borrow_mut().clear().write_text(message).flush() {
                    info!("WiFi status display failed: {error:?}");
                }
                info!("WiFi: {message}");
                Ok(())
            },
        )
        .await?;

    wifi_status_frame
        .borrow_mut()
        .clear()
        .write_text("WiFi OK")
        .flush()?;
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
    info!("clock sync ready; entering clock loop");

    // Hand off to the device-agnostic render loop.
    Ok(clock(&mut cyd, &clock_sync).await?)
}
