#![no_std]
#![no_main]

use core::{convert::Infallible, fmt};

use device_envoy_core::cyd::{Cyd as _, CydDisplay, display::CydFrame};
use device_envoy_esp::cyd::{self, DEFAULT_DISPLAY_SPI_HZ};
use device_envoy_esp::{
    Error as DeviceEnvoyError,
    button::PressedTo,
    button_watch,
    cyd::{CydEspOneSpi, CydStaticEsp, DEFAULT_FONT, Orientation},
    flash_block::{FlashBlock as _, FlashBlockEsp},
    init_and_start,
};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze::examples::armatron::{self, BACKGROUND_COLOR, Exit, FOREGROUND_COLOR};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

button_watch! {
    ButtonWatch {
        pin: GPIO18,
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
    info!("Starting CYD armatron loop (one-SPI) on ESP32-C2 / generic");

    let [mut calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let button_watch = ButtonWatch::new(p.GPIO18, PressedTo::Ground, spawner).await?;

    static CYD_STATIC: CydStaticEsp<{ CydEspOneSpi::SCREEN_PIXELS }> = CydEspOneSpi::new_static();
    let mut cyd = CydEspOneSpi::new(
        &CYD_STATIC, // statics
        p.SPI2,      // spi
        p.GPIO6,     // sck_pin
        p.GPIO7,     // mosi_pin
        p.GPIO2,     // miso_pin
        p.GPIO10,    // lcd_cs_pin
        p.GPIO3,     // lcd_dc_pin
        p.GPIO4,     // lcd_rst_pin
        p.GPIO5,     // lcd_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        p.GPIO0,                      // touch_cs_pin
        p.GPIO1,                      // touch_irq_pin
        Orientation::Landscape,       // orientation
        BACKGROUND_COLOR,             // background_color
        FOREGROUND_COLOR,             // foreground_color
        &DEFAULT_FONT,                // font
        &mut calibration_flash_block, // calibration_flash_block
        &mut *button_watch,           // button_watch
    )
    .await?;
    info!("CYD display and touch initialized");
    match armatron::run(&mut cyd, &mut *button_watch).await? {
        Exit::CalibrationRequested => {
            calibration_flash_block.clear()?;
            let mut frame = cyd.display().full_frame_mut();
            frame.clear().write_text("rebooting").flush()?;
            info!("Restarting");
            esp_hal::system::software_reset();
        }
    }
}

#[derive(derive_more::From)]
enum Error {
    DeviceEnvoy(DeviceEnvoyError),
    Cyd(cyd::Error),
    Armatron(armatron::Error<cyd::Error>),
}

impl fmt::Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceEnvoy(error) => formatter.debug_tuple("DeviceEnvoy").field(error).finish(),
            Self::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
            Self::Armatron(error) => formatter.debug_tuple("Armatron").field(error).finish(),
        }
    }
}
