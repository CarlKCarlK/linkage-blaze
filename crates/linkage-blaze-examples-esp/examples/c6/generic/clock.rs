#![no_std]
#![no_main]

use core::{cell::RefCell, convert::Infallible};

use device_envoy_core::cyd::display::CydFrame;
use device_envoy_esp::cyd::{
    CydDisplay as _, CydDisplayEsp, CydEsp, CydStaticEsp, DEFAULT_DISPLAY_SPI_HZ, Error as CydError,
};
use device_envoy_esp::{
    Error as DeviceEnvoyError,
    button::PressedTo,
    button_watch,
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
    WIFI_STATUS_RECTANGLE, run, splash,
};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

button_watch! {
    ButtonWatch {
        pin: GPIO6,
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, Error> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD clock with WiFi on ESP32-C6 / generic");

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

    splash(&mut display).await?;

    let [wifi_auto_flash_block, timezone_flash_block] = FlashBlockEsp::new_array::<2>(p.FLASH)?;

    static TIMEZONE_FIELD_STATIC: TimezoneFieldStatic = TimezoneField::new_static();
    let timezone_field = TimezoneField::new(&TIMEZONE_FIELD_STATIC, timezone_flash_block);
    let button_watch = ButtonWatch::new(p.GPIO6, PressedTo::Ground, spawner).await?;

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
            &mut *button_watch,
            async |wifi_auto_event| -> Result<(), DeviceEnvoyError> {
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
        .ok_or(DeviceEnvoyError::MissingCustomWifiAutoField)?;

    static CLOCK_SYNC_STATIC: ClockSyncStaticEsp = ClockSyncEsp::new_static();
    let clock_sync = ClockSyncEsp::new(
        &CLOCK_SYNC_STATIC,
        stack,
        timezone_offset_minutes,
        Some(ONE_SECOND),
        spawner,
    )?;
    info!("clock sync ready; entering clock loop");

    match run(&mut display, &clock_sync, &mut *button_watch).await? {
        clock::Exit::ResetWifi => {
            wifi_auto.reset_to_captive_portal()?;
            device_envoy_esp::esp_hal::system::software_reset();
        }
    }
}

// Derived Debug reads these payloads at runtime, but dead_code analysis ignores
// derived impls under -D warnings.
#[derive(derive_more::From)]
enum Error {
    DeviceEnvoy(DeviceEnvoyError),
    Core(CoreError),
    CydEsp(CydError),
    Clock(clock::Error<CydError>),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DeviceEnvoy(error) => formatter.debug_tuple("DeviceEnvoy").field(error).finish(),
            Self::Core(error) => formatter.debug_tuple("Core").field(error).finish(),
            Self::CydEsp(error) => formatter.debug_tuple("CydEsp").field(error).finish(),
            Self::Clock(error) => formatter.debug_tuple("Clock").field(error).finish(),
        }
    }
}
