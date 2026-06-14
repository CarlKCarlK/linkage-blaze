#![no_std]
#![no_main]

// todo00 may be repeating display code
// todo00 should start with calibrate to flash
// todo00 wifi's memory needs means it can't have a full screen memory. I'm not sure what it is doing instead.

use core::{cell::RefCell, convert::Infallible, fmt};

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
use esp_backtrace as _;
use log::info;

mod display;

use display::{ClockTime, CydClockDisplay, CydClockDisplayError};

esp_bootloader_esp_idf::esp_app_desc!();

enum MainError {
    DeviceEnvoy(Error),
    Display(CydClockDisplayError),
}

impl fmt::Debug for MainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MainError::DeviceEnvoy(error) => {
                formatter.debug_tuple("DeviceEnvoy").field(error).finish()
            }
            MainError::Display(error) => formatter.debug_tuple("Display").field(error).finish(),
        }
    }
}

impl From<Error> for MainError {
    fn from(error: Error) -> Self {
        MainError::DeviceEnvoy(error)
    }
}

impl From<CoreError> for MainError {
    fn from(error: CoreError) -> Self {
        MainError::DeviceEnvoy(error.into())
    }
}

impl From<CydClockDisplayError> for MainError {
    fn from(error: CydClockDisplayError) -> Self {
        MainError::Display(error)
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

    let display = RefCell::new(CydClockDisplay::new(
        p.SPI2, p.GPIO14, p.GPIO13, p.GPIO12, p.GPIO15, p.GPIO2, p.GPIO4, p.GPIO21,
    )?);
    display.borrow_mut().show("booting", None)?;

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

    let display_ref = &display;
    let stack = wifi_auto
        .connect(&mut force_portal_button, |wifi_auto_event| {
            let display_ref = display_ref;
            async move {
                let wifi_mode = match wifi_auto_event {
                    WifiAutoEvent::CaptivePortalReady => "setup CydClock",
                    WifiAutoEvent::Connecting { .. } => "connecting",
                    WifiAutoEvent::ConnectionFailed => "connect failed",
                };
                display_ref
                    .borrow_mut()
                    .show(wifi_mode, None)
                    .map_err(|_| Error::FormatError)?;
                Ok(())
            }
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
    }
}
