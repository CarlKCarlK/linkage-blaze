#![no_std]
#![no_main]

use core::convert::Infallible;

use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::pixelcolor::RgbColor;
use esp_backtrace as _;
use log::info;
use robot_arm_core::cyd::CydSim;

use device_envoy_esp::{Result, init_and_start};

mod display;
mod touch;

use display::{DisplayRect, NullDisplayRectWriter, RectDisplay};
use touch::{NullTouchInput, TouchEvent, TouchInput};

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
    let mut display_rect_writer = NullDisplayRectWriter::new(width, height);
    let mut touch_input = NullTouchInput;

    info!("robot-arm-cyd started: {width}x{height}");

    flush_full_frame(&mut display_rect_writer, &cyd_sim);

    let mut previous_tick = Instant::now();
    loop {
        let now = Instant::now();
        let dt_seconds = (now - previous_tick).as_micros() as f32 / 1_000_000.0;
        previous_tick = now;

        if let Some(touch_event) = touch_input.read_touch_event() {
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
    let mut rgb565_pixels = [0u16; robot_arm_core::cyd::SCREEN_PIXELS];

    for (pixel_index, pixel) in cyd_sim.pixels().iter().enumerate() {
        rgb565_pixels[pixel_index] = rgb565_word(pixel.r(), pixel.g(), pixel.b());
    }

    let full_frame = DisplayRect::new(0, 0, width, height);
    display_rect_writer.write_rect_rgb565(full_frame, &rgb565_pixels);
}

const fn rgb565_word(red_5: u8, green_6: u8, blue_5: u8) -> u16 {
    ((red_5 as u16) << 11) | ((green_6 as u16) << 5) | (blue_5 as u16)
}
