use embedded_hal::{digital::OutputPin, spi::SpiBus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayRect {
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16,
}

impl DisplayRect {
    #[must_use]
    pub const fn new(left: u16, top: u16, width: u16, height: u16) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

pub trait RectDisplay {
    fn write_rect_rgb565(&mut self, display_rect: DisplayRect, pixels: &[u16]);
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ili9341Rotation {
    Portrait,
    Landscape,
    PortraitInverted,
    LandscapeInverted,
}

impl Ili9341Rotation {
    const fn madctl(self) -> u8 {
        const MADCTL_MY: u8 = 0x80;
        const MADCTL_MX: u8 = 0x40;
        const MADCTL_MV: u8 = 0x20;
        const MADCTL_BGR: u8 = 0x08;

        match self {
            Self::Portrait => MADCTL_MY | MADCTL_BGR,
            Self::Landscape => MADCTL_MX | MADCTL_MV | MADCTL_BGR,
            Self::PortraitInverted => MADCTL_MX | MADCTL_BGR,
            Self::LandscapeInverted => MADCTL_MY | MADCTL_MV | MADCTL_BGR,
        }
    }
}

pub struct Ili9341RectWriter<SPI, DC, RST, CS> {
    spi: SPI,
    dc: DC,
    rst: RST,
    cs: CS,
    width: u16,
    height: u16,
}

impl<SPI, DC, RST, CS> Ili9341RectWriter<SPI, DC, RST, CS>
where
    SPI: SpiBus,
    DC: OutputPin,
    RST: OutputPin,
    CS: OutputPin,
{
    pub fn new(
        spi: SPI,
        dc: DC,
        rst: RST,
        cs: CS,
        width: u16,
        height: u16,
        rotation: Ili9341Rotation,
    ) -> Self {
        let mut display = Self {
            spi,
            dc,
            rst,
            cs,
            width,
            height,
        };
        display.reset_panel();
        display.initialize_panel(rotation);
        display
    }

    fn reset_panel(&mut self) {
        let _ = self.rst.set_high();
        let _ = self.rst.set_low();
        for _ in 0..200_000 {
            core::hint::spin_loop();
        }
        let _ = self.rst.set_high();
        for _ in 0..200_000 {
            core::hint::spin_loop();
        }
    }

    fn initialize_panel(&mut self, rotation: Ili9341Rotation) {
        self.write_command_with_data(0xCF, &[0x00, 0xC1, 0x30]);
        self.write_command_with_data(0xED, &[0x64, 0x03, 0x12, 0x81]);
        self.write_command_with_data(0xE8, &[0x85, 0x00, 0x78]);
        self.write_command_with_data(0xCB, &[0x39, 0x2C, 0x00, 0x34, 0x02]);
        self.write_command_with_data(0xF7, &[0x20]);
        self.write_command_with_data(0xEA, &[0x00, 0x00]);

        self.write_command_with_data(0xC0, &[0x10]);
        self.write_command_with_data(0xC1, &[0x00]);
        self.write_command_with_data(0xC5, &[0x30, 0x30]);
        self.write_command_with_data(0xC7, &[0xB7]);

        self.write_command_with_data(0x3A, &[0x55]);
        self.write_command_with_data(0x36, &[rotation.madctl()]);
        self.write_command_with_data(0xB1, &[0x00, 0x1A]);
        self.write_command_with_data(0xB6, &[0x08, 0x82, 0x27]);
        self.write_command_with_data(0xF2, &[0x00]);
        self.write_command_with_data(0x26, &[0x01]);

        self.write_command_with_data(
            0xE0,
            &[
                0x0F, 0x2A, 0x28, 0x08, 0x0E, 0x08, 0x54, 0xA9, 0x43, 0x0A, 0x0F, 0x00, 0x00, 0x00,
                0x00,
            ],
        );
        self.write_command_with_data(
            0xE1,
            &[
                0x00, 0x15, 0x17, 0x07, 0x11, 0x06, 0x2B, 0x56, 0x3C, 0x05, 0x10, 0x0F, 0x3F, 0x3F,
                0x0F,
            ],
        );

        self.write_command_with_data(0x2B, &[0x00, 0x00, 0x01, 0x3F]);
        self.write_command_with_data(0x2A, &[0x00, 0x00, 0x00, 0xEF]);

        self.write_command(0x11);
        for _ in 0..300_000 {
            core::hint::spin_loop();
        }

        self.write_command(0x29);
    }

    fn set_window(&mut self, display_rect: DisplayRect) {
        let (x0, y0, x1, y1) = self.window_bounds(display_rect);

        self.write_command_with_data(
            0x2A,
            &[
                (x0 >> 8) as u8,
                (x0 & 0xFF) as u8,
                (x1 >> 8) as u8,
                (x1 & 0xFF) as u8,
            ],
        );
        self.write_command_with_data(
            0x2B,
            &[
                (y0 >> 8) as u8,
                (y0 & 0xFF) as u8,
                (y1 >> 8) as u8,
                (y1 & 0xFF) as u8,
            ],
        );
        self.write_command(0x2C);
    }

    fn window_bounds(&self, display_rect: DisplayRect) -> (u16, u16, u16, u16) {
        let x0 = display_rect.left;
        let y0 = display_rect.top;
        let x1 = x0 + display_rect.width - 1;
        let y1 = y0 + display_rect.height - 1;
        (x0, y0, x1, y1)
    }

    fn write_command_with_data(&mut self, command: u8, data: &[u8]) {
        self.write_command(command);
        self.write_data(data);
    }

    pub fn begin_rect_rgb565(&mut self, display_rect: DisplayRect) {
        assert!(
            display_rect.width > 0 && display_rect.height > 0,
            "display rect must be non-empty"
        );
        assert!(
            display_rect.left as u32 + display_rect.width as u32 <= self.width as u32,
            "display rect exceeds width"
        );
        assert!(
            display_rect.top as u32 + display_rect.height as u32 <= self.height as u32,
            "display rect exceeds height"
        );

        self.set_window(display_rect);
    }

    pub fn write_rgb565_words(&mut self, pixels: &[u16]) {
        const PIXELS_PER_CHUNK: usize = 64;
        let mut chunk_bytes = [0u8; PIXELS_PER_CHUNK * 2];

        for pixel_chunk in pixels.chunks(PIXELS_PER_CHUNK) {
            for (pixel_index, pixel) in pixel_chunk.iter().enumerate() {
                let bytes = pixel.to_be_bytes();
                chunk_bytes[pixel_index * 2] = bytes[0];
                chunk_bytes[pixel_index * 2 + 1] = bytes[1];
            }
            self.write_data(&chunk_bytes[..pixel_chunk.len() * 2]);
        }
    }

    pub fn fill_fullscreen_solid_rgb565(&mut self, color: u16) {
        const PIXELS_PER_CHUNK: usize = 128;
        let mut chunk_bytes = [0u8; PIXELS_PER_CHUNK * 2];

        let [high_byte, low_byte] = color.to_be_bytes();
        for pixel_index in 0..PIXELS_PER_CHUNK {
            chunk_bytes[pixel_index * 2] = high_byte;
            chunk_bytes[pixel_index * 2 + 1] = low_byte;
        }

        let x1 = self.width - 1;
        let y1 = self.height - 1;
        let total_pixels = self.width as usize * self.height as usize;

        let _ = self.cs.set_low();

        self.write_command_in_transaction(0x2A);
        self.write_data_in_transaction(&[0x00, 0x00, (x1 >> 8) as u8, (x1 & 0xFF) as u8]);
        self.write_command_in_transaction(0x2B);
        self.write_data_in_transaction(&[0x00, 0x00, (y1 >> 8) as u8, (y1 & 0xFF) as u8]);
        self.write_command_in_transaction(0x2C);

        let mut remaining_pixels = total_pixels;
        while remaining_pixels > 0 {
            let chunk_pixels = core::cmp::min(remaining_pixels, PIXELS_PER_CHUNK);
            self.write_data_in_transaction(&chunk_bytes[..chunk_pixels * 2]);
            remaining_pixels -= chunk_pixels;
        }

        let _ = self.cs.set_high();
    }

    fn write_command(&mut self, command: u8) {
        let _ = self.cs.set_low();
        self.write_command_in_transaction(command);
        let _ = self.cs.set_high();
    }

    fn write_data(&mut self, data: &[u8]) {
        let _ = self.cs.set_low();
        self.write_data_in_transaction(data);
        let _ = self.cs.set_high();
    }

    fn write_command_in_transaction(&mut self, command: u8) {
        let _ = self.dc.set_low();
        self.spi
            .write(&[command])
            .expect("ILI9341 command write failed");
    }

    fn write_data_in_transaction(&mut self, data: &[u8]) {
        let _ = self.dc.set_high();
        self.spi.write(data).expect("ILI9341 data write failed");
    }

    /// Perform a read/write SPI transaction for touch controller access.
    /// Caller is responsible for managing touch CS pin state and DC pin (left as-is).
    pub fn touch_spi_transact(&mut self, tx_buf: &[u8], rx_buf: &mut [u8]) {
        self.spi
            .transfer(rx_buf, tx_buf)
            .expect("touch SPI transaction failed");
    }
}

impl<SPI, DC, RST, CS> RectDisplay for Ili9341RectWriter<SPI, DC, RST, CS>
where
    SPI: SpiBus,
    DC: OutputPin,
    RST: OutputPin,
    CS: OutputPin,
{
    fn write_rect_rgb565(&mut self, display_rect: DisplayRect, pixels: &[u16]) {
        assert!(
            pixels.len() == display_rect.pixel_count(),
            "pixel slice length must match display rect area"
        );

        self.begin_rect_rgb565(display_rect);
        self.write_rgb565_words(pixels);
    }
}
