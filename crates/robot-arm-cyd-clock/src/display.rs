use core::{convert::Infallible, fmt};

use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::{IntoStorage, Rgb565, RgbColor, raw::RawU16},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    primitives::{Circle, Primitive, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
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
use robot_arm_core::{Linkage, Pose};
use static_cell::StaticCell;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 240;
const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const MONO_MASK_BYTES: usize = SCREEN_PIXELS / 8;
const DISPLAY_SPI_HZ: u32 = 60_000_000;
const DISPLAY_SPI_BUFFER_LEN: usize = 64;
const CLOCK_CENTER_X: i32 = 160;
const CLOCK_CENTER_Y: i32 = 154;
const CLOCK_RADIUS: i32 = 66;
const HAND_SCALE: f32 = 1.0;
const HOUR_PARAM: usize = 0;
const MINUTE_PARAM: usize = 1;
const SECOND_PARAM: usize = 2;
const CLOCK_HANDS: Linkage<3, 9> = Linkage::start()
    .yaw_param(HOUR_PARAM, -90.0, 270.0)
    .forward(38.0)
    .restart()
    .yaw_param(MINUTE_PARAM, -90.0, 270.0)
    .forward(54.0)
    .restart()
    .yaw_param(SECOND_PARAM, -90.0, 270.0)
    .forward(62.0);

type CydClockDisplaySpiBus = spi::master::Spi<'static, esp_hal::Blocking>;
type CydClockDisplaySpiDevice = ExclusiveDevice<CydClockDisplaySpiBus, Output<'static>, NoDelay>;
type CydClockDisplayInterface = SpiInterface<'static, CydClockDisplaySpiDevice, Output<'static>>;
type CydClockDisplayDevice =
    mipidsi::Display<CydClockDisplayInterface, ILI9341Rgb565, Output<'static>>;

#[derive(Clone, Copy, Debug)]
pub enum CydClockDisplayError {
    ConfigureDisplaySpi,
    CreateDisplaySpiDevice,
    InitDisplay,
    FlushFrameBuffer,
}

pub struct CydClockDisplay {
    display: CydClockDisplayDevice,
    line_buffer: LineBuffer,
    mono_mask: &'static mut MonoMask,
}

impl CydClockDisplay {
    pub fn new(
        spi: impl spi::master::Instance + 'static,
        sck_pin: impl PeripheralOutput<'static>,
        mosi_pin: impl PeripheralOutput<'static>,
        miso_pin: impl PeripheralInput<'static>,
        cs_pin: impl OutputPin + 'static,
        dc_pin: impl OutputPin + 'static,
        rst_pin: impl OutputPin + 'static,
        backlight_pin: impl OutputPin + 'static,
    ) -> Result<Self, CydClockDisplayError> {
        let spi_config = spi::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_hz(DISPLAY_SPI_HZ))
            .with_mode(spi::Mode::_0);
        let spi = spi::master::Spi::new(spi, spi_config)
            .map_err(|_| CydClockDisplayError::ConfigureDisplaySpi)?
            .with_sck(sck_pin)
            .with_mosi(mosi_pin)
            .with_miso(miso_pin);

        let cs = Output::new(cs_pin, Level::High, OutputConfig::default());
        let dc = Output::new(dc_pin, Level::Low, OutputConfig::default());
        let rst = Output::new(rst_pin, Level::High, OutputConfig::default());
        let mut backlight = Output::new(backlight_pin, Level::High, OutputConfig::default());

        let spi_device = ExclusiveDevice::<_, _, NoDelay>::new_no_delay(spi, cs)
            .map_err(|_| CydClockDisplayError::CreateDisplaySpiDevice)?;

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
            .map_err(|_| CydClockDisplayError::InitDisplay)?;

        backlight.set_high();

        static MONO_MASK: StaticCell<MonoMask> = StaticCell::new();

        Ok(Self {
            display,
            line_buffer: LineBuffer::new(),
            mono_mask: MONO_MASK.init_with(MonoMask::new),
        })
    }

    pub fn show(
        &mut self,
        wifi_mode: &str,
        clock_time: Option<&ClockTime>,
    ) -> Result<(), CydClockDisplayError> {
        self.mono_mask.clear();
        if let Some(clock_time) = clock_time {
            draw_clock_hands(self.mono_mask, clock_time);
        }
        let time_text = clock_time.map_or("--:--:--", ClockTime::as_str);

        for screen_y in 0..SCREEN_HEIGHT {
            self.line_buffer.set_screen_y(screen_y);
            self.line_buffer.clear(Rgb565::BLACK);
            draw_clock_screen(&mut self.line_buffer, wifi_mode, time_text);
            self.line_buffer.overlay_mask_row(self.mono_mask);

            let row = Rectangle::new(
                Point::new(0, screen_y as i32),
                Size::new(SCREEN_WIDTH as u32, 1),
            );
            self.display
                .fill_contiguous(
                    &row,
                    self.line_buffer
                        .raw_pixels()
                        .iter()
                        .copied()
                        .map(|pixel| Rgb565::from(RawU16::new(pixel))),
                )
                .map_err(|_| CydClockDisplayError::FlushFrameBuffer)?;
        }

        Ok(())
    }
}

