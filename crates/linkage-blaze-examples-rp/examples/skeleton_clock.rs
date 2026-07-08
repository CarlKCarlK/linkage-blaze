#![cfg(feature = "wifi")]
#![no_std]
#![no_main]
#![allow(clippy::future_not_send, reason = "single-threaded")]

use core::{cell::RefCell, convert::Infallible};

use defmt::info;
use defmt_rtt as _;
use device_envoy_rp::{
    Error, Result,
    button::{ButtonRp, PressedTo},
    clock_sync::{ClockSyncRp, ClockSyncStaticRp, CoreError, ONE_SECOND},
    cyd::{
        CydDisplay as _, CydDisplayRp, CydError, CydRp, CydStaticRp, DEFAULT_DISPLAY_SPI_HZ,
        tiling::rectangle_pixel_count,
    },
    flash_block::FlashBlockRp,
    wifi_auto::{
        WifiAutoEvent, WifiAutoRp,
        fields::{TimezoneField, TimezoneFieldStatic},
    },
};
use embassy_executor::Spawner;
use linkage_blaze_core::examples::skeleton_clock::{
    self, BACKGROUND, FIGURE_TILE_GRID, FOREGROUND, ORIENTATION, TOP_FONT, WIFI_STATUS_RECTANGLE,
    skeleton_clock,
};
use panic_probe as _;

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(spawner: Spawner) -> Result<Infallible, MainError> {
    info!("Starting CYD skeleton-clock with WiFi on RP Pico");

    let p = embassy_rp::init(Default::default());

    const BUFFER_PIXEL_COUNT: usize = max_usize(
        rectangle_pixel_count(WIFI_STATUS_RECTANGLE),
        FIGURE_TILE_GRID.max_tile_pixel_count(),
    );
    static CYD_STATIC: CydStaticRp<BUFFER_PIXEL_COUNT> = CydRp::new_static();
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
        ORIENTATION, // orientation
        BACKGROUND,  // background
        FOREGROUND,  // foreground
        &TOP_FONT,   // font
    )?;
    info!("CYD display initialized");

    skeleton_clock::skeleton_clock_splash(&mut display).await?;

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
        "SkelClock",           // ssid
        [timezone_field],      // custom_fields
        spawner,               // spawner
    )?;

    let background565 = display.background_565();
    let wifi_status_frame = RefCell::new(display.frame_mut(WIFI_STATUS_RECTANGLE));
    let stack = wifi_auto
        .connect(
            &mut button_watch15,
            async |wifi_auto_event| -> Result<(), Error> {
                let message = match wifi_auto_event {
                    WifiAutoEvent::CaptivePortalReady => "WiFi: setup SkelClock",
                    WifiAutoEvent::Connecting { .. } => "WiFi: connecting",
                    WifiAutoEvent::ConnectionFailed => "WiFi: connect failed",
                };
                if wifi_status_frame
                    .borrow_mut()
                    .fill(background565)
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
        .fill(background565)
        .write_text("WiFi: OK")
        .flush()?;
    drop(wifi_status_frame);
    info!("WiFi connected");

    let timezone_offset_minutes = timezone_field
        .offset_minutes()?
        .ok_or(Error::MissingCustomWifiAutoField)?;

    static CLOCK_SYNC_STATIC: ClockSyncStaticRp = ClockSyncRp::new_static();
    let clock_sync = ClockSyncRp::new(
        &CLOCK_SYNC_STATIC,
        stack,
        timezone_offset_minutes,
        Some(ONE_SECOND),
        spawner,
    )?;
    info!("clock sync ready; entering skeleton-clock loop");

    Ok(skeleton_clock(&mut display, &clock_sync).await?)
}

const fn max_usize(first: usize, second: usize) -> usize {
    if first > second { first } else { second }
}

#[derive(Debug)]
enum MainError {
    DeviceEnvoy,
    Core,
    Cyd,
    SkeletonClock,
}

impl From<device_envoy_rp::Error> for MainError {
    fn from(_error: device_envoy_rp::Error) -> Self {
        Self::DeviceEnvoy
    }
}

impl From<CoreError> for MainError {
    fn from(_error: CoreError) -> Self {
        Self::Core
    }
}

impl From<CydError> for MainError {
    fn from(_error: CydError) -> Self {
        Self::Cyd
    }
}

impl From<skeleton_clock::Error<CydError>> for MainError {
    fn from(_error: skeleton_clock::Error<CydError>) -> Self {
        Self::SkeletonClock
    }
}
