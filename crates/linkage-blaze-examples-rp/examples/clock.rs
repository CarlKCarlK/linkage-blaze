#![cfg(feature = "wifi")]
#![no_std]
#![no_main]
#![allow(clippy::future_not_send, reason = "single-threaded")]

use core::{cell::RefCell, convert::Infallible, fmt};

use defmt::info;
use defmt_rtt as _;
use device_envoy_core::cyd::display::CydFrame;
use device_envoy_core::wifi_auto::WifiAuto;
use device_envoy_rp::{
    Result,
    button::{ButtonRp, PressedTo},
    clock_sync::{ClockSyncRp, ClockSyncStaticRp, CoreError, ONE_SECOND},
    cyd::{
        CydDisplay as _, CydDisplayRp, CydRp, CydStaticRp, DEFAULT_DISPLAY_SPI_HZ,
        Error as CydError,
    },
    flash_block::FlashBlockRp,
    wifi_auto::{
        WifiAutoEvent, WifiAutoRp,
        fields::{TimezoneField, TimezoneFieldStatic},
    },
};
use embassy_executor::Spawner;
use linkage_blaze::examples::clock::{
    self, BACKGROUND_COLOR, Exit, FOREGROUND_COLOR, MAX_FRAME_PIXEL_COUNT, ORIENTATION,
    WIFI_STATUS_FONT, WIFI_STATUS_RECTANGLE,
};
use panic_probe as _;

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, Error> {
    info!("Starting CYD clock with WiFi on RP Pico");

    let p = embassy_rp::init(Default::default());

    static CYD_STATIC: CydStaticRp<MAX_FRAME_PIXEL_COUNT> = CydRp::new_static();
    let mut display = CydDisplayRp::new(
        &CYD_STATIC, // statics
        p.SPI0,      // display_spi
        p.PIN_18,    // display_sck_pin
        p.PIN_19,    // display_mosi_pin
        p.PIN_16,    // display_miso_pin
        p.PIN_17,    // display_cs_pin
        p.PIN_20,    // display_dc_pin
        p.PIN_21,    // display_rst_pin
        p.PIN_22,    // display_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        ORIENTATION,       // orientation
        BACKGROUND_COLOR,  // background_color
        FOREGROUND_COLOR,  // foreground_color
        &WIFI_STATUS_FONT, // font
    )?;
    info!("CYD display initialized");

    clock::splash(&mut display).await?;

    let [wifi_auto_flash_block, timezone_flash_block] = FlashBlockRp::new_array::<2>(p.FLASH)?;

    static TIMEZONE_FIELD_STATIC: TimezoneFieldStatic = TimezoneField::new_static();
    let timezone_field = TimezoneField::new(&TIMEZONE_FIELD_STATIC, timezone_flash_block);
    let mut button_watch15 = ButtonRp::new(p.PIN_15, PressedTo::Ground);

    let wifi_auto = WifiAutoRp::new(
        p.PIN_23,              // power_pin
        p.PIN_24,              // data_pin
        p.PIN_25,              // chip_select_pin
        p.PIN_29,              // clock_pin
        p.PIO0,                // pio
        p.DMA_CH0,             // dma_channel
        wifi_auto_flash_block, // credential_store
        "CydClock",            // ssid
        [timezone_field],      // custom_fields
        spawner,               // spawner
    )?;

    let wifi_status_frame = RefCell::new(display.frame_mut(WIFI_STATUS_RECTANGLE));
    let stack = wifi_auto
        .connect(
            &mut button_watch15,
            async |wifi_auto_event| -> Result<(), Error> {
                let message = match wifi_auto_event {
                    WifiAutoEvent::CaptivePortalReady => "WiFi setup",
                    WifiAutoEvent::Connecting { .. } => "WiFi ...",
                    WifiAutoEvent::ConnectionFailed => "WiFi fail",
                };
                if wifi_status_frame
                    .borrow_mut()
                    .clear()
                    .write_text(message)
                    .flush()
                    .is_err()
                {
                    info!("WiFi status display failed");
                }
                info!("WiFi: {}", message);
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
        .ok_or(device_envoy_rp::Error::MissingCustomWifiAutoField)?;

    static CLOCK_SYNC_STATIC: ClockSyncStaticRp = ClockSyncRp::new_static();
    let clock_sync = ClockSyncRp::new(
        &CLOCK_SYNC_STATIC,
        stack,
        timezone_offset_minutes,
        Some(ONE_SECOND),
        spawner,
    )?;
    info!("clock sync ready; entering clock loop");

    match clock::run(&mut display, &clock_sync, &mut button_watch15).await? {
        Exit::ResetWifi => {
            wifi_auto.reset_to_captive_portal()?;
            cortex_m::peripheral::SCB::sys_reset();
        }
    }
}

#[derive(derive_more::From)]
enum Error {
    DeviceEnvoy(device_envoy_rp::Error),
    Core(CoreError),
    Cyd(CydError),
    Clock(clock::Error<CydError>),
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceEnvoy(error) => formatter.debug_tuple("DeviceEnvoy").field(error).finish(),
            Self::Core(error) => formatter.debug_tuple("Core").field(error).finish(),
            Self::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
            Self::Clock(error) => formatter.debug_tuple("Clock").field(error).finish(),
        }
    }
}
