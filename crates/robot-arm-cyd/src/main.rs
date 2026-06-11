#![no_std]
#![no_main]

use core::cell::RefCell;

use embedded_graphics::{
    Pixel,
    pixelcolor::Rgb565,
    prelude::{DrawTarget, RgbColor},
};
use embedded_hal_bus::spi::RefCellDevice;
use esp_backtrace as _;
use esp_hal::{
    Config,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    spi,
    time::Instant,
};
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
};
use robot_arm_core::cyd::{CydSim, FrameBuffer};
use static_cell::StaticCell;

mod touch;

use touch::{TouchEvent, TouchInput, Xpt2046TouchInput};

esp_bootloader_esp_idf::esp_app_desc!();

static CYD_SIM: StaticCell<CydSim> = StaticCell::new();
static FRAME_BUFFER: StaticCell<FrameBuffer> = StaticCell::new();
static DISPLAY_INTERFACE_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();

#[esp_hal::main]
fn main() -> ! {
    let p = esp_hal::init(Config::default());
    esp_println::logger::init_logger(log::LevelFilter::Info);
    esp_println::println!("boot: after esp_hal::init");
    run_after_init(p)
}

fn run_after_init(p: esp_hal::peripherals::Peripherals) -> ! {
    let mut delay = Delay::new();

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

    esp_println::println!("boot: before display spi init");
    let spi_config = spi::master::Config::default()
        .with_frequency(esp_hal::time::Rate::from_hz(10_000_000))
        .with_mode(spi::Mode::_0);
    let spi = spi::master::Spi::new(p.SPI2, spi_config)
        .expect("boot: failed to configure display spi")
        .with_sck(p.GPIO14)
        .with_mosi(p.GPIO13)
        .with_miso(p.GPIO12);
    let spi_bus = RefCell::new(spi);
    esp_println::println!("boot: after display spi init");

    esp_println::println!("boot: before display pin init");
    let display_cs = Output::new(p.GPIO15, Level::High, OutputConfig::default());
    let dc = Output::new(p.GPIO2, Level::Low, OutputConfig::default());
    let rst = Output::new(p.GPIO4, Level::High, OutputConfig::default());
    let touch_cs = Output::new(p.GPIO33, Level::High, OutputConfig::default());
    let touch_irq = Input::new(p.GPIO36, InputConfig::default().with_pull(Pull::Up));
    let mut display_backlight = Output::new(p.GPIO21, Level::High, OutputConfig::default());

    let display_spi_device = RefCellDevice::new_no_delay(&spi_bus, display_cs)
        .expect("boot: failed to create display spi device");
    let mut touch_spi_device = RefCellDevice::new_no_delay(&spi_bus, touch_cs)
        .expect("boot: failed to create touch spi device");
    let mut touch_input = Xpt2046TouchInput::new(touch_irq);

    let display_interface_buffer = DISPLAY_INTERFACE_BUFFER.init([0u8; 512]);
    let display_interface = SpiInterface::new(display_spi_device, dc, display_interface_buffer);
    esp_println::println!("boot: after display pin init");

    esp_println::println!("boot: before display init");
    let mut display = Builder::new(ILI9341Rgb565, display_interface)
        .reset_pin(rst)
        .display_size(240, 320)
        .color_order(ColorOrder::Bgr)
        .orientation(
            Orientation::new()
                .rotate(Rotation::Deg90)
                .flip_horizontal()
                .rotate(Rotation::Deg180),
        )
        .init(&mut delay)
        .expect("boot: failed to initialize mipidsi display");
    esp_println::println!("boot: after display init");

    esp_println::println!("boot: before display probe write");
    display_backlight.set_high();
    display
        .clear(Rgb565::RED)
        .expect("boot: failed to clear display red");
    esp_println::println!("boot: full-screen red");
    delay.delay_millis(200);
    display
        .clear(Rgb565::GREEN)
        .expect("boot: failed to clear display green");
    esp_println::println!("boot: full-screen green");
    delay.delay_millis(200);
    display
        .clear(Rgb565::BLUE)
        .expect("boot: failed to clear display blue");
    esp_println::println!("boot: full-screen blue");
    delay.delay_millis(200);
    display
        .clear(Rgb565::WHITE)
        .expect("boot: failed to clear display white");
    esp_println::println!("boot: full-screen white");
    delay.delay_millis(100);
    esp_println::println!("boot: after display probe write");

    esp_println::println!("boot: before initial frame flush");
    cyd_sim.touch_up();
    cyd_sim.start_reverse_kinematics();
    esp_println::println!("boot: RK auto-start enabled");
    cyd_sim.render_to(frame_buffer);
    flush_full_frame(&mut display, frame_buffer, width, height);
    esp_println::println!("boot: after initial frame flush");

    let mut previous_tick = Instant::now();
    let mut alive_tick_count: u32 = 0;
    let mut touch_poll_counter: u32 = 0;
    esp_println::println!("boot: entering app loop");

    loop {
        let now = Instant::now();
        let dt_seconds = (now - previous_tick).as_micros() as f32 / 1_000_000.0;
        previous_tick = now;

        let mut should_flush = cyd_sim.tick_reverse_kinematics(dt_seconds);

        // Poll touch at a reduced cadence to keep animation fluid.
        touch_poll_counter = touch_poll_counter.wrapping_add(1);
        if touch_poll_counter % 4 == 0 {
            if let Some(touch_event) = touch_input.read_touch_event(&mut touch_spi_device) {
                match touch_event {
                    TouchEvent::Down { x, y } => {
                        let (mapped_x, mapped_y) =
                            remap_touch_for_display_rotation(x, y, width as f32, height as f32);
                        cyd_sim.touch_down(mapped_x, mapped_y);
                        should_flush = true;
                    }
                    TouchEvent::Move { x, y } => {
                        let (mapped_x, mapped_y) =
                            remap_touch_for_display_rotation(x, y, width as f32, height as f32);
                        cyd_sim.touch_move(mapped_x, mapped_y);
                        should_flush = true;
                    }
                    TouchEvent::Up => {
                        cyd_sim.touch_up();
                    }
                }
            }
        }

        if should_flush {
            cyd_sim.render_to(frame_buffer);
            flush_full_frame(&mut display, frame_buffer, width, height);
        }

        alive_tick_count = alive_tick_count.wrapping_add(1);
        if alive_tick_count % 60 == 0 {
            esp_println::println!("boot: alive");
        }

        delay.delay_millis(1);
    }
}

fn remap_touch_for_display_rotation(x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
    // Display is rotated 180 degrees relative to the original touch mapping.
    let mapped_x = (width - x).clamp(0.0, width);
    let mapped_y = (height - y).clamp(0.0, height);
    (mapped_x, mapped_y)
}

fn flush_full_frame(
    display: &mut impl DrawTarget<Color = Rgb565>,
    frame_buffer: &FrameBuffer,
    width: u16,
    _height: u16,
) {
    let width_usize = width as usize;
    let pixel_iter = frame_buffer
        .pixels()
        .iter()
        .enumerate()
        .map(|(pixel_index, color_pixel)| {
            let column = (pixel_index % width_usize) as i32;
            let row = (pixel_index / width_usize) as i32;
            Pixel(
                embedded_graphics::prelude::Point::new(column, row),
                *color_pixel,
            )
        });

    if display.draw_iter(pixel_iter).is_err() {
        panic!("boot: failed to draw framebuffer");
    }
}
