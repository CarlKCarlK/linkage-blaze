use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::{Point, Size},
    primitives::Rectangle,
};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use esp_hal::{
    delay::Delay,
    gpio::{
        Level, Output, OutputConfig, OutputPin,
        interconnect::{PeripheralInput, PeripheralOutput},
    },
    spi,
};
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
};
use static_cell::StaticCell;

use crate::{RectPixels, SCREEN_HEIGHT, SCREEN_WIDTH};

// 80 MHz measured 10.9 draw+flush fps but produced visible display corruption.
pub const DISPLAY_SPI_HZ: u32 = 60_000_000;
const DISPLAY_SPI_BUFFER_LEN: usize = 64;

type CydDisplaySpiBus = spi::master::Spi<'static, esp_hal::Blocking>;
type CydDisplaySpiDevice = ExclusiveDevice<CydDisplaySpiBus, Output<'static>, NoDelay>;
type CydDisplayInterface = SpiInterface<'static, CydDisplaySpiDevice, Output<'static>>;
type CydDisplayDevice = mipidsi::Display<CydDisplayInterface, ILI9341Rgb565, Output<'static>>;

#[derive(Clone, Copy, Debug)]
pub enum CydDisplayInitError {
    ConfigureDisplaySpi,
    CreateDisplaySpiDevice,
    InitDisplay,
}

#[derive(Clone, Copy, Debug)]
pub enum CydDisplayFlushError {
    FlushFrameBuffer,
}

pub struct CydDisplay {
    display: CydDisplayDevice,
}

impl CydDisplay {
    #[must_use]
    pub const fn screen_size() -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }

    pub fn new(
        spi: impl spi::master::Instance + 'static,
        sck_pin: impl PeripheralOutput<'static>,
        mosi_pin: impl PeripheralOutput<'static>,
        miso_pin: impl PeripheralInput<'static>,
        cs_pin: impl OutputPin + 'static,
        dc_pin: impl OutputPin + 'static,
        rst_pin: impl OutputPin + 'static,
        backlight_pin: impl OutputPin + 'static,
    ) -> Result<CydDisplay, CydDisplayInitError> {
        let spi_config = spi::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_hz(DISPLAY_SPI_HZ))
            .with_mode(spi::Mode::_0);
        let spi = spi::master::Spi::new(spi, spi_config)
            .map_err(|_| CydDisplayInitError::ConfigureDisplaySpi)?
            .with_sck(sck_pin)
            .with_mosi(mosi_pin)
            .with_miso(miso_pin);

        let cs = Output::new(cs_pin, Level::High, OutputConfig::default());
        let dc = Output::new(dc_pin, Level::Low, OutputConfig::default());
        let rst = Output::new(rst_pin, Level::High, OutputConfig::default());
        let mut backlight = Output::new(backlight_pin, Level::High, OutputConfig::default());

        let spi_device = ExclusiveDevice::<_, _, NoDelay>::new_no_delay(spi, cs)
            .map_err(|_| CydDisplayInitError::CreateDisplaySpiDevice)?;

        static SPI_BUFFER: StaticCell<[u8; DISPLAY_SPI_BUFFER_LEN]> = StaticCell::new();
        let spi_buffer = SPI_BUFFER.init([0u8; DISPLAY_SPI_BUFFER_LEN]);
        let interface = SpiInterface::new(spi_device, dc, spi_buffer);
        let mut delay = Delay::new();

        let display = Builder::new(ILI9341Rgb565, interface)
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
            .map_err(|_| CydDisplayInitError::InitDisplay)?;

        backlight.set_high();

        Ok(CydDisplay { display })
    }

    pub fn flush_buffer(
        &mut self,
        buffer: &impl RectPixels,
        top_left: Point,
    ) -> Result<(), CydDisplayFlushError> {
        let rectangle = Rectangle::new(
            top_left,
            Size::new(buffer.width() as u32, buffer.height() as u32),
        );
        self.display
            .fill_contiguous(
                &rectangle,
                buffer
                    .raw_pixels()
                    .iter()
                    .copied()
                    .map(|pixel| Rgb565::from(RawU16::new(pixel))),
            )
            .map_err(|_| CydDisplayFlushError::FlushFrameBuffer)
    }

    pub fn clear_now(&mut self, color: Rgb565) -> Result<(), CydDisplayFlushError> {
        self.fill_rect_now(
            Rectangle::new(
                Point::new(0, 0),
                Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
            ),
            color,
        )
    }

    pub fn fill_rect_now(
        &mut self,
        rectangle: Rectangle,
        color: Rgb565,
    ) -> Result<(), CydDisplayFlushError> {
        let screen_rectangle = Rectangle::new(
            Point::new(0, 0),
            Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
        );
        let rectangle = rectangle.intersection(&screen_rectangle);
        if rectangle.size.width == 0 || rectangle.size.height == 0 {
            return Ok(());
        }
        self.display
            .fill_solid(&rectangle, color)
            .map_err(|_| CydDisplayFlushError::FlushFrameBuffer)
    }
}
