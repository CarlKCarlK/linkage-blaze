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
use robot_arm_core::cyd::{CydSim, FrameBuffer};
use static_cell::StaticCell;

use device_envoy_esp::Result;

mod display;
mod touch;

use display::{DisplayRect, Ili9341RectWriter, Ili9341Rotation, RectDisplay};
use touch::{TouchEvent, TouchInput, Xpt2046TouchInput};

esp_bootloader_esp_idf::esp_app_desc!();

static CYD_SIM: StaticCell<CydSim> = StaticCell::new();
static FRAME_BUFFER: StaticCell<FrameBuffer> = StaticCell::new();

#[esp_hal::main]
fn main() -> ! {
    let p = esp_hal::init(Config::default());
    esp_println::logger::init_logger(log::LevelFilter::Info);
    esp_println::println!("boot: after esp_hal::init");
    run_after_init(p)
}

fn run_after_init(p: esp_hal::peripherals::Peripherals) -> ! {
    let delay = Delay::new();

    esp_println::println!("boot: before cyd sim init");
    let cyd_sim = CYD_SIM.init_with(CydSim::new);
    esp_println::println!("boot: after cyd sim new");
    esp_println::println!("boot: before frame buffer init");
    let frame_buffer = FRAME_BUFFER.init_with(FrameBuffer::new);
    esp_println::println!("boot: after frame buffer init");
    esp_println::println!("boot: before cyd sim render");
    cyd_sim.render_to(frame_buffer);
    esp_println::println!("boot: after cyd sim render");
    let width = cyd_sim.width() as u16;
    let height = cyd_sim.height() as u16;
    esp_println::println!("boot: after cyd sim init");
    cyd_sim.touch_up();

    esp_println::println!("boot: before display spi init");
    let spi_config = spi::master::Config::default()
        .with_frequency(esp_hal::time::Rate::from_hz(26_000_000))
        .with_mode(spi::Mode::_0);
    let spi = spi::master::Spi::new(p.SPI2, spi_config)
        .expect("boot: failed to configure display spi")
        .with_sck(p.GPIO14)
        .with_mosi(p.GPIO13)
        .with_miso(p.GPIO12);
    esp_println::println!("boot: after display spi init");

    esp_println::println!("boot: before display pin init");
    let dc = Output::new(p.GPIO2, Level::Low, OutputConfig::default());
    let rst = Output::new(p.GPIO4, Level::High, OutputConfig::default());
    let cs = Output::new(p.GPIO15, Level::High, OutputConfig::default());
    let mut display_backlight = Output::new(p.GPIO21, Level::High, OutputConfig::default());
    esp_println::println!("boot: after display pin init");

    esp_println::println!("boot: before display init");
    let mut display_rect_writer =
        Ili9341RectWriter::new(spi, dc, rst, cs, width, height, Ili9341Rotation::Landscape);
    esp_println::println!("boot: after display init");

    esp_println::println!("boot: before display probe write");
    fill_screen_rgb565(
        &mut display_rect_writer,
        width,
        height,
        rgb565_word(31, 63, 31),
    );
    esp_println::println!("boot: display probe with backlight high");
    delay.delay_millis(1000);
    display_backlight.set_low();
    esp_println::println!("boot: display probe with backlight low");
    delay.delay_millis(1000);
    display_backlight.set_high();
    fill_screen_rgb565(
        &mut display_rect_writer,
        width,
        height,
        rgb565_word(31, 0, 0),
    );
    esp_println::println!("boot: after display probe write");

    esp_println::println!("boot: before touch init");
    let touch_cs = Output::new(p.GPIO33, Level::High, OutputConfig::default());
    let touch_irq = Input::new(p.GPIO36, InputConfig::default().with_pull(Pull::Up));
    let mut touch_input = Xpt2046TouchInput::new(touch_cs, touch_irq);
    esp_println::println!("boot: after touch init");

    esp_println::println!("boot: before initial frame flush");
    fill_frame_buffer_test_pattern(frame_buffer, width as usize, height as usize);
    flush_full_frame(&mut display_rect_writer, cyd_sim, frame_buffer);
    esp_println::println!("boot: showing framebuffer test pattern");
    delay.delay_millis(5000);

    cyd_sim.render_to(frame_buffer);
    flush_full_frame(&mut display_rect_writer, cyd_sim, frame_buffer);
    esp_println::println!("boot: after initial frame flush");

    let mut previous_tick = Instant::now();
    let mut alive_tick_count: u32 = 0;
    esp_println::println!("boot: entering app loop");

    loop {
        let now = Instant::now();
        let dt_seconds = (now - previous_tick).as_micros() as f32 / 1_000_000.0;
        previous_tick = now;

        let mut should_flush = false;

        if let Some(touch_event) = touch_input.read_touch_event(&mut display_rect_writer) {
            match touch_event {
                TouchEvent::Down { x, y } => {
                    cyd_sim.touch_down(x, y);
                    should_flush = true;
                }
                TouchEvent::Move { x, y } => {
                    cyd_sim.touch_move(x, y);
                    should_flush = true;
                }
                TouchEvent::Up => {
                    cyd_sim.touch_up();
                }
            }
        }

        if cyd_sim.tick_reverse_kinematics(dt_seconds) {
            should_flush = true;
        }

        if should_flush {
            cyd_sim.render_to(frame_buffer);
            flush_full_frame(&mut display_rect_writer, cyd_sim, frame_buffer);
        }

        alive_tick_count = alive_tick_count.wrapping_add(1);
        if alive_tick_count % 60 == 0 {
            esp_println::println!("boot: alive");
        }

        delay.delay_millis(16);
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
    frame_buffer: &FrameBuffer,
) {
    let width = cyd_sim.width() as u16;
    let height = cyd_sim.height() as u16;

    let mut row_rgb565_words = [0u16; robot_arm_core::cyd::SCREEN_WIDTH];
    let width_usize = width as usize;
    for row in 0..height {
        let row_start = row as usize * width_usize;
        for (column, pixel) in frame_buffer.pixels()[row_start..row_start + width_usize]
            .iter()
            .enumerate()
        {
            row_rgb565_words[column] = rgb565_word(pixel.r(), pixel.g(), pixel.b());
        }

        display_rect_writer
            .write_rect_rgb565(DisplayRect::new(0, row, width, 1), &row_rgb565_words);
    }
}

fn fill_screen_rgb565(
    display_rect_writer: &mut impl RectDisplay,
    width: u16,
    height: u16,
    color: u16,
) {
    let row_rgb565_words = [color; robot_arm_core::cyd::SCREEN_WIDTH];
    for row in 0..height {
        display_rect_writer
            .write_rect_rgb565(DisplayRect::new(0, row, width, 1), &row_rgb565_words);
    }
}

fn fill_frame_buffer_test_pattern(frame_buffer: &mut FrameBuffer, width: usize, height: usize) {
    let pixels = frame_buffer.pixels_mut();
    for row in 0..height {
        let color = if row < height / 3 {
            embedded_graphics::pixelcolor::Rgb565::WHITE
        } else if row < 2 * height / 3 {
            embedded_graphics::pixelcolor::Rgb565::RED
        } else {
            embedded_graphics::pixelcolor::Rgb565::BLUE
        };

        let row_start = row * width;
        let row_end = row_start + width;
        pixels[row_start..row_end].fill(color);
    }
}

const fn rgb565_word(red_5: u8, green_6: u8, blue_5: u8) -> u16 {
    ((red_5 as u16) << 11) | ((green_6 as u16) << 5) | (blue_5 as u16)
}
