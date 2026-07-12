#![no_std]
#![no_main]

// todo000 can't we allocate the largest buffer and then use it for smaller things?
// todo000 get wifi portal and drawing work at the same time.

use core::{cell::RefCell, convert::Infallible};

use device_envoy_esp::cyd::{
    CydDisplay as _, CydDisplayEsp, CydError, CydEsp, CydStaticEsp, DEFAULT_DISPLAY_SPI_HZ,
};
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
use linkage_blaze_core::examples::clock::{
    self, BACKGROUND, FOREGROUND, MAX_FRAME_PIXEL_COUNT, ORIENTATION, WIFI_STATUS_FONT,
    WIFI_STATUS_RECTANGLE, clock, clock_splash,
};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD clock with WiFi on ESP32-S3 / esp32-s3-devkitc-1-v1.0-n16r8");

    static CYD_STATIC: CydStaticEsp<MAX_FRAME_PIXEL_COUNT> = CydEsp::new_static();
    let mut display = CydDisplayEsp::new(
        &CYD_STATIC, // statics
        p.SPI2,      // display_spi
        p.GPIO1,     // display_sck_pin
        p.GPIO2,     // display_mosi_pin
        p.GPIO3,     // display_miso_pin
        p.GPIO4,     // display_cs_pin
        p.GPIO5,     // display_dc_pin
        p.GPIO7,     // display_rst_pin
        p.GPIO8,     // display_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        ORIENTATION,       // orientation
        BACKGROUND,        // background
        FOREGROUND,        // foreground
        &WIFI_STATUS_FONT, // font
    )?;
    info!("CYD display initialized");

    clock_splash(&mut display).await?;

    let [wifi_auto_flash_block, timezone_flash_block] = FlashBlockEsp::new_array::<2>(p.FLASH)?;

    static TIMEZONE_FIELD_STATIC: TimezoneFieldStatic = TimezoneField::new_static();
    let timezone_field = TimezoneField::new(&TIMEZONE_FIELD_STATIC, timezone_flash_block);
    let mut force_portal_button = ButtonEsp::new(p.GPIO6, PressedTo::Ground);

    let wifi_auto = WifiAutoEsp::new(
        p.WIFI,
        wifi_auto_flash_block,
        "CydClock",
        [timezone_field],
        spawner,
    )?;

    let wifi_status_frame = RefCell::new(display.frame_mut(WIFI_STATUS_RECTANGLE));
    let stack = wifi_auto
        .connect(
            &mut force_portal_button,
            async |wifi_auto_event| -> Result<(), Error> {
                let message = match wifi_auto_event {
                    WifiAutoEvent::CaptivePortalReady => "WiFi setup",
                    WifiAutoEvent::Connecting { .. } => "WiFi ...",
                    WifiAutoEvent::ConnectionFailed => "WiFi fail",
                };
                if let Err(error) = wifi_status_frame
                    .borrow_mut()
                    .clear()
                    .write_text(message)
                    .flush()
                {
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

    Ok(clock(&mut display, &clock_sync).await?)
}

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
