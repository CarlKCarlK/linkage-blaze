#![no_std]
#![no_main]

use core::convert::Infallible;

use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::pixelcolor::RgbColor;
use esp_backtrace as _;
use esp_hal::{
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    spi,
};
use log::info;
use robot_arm_core::cyd::CydSim;

use device_envoy_esp::{Result, init_and_start};

mod display;
mod touch;

use display::{DisplayRect, Ili9341RectWriter, Ili9341Rotation, RectDisplay};
use touch::{TouchEvent, TouchInput, Xpt2046TouchInput};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible> {
    init_and_start!(p);

    esp_println::logger::init_logger(log::LevelFilter::Info);

    // TODO0000 Verify CYD ESP32-2432S028R display and touch pinout on hardware.
    let mut cyd_sim = CydSim::new();
    let width = cyd_sim.width() as u16;
    let height = cyd_sim.height() as u16;

    // Common CYD TFT wiring: SCK=GPIO14, MOSI=GPIO13, MISO=GPIO12, CS=GPIO15,
    // DC=GPIO2, RST=GPIO4, BL=GPIO21.
    let spi_config = spi::master::Config::default()
        .with_frequency(esp_hal::time::Rate::from_hz(26_000_000))
        .with_mode(spi::Mode::_0);
    let spi = spi::master::Spi::new(p.SPI2, spi_config)
        .map_err(device_envoy_esp::Error::SpiConfig)?
        .with_sck(p.GPIO14)
        .with_mosi(p.GPIO13)
        .with_miso(p.GPIO12);

    let dc = Output::new(p.GPIO2, Level::Low, OutputConfig::default());
    let rst = Output::new(p.GPIO4, Level::High, OutputConfig::default());
    let cs = Output::new(p.GPIO15, Level::High, OutputConfig::default());
    let _display_backlight = Output::new(p.GPIO21, Level::High, OutputConfig::default());

    let mut display_rect_writer =
        Ili9341RectWriter::new(spi, dc, rst, cs, width, height, Ili9341Rotation::Landscape);

    let touch_cs = Output::new(p.GPIO33, Level::High, OutputConfig::default());
    let touch_irq = Input::new(p.GPIO36, InputConfig::default().with_pull(Pull::Up));
    let mut touch_input = Xpt2046TouchInput::new(touch_cs, touch_irq);

    info!("robot-arm-cyd started: {width}x{height}");

    flush_full_frame(&mut display_rect_writer, &cyd_sim);

    let mut previous_tick = Instant::now();
    loop {
        let now = Instant::now();
        let dt_seconds = (now - previous_tick).as_micros() as f32 / 1_000_000.0;
        previous_tick = now;

        if let Some(touch_event) = touch_input.read_touch_event(&mut display_rect_writer) {
            match touch_event {
                TouchEvent::Down { x, y } => cyd_sim.touch_down(x, y),
                TouchEvent::Move { x, y } => cyd_sim.touch_move(x, y),
                TouchEvent::Up => cyd_sim.touch_up(),
            }
            flush_full_frame(&mut display_rect_writer, &cyd_sim);
        }

        if cyd_sim.tick_reverse_kinematics(dt_seconds) {
            flush_full_frame(&mut display_rect_writer, &cyd_sim);
        }

        Timer::after(Duration::from_millis(16)).await;
    }
}

fn flush_full_frame(display_rect_writer: &mut impl RectDisplay, cyd_sim: &CydSim) {
    let width = cyd_sim.width() as u16;
    let height = cyd_sim.height() as u16;
    let mut frame_buffer = [0u16; robot_arm_core::cyd::SCREEN_PIXELS];
    for (pixel_index, pixel) in cyd_sim.pixels().iter().enumerate() {
        frame_buffer[pixel_index] = rgb565_word(pixel.r(), pixel.g(), pixel.b());
    }

    let full_frame = DisplayRect::new(0, 0, width, height);
    display_rect_writer.write_rect_rgb565(full_frame, &frame_buffer);
}

const fn rgb565_word(red_5: u8, green_6: u8, blue_5: u8) -> u16 {
    ((red_5 as u16) << 11) | ((green_6 as u16) << 5) | (blue_5 as u16)
}
