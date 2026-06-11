#![no_std]
#![no_main]

use core::cell::RefCell;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{DrawTarget, RgbColor},
};
use embedded_hal_bus::spi::RefCellDevice;
use esp_backtrace as _;
use esp_hal::{
    Config,
    delay::Delay,
    gpio::{Level, Output, OutputConfig},
    spi,
};
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
};
use robot_arm_core::cyd::{CydSim, FrameBuffer};
use static_cell::StaticCell;

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
    let mut display_backlight = Output::new(p.GPIO21, Level::High, OutputConfig::default());

    let display_spi_device = RefCellDevice::new_no_delay(&spi_bus, display_cs)
        .expect("boot: failed to create display spi device");
    let _touch_spi_device = RefCellDevice::new_no_delay(&spi_bus, touch_cs)
        .expect("boot: failed to create touch spi device");

    let display_interface_buffer = DISPLAY_INTERFACE_BUFFER.init([0u8; 512]);
    let display_interface = SpiInterface::new(display_spi_device, dc, display_interface_buffer);
    esp_println::println!("boot: after display pin init");

    esp_println::println!("boot: before display init");
    let mut display = Builder::new(ILI9341Rgb565, display_interface)
        .reset_pin(rst)
        .display_size(240, 320)
        .color_order(ColorOrder::Bgr)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .init(&mut delay)
        .expect("boot: failed to initialize mipidsi display");
    esp_println::println!("boot: after display init");

    esp_println::println!("boot: before display probe write");
    display_backlight.set_high();
    display
        .clear(Rgb565::RED)
        .expect("boot: failed to clear display red");
    esp_println::println!("boot: full-screen red");
    delay.delay_millis(1000);
    display
        .clear(Rgb565::GREEN)
        .expect("boot: failed to clear display green");
    esp_println::println!("boot: full-screen green");
    delay.delay_millis(1000);
    display
        .clear(Rgb565::BLUE)
        .expect("boot: failed to clear display blue");
    esp_println::println!("boot: full-screen blue");
    delay.delay_millis(1000);
    display
        .clear(Rgb565::WHITE)
        .expect("boot: failed to clear display white");
    esp_println::println!("boot: full-screen white");
    delay.delay_millis(1000);
    esp_println::println!("boot: after display probe write");

    esp_println::println!("boot: diagnostic hold loop");
    loop {
        delay.delay_millis(1000);
        esp_println::println!("boot: alive");
    }
}