fn draw_clock_screen(
    target: &mut impl DrawTarget<Color = Rgb565>,
    wifi_mode: &str,
    time_text: &str,
) {
    let title_style = MonoTextStyle::new(&FONT_10X20, Rgb565::CYAN);
    let status_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let time_style = MonoTextStyle::new(&FONT_10X20, Rgb565::YELLOW);
    let clock_face_style = PrimitiveStyle::with_stroke(Rgb565::BLUE, 1);

    Text::with_baseline("CYD Clock", Point::new(16, 6), title_style, Baseline::Top)
        .draw(target)
        .ok();
    Text::with_baseline("WiFi:", Point::new(16, 32), status_style, Baseline::Top)
        .draw(target)
        .ok();
    Text::with_baseline(wifi_mode, Point::new(78, 32), status_style, Baseline::Top)
        .draw(target)
        .ok();
    Text::with_baseline("Time:", Point::new(16, 58), status_style, Baseline::Top)
        .draw(target)
        .ok();
    Text::with_baseline(time_text, Point::new(78, 58), time_style, Baseline::Top)
        .draw(target)
        .ok();
    Circle::with_center(
        Point::new(CLOCK_CENTER_X, CLOCK_CENTER_Y),
        (CLOCK_RADIUS * 2) as u32,
    )
    .into_styled(clock_face_style)
    .draw(target)
    .ok();
}

fn draw_clock_hands(mono_mask: &mut MonoMask, clock_time: &ClockTime) {
    let params = clock_time.params();
    let mut previous_pose = None;
    let mut hand_index = 0;
    for pose in CLOCK_HANDS.poses(&params) {
        if is_origin_pose(pose) {
            previous_pose = Some(pose);
            continue;
        }

        if let Some(previous_pose) = previous_pose {
            let radius = match hand_index {
                0 => 3,
                1 => 2,
                _ => 1,
            };
            draw_pose_line(mono_mask, previous_pose, pose, radius);
            hand_index += 1;
        }
        previous_pose = Some(pose);
    }
}

fn is_origin_pose(pose: Pose) -> bool {
    let position = pose.position();
    position[0].abs() < 0.001 && position[1].abs() < 0.001 && position[2].abs() < 0.001
}

fn draw_pose_line(mono_mask: &mut MonoMask, start: Pose, end: Pose, radius: i32) {
    let start = pose_to_point(start);
    let end = pose_to_point(end);
    draw_wide_line(mono_mask, start, end, radius);
}

fn pose_to_point(pose: Pose) -> Point {
    let position = pose.position();
    Point::new(
        CLOCK_CENTER_X + (position[0] * HAND_SCALE) as i32,
        CLOCK_CENTER_Y + (position[1] * HAND_SCALE) as i32,
    )
}

