#![no_std]
#![no_main]

// todo000 can't we algorithmly clear the screen?
// todo000 can't we allocate the largest buffer and then use it for smaller things?
// todo000 get wifi portal and drawing work at the same time.
// todo different color hands
// todo00 may be repeating display code
// todo00 should start with calibrate to flash
// todo00 wifi's memory needs means it can't have a full screen memory. I'm not sure what it is doing instead.

use core::{cell::RefCell, convert::Infallible};

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
use embedded_graphics::pixelcolor::{Rgb888, WebColors};
use esp_backtrace as _;
use linkage_blaze_cyd::{Cyd, CydDisplayConfig};
use log::info;
use static_cell::StaticCell;

mod display;

use display::{ClockTime, CydClockDisplay, CydClockDisplayError};

const BLACK: Rgb888 = Rgb888::CSS_BLACK;

esp_bootloader_esp_idf::esp_app_desc!();

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[allow(dead_code)]
#[derive(Debug, derive_more::From)]
enum MainError {
    DeviceEnvoy(Error),
    Cyd(linkage_blaze_cyd::CydError),
    Display(CydClockDisplayError),
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

    info!("Starting CYD clock with WiFi");

    let mut cyd = Cyd::new_display(
        p.SPI2,
        p.GPIO14,
        p.GPIO13,
        p.GPIO12,
        p.GPIO15,
        p.GPIO2,
        p.GPIO4,
        p.GPIO21,
        CydDisplayConfig::LANDSCAPE,
    )?;
    cyd.fill(Cyd::rgb565(BLACK))?;
    static DISPLAY: StaticCell<RefCell<CydClockDisplay>> = StaticCell::new();
    let display = &*DISPLAY.init(RefCell::new(CydClockDisplay::new(cyd)));
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

    let stack = wifi_auto
        .connect(&mut force_portal_button, |wifi_auto_event| async move {
            let wifi_mode = match wifi_auto_event {
                WifiAutoEvent::CaptivePortalReady => "setup CydClock",
                WifiAutoEvent::Connecting { .. } => "connecting",
                WifiAutoEvent::ConnectionFailed => "connect failed",
            };
            info!("WiFi mode: {wifi_mode}");
            if let Err(error) = display.borrow_mut().show(wifi_mode, None) {
                info!("WiFi mode display failed: {error:?}");
            }
            Ok(())
        })
        .await?;

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

    info!("WiFi connected; drawing clock");
    display.borrow_mut().show("connected", None)?;

    loop {
        let tick = clock_sync.wait_for_tick().await;
        let local_time = tick.local_time;
        let clock_time =
            ClockTime::new(local_time.hour(), local_time.minute(), local_time.second())
                .map_err(|_| MainError::DeviceEnvoy(Error::FormatError))?;
        display.borrow_mut().show("connected", Some(&clock_time))?;
        info!(
            "time {:02}:{:02}:{:02}",
            local_time.hour(),
            local_time.minute(),
            local_time.second()
        );
    }
}
