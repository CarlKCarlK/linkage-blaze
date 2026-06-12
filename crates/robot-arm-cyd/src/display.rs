use embedded_graphics::{
    prelude::{DrawTarget, Point, Size},
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
use robot_arm_core::cyd::{FrameBuffer, SCREEN_HEIGHT, SCREEN_WIDTH};
use static_cell::StaticCell;

const DISPLAY_SPI_HZ: u32 = 60_000_000;
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

//todo00 review all the code related to CydDisplay, including its name.
pub struct CydDisplay {
    display: CydDisplayDevice,
    frame_buffer: &'static mut FrameBuffer,
}

impl CydDisplay {
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

        Ok(CydDisplay {
            display,
            frame_buffer: FrameBuffer::static_new(),
        })
    }

    pub fn frame_buffer_mut(&mut self) -> &mut FrameBuffer {
        self.frame_buffer
    }

    pub fn flush(&mut self) -> Result<(), CydDisplayFlushError> {
        let full_screen = Rectangle::new(
            Point::new(0, 0),
            Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
        );
        if self
            .display
            .fill_contiguous(&full_screen, self.frame_buffer.pixels().iter().copied())
            .is_err()
        {
            return Err(CydDisplayFlushError::FlushFrameBuffer);
        }
        Ok(())
    }
}