fn draw_wide_line(mono_mask: &mut MonoMask, start: Point, end: Point, radius: i32) {
    let mut x0 = start.x;
    let mut y0 = start.y;
    let x1 = end.x;
    let y1 = end.y;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        mono_mask.set_wide(x0, y0, radius);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let error2 = 2 * error;
        if error2 >= dy {
            error += dy;
            x0 += sx;
        }
        if error2 <= dx {
            error += dx;
            y0 += sy;
        }
    }
}

struct LineBuffer {
    screen_y: usize,
    pixels: [u16; SCREEN_WIDTH],
}

impl LineBuffer {
    fn new() -> Self {
        Self {
            screen_y: 0,
            pixels: [0; SCREEN_WIDTH],
        }
    }

    fn set_screen_y(&mut self, screen_y: usize) {
        self.screen_y = screen_y;
    }

    fn clear(&mut self, color: Rgb565) {
        self.pixels.fill(color.into_storage());
    }

    fn raw_pixels(&self) -> &[u16; SCREEN_WIDTH] {
        &self.pixels
    }

    fn overlay_mask_row(&mut self, mono_mask: &MonoMask) {
        for point_x in 0..SCREEN_WIDTH {
            if mono_mask.is_set(point_x, self.screen_y) {
                self.pixels[point_x] = Rgb565::GREEN.into_storage();
            }
        }
    }
}

impl DrawTarget for LineBuffer {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let point_x = point.x as usize;
            let point_y = point.y as usize;
            if point_x < SCREEN_WIDTH && point_y == self.screen_y {
                self.pixels[point_x] = color.into_storage();
            }
        }
        Ok(())
    }
}

impl OriginDimensions for LineBuffer {
    fn size(&self) -> Size {
        Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
    }
}

struct MonoMask {
    bytes: [u8; MONO_MASK_BYTES],
}

impl MonoMask {
    fn new() -> Self {
        Self {
            bytes: [0; MONO_MASK_BYTES],
        }
    }

    fn clear(&mut self) {
        self.bytes.fill(0);
    }

    fn set_wide(&mut self, x: i32, y: i32, radius: i32) {
        let radius_squared = radius * radius;
        for offset_y in -radius..=radius {
            for offset_x in -radius..=radius {
                if offset_x * offset_x + offset_y * offset_y <= radius_squared {
                    self.set(x + offset_x, y + offset_y);
                }
            }
        }
    }

    fn set(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as usize;
        let y = y as usize;
        if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
            return;
        }
        let bit_index = y * SCREEN_WIDTH + x;
        self.bytes[bit_index / 8] |= 1 << (bit_index % 8);
    }

    fn is_set(&self, x: usize, y: usize) -> bool {
        let bit_index = y * SCREEN_WIDTH + x;
        self.bytes[bit_index / 8] & (1 << (bit_index % 8)) != 0
    }
}

pub struct ClockTime {
    text: heapless::String<16>,
    hours: u8,
    minutes: u8,
    seconds: u8,
}

impl ClockTime {
    pub fn new(hours: u8, minutes: u8, seconds: u8) -> Result<Self, fmt::Error> {
        let mut text = heapless::String::<16>::new();
        let meridiem = if hours < 12 { "AM" } else { "PM" };
        let hours12 = match hours % 12 {
            0 => 12,
            hours12 => hours12,
        };
        fmt::Write::write_fmt(
            &mut text,
            format_args!("{}:{:02}:{:02} {}", hours12, minutes, seconds, meridiem),
        )?;
        Ok(Self {
            text,
            hours,
            minutes,
            seconds,
        })
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn params(&self) -> [f32; 3] {
        let second = self.seconds as f32 / 60.0;
        let minute = (self.minutes as f32 + second) / 60.0;
        let hour = ((self.hours % 12) as f32 + minute) / 12.0;
        [hour, minute, second]
    }
}
