#![no_std]
#![no_main]

use core::convert::Infallible;

use device_envoy_core::cyd::{Cyd as _, CydDisplay, display::CydFrame};
use device_envoy_esp::cyd::DEFAULT_DISPLAY_SPI_HZ;
use device_envoy_esp::{
    button::PressedTo,
    button_watch,
    cyd::{CydEspOneSpi, CydStaticEsp, DEFAULT_FONT, Orientation},
    flash_block::{FlashBlock as _, FlashBlockEsp},
    init_and_start,
};
use embassy_executor::Spawner;
use esp_backtrace as _;
use linkage_blaze_core::examples::armatron::{self, BACKGROUND, FOREGROUND, run};
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
    info!("Starting CYD armatron loop (one-SPI) on ESP32-C61 / generic");

    let [mut calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let button_watch = ButtonWatch::new(p.GPIO6, PressedTo::Ground, spawner).await?;

    static CYD_STATIC: CydStaticEsp<{ CydEspOneSpi::SCREEN_PIXELS }> = CydEspOneSpi::new_static();
    let mut cyd = CydEspOneSpi::new(
        &CYD_STATIC, // statics
        p.SPI2,      // spi
        p.GPIO1,     // sck_pin
        p.GPIO2,     // mosi_pin
        p.GPIO3,     // miso_pin
        p.GPIO4,     // lcd_cs_pin
        p.GPIO5,     // lcd_dc_pin
        p.GPIO7,     // lcd_rst_pin
        p.GPIO8,     // lcd_backlight_pin
        DEFAULT_DISPLAY_SPI_HZ,
        p.GPIO12,               // touch_cs_pin
        p.GPIO13,               // touch_irq_pin
        Orientation::Landscape, // orientation
        //todo000 should rename with _COLOR
        BACKGROUND,                   // background
        FOREGROUND,                   // foreground
        &DEFAULT_FONT,                // font
        &mut calibration_flash_block, // calibration_flash_block
        &mut *button_watch,           // button_watch
    )
    .await?;
    info!("CYD display and touch initialized");
    match run(&mut cyd, &mut *button_watch).await? {
        // todo0000 can't this just be Exit (via module/namespace)
        armatron::Exit::CalibrationRequested => {
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
    DeviceEnvoy(device_envoy_esp::Error),
    Cyd(device_envoy_esp::cyd::Error), //todo0000 shouldn't this just be Error (plus module and namespace) (may no longer apply)
    Armatron(armatron::Error<device_envoy_esp::cyd::Error>), //todo0000 shouldn't this just be Error (plus module and namespace) (may no longer apply)
}

impl core::fmt::Debug for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DeviceEnvoy(error) => formatter.debug_tuple("DeviceEnvoy").field(error).finish(),
            Self::Cyd(error) => formatter.debug_tuple("Cyd").field(error).finish(),
            Self::Armatron(error) => formatter.debug_tuple("Armatron").field(error).finish(),
        }
    }
}
