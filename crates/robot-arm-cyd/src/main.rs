#![no_std]
#![no_main]

use embedded_graphics::pixelcolor::RgbColor;
use esp_backtrace as _;
use esp_hal::{
    Config,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    spi,
    time::Instant,
};
use log::info;
use robot_arm_core::cyd::CydSim;
use static_cell::StaticCell;

use device_envoy_esp::Result;

mod display;
mod touch;

use display::{DisplayRect, Ili9341RectWriter, Ili9341Rotation, RectDisplay};
use touch::{TouchEvent, TouchInput, Xpt2046TouchInput};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal::main]
fn main() -> ! {
    let p = esp_hal::init(Config::default());
    esp_println::logger::init_logger(log::LevelFilter::Info);
    let delay = Delay::new();

    esp_println::println!("boot: after esp_hal::init");

    let _ = p;

    esp_println::println!("boot: entering keepalive test loop");

    loop {
        esp_println::println!("boot: alive");
        delay.delay_millis(1000);
    }
}

fn inner_main(p: esp_hal::peripherals::Peripherals) -> Result<()> {
    let delay = Delay::new();

    esp_println::println!("boot: before led init");

    // RGB LED diagnostic: Red=GPIO4 (used by display RST), Green=GPIO16, Blue=GPIO17 (active-low).
    // Use green LED since GPIO4 is needed for display reset pin.
    let mut led_green = Output::new(p.GPIO16, Level::High, OutputConfig::default());
    let _led_blue = Output::new(p.GPIO17, Level::High, OutputConfig::default());

    // Blink green to show boot checkpoint 1
    led_green.set_low();
    delay.delay_millis(100);
    led_green.set_high();
    delay.delay_millis(100);

    esp_println::println!("boot: before cyd sim init");

    // TODO0000 Verify CYD ESP32-2432S028R display and touch pinout on hardware.
    let _ = led_green;
    let _ = p;
    let _ = delay;

    Ok(())
}

fn flush_full_frame(
    display_rect_writer: &mut impl RectDisplay,
    cyd_sim: &CydSim,
    frame_buffer: &mut [u16; robot_arm_core::cyd::SCREEN_PIXELS],
) {
    let width = cyd_sim.width() as u16;
    let height = cyd_sim.height() as u16;
    for (pixel_index, pixel) in cyd_sim.pixels().iter().enumerate() {
        frame_buffer[pixel_index] = rgb565_word(pixel.r(), pixel.g(), pixel.b());
    }

    let full_frame = DisplayRect::new(0, 0, width, height);
    display_rect_writer.write_rect_rgb565(full_frame, frame_buffer);
}

const fn rgb565_word(red_5: u8, green_6: u8, blue_5: u8) -> u16 {
    ((red_5 as u16) << 11) | ((green_6 as u16) << 5) | (blue_5 as u16)
}
