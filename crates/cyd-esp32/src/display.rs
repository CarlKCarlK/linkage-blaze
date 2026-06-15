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

#[derive(Clone, Copy, Debug)]
pub struct LineSegment {
    pub start: Point,
    pub end: Point,
    pub width: u16,
    pub color: Rgb565,
}

#[derive(Clone, Copy, Debug)]
pub struct Ellipse {
    pub center: Point,
    pub axis_a: (f32, f32), // v0_xy * radius
    pub axis_b: (f32, f32), // v1_xy * radius
    pub radius: f32,
    pub stroke_width: u16,
    pub color: Rgb565,
    pub filled: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum DrawPrimitive {
    LineSegment(LineSegment),
    Ellipse(Ellipse),
}

struct LineSegmentPixels<'a> {
    x0: i32,
    y0: i32,
    width: usize,
    index: usize,
    pixel_count: usize,
    background: Rgb565,
    segments: &'a [LineSegment],
}

impl Iterator for LineSegmentPixels<'_> {
    type Item = Rgb565;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.pixel_count {
            return None;
        }

        let offset_x = self.index % self.width;
        let offset_y = self.index / self.width;
        self.index += 1;

        let point_x = self.x0 + offset_x as i32;
        let point_y = self.y0 + offset_y as i32;
        let mut color = self.background;

        for segment in self.segments {
            if point_covered_by_segment(point_x, point_y, *segment) {
                color = segment.color;
            }
        }

        Some(color)
    }
}

struct PrimitivePixels<'a> {
    x0: i32,
    y0: i32,
    width: usize,
    index: usize,
    pixel_count: usize,
    background: Rgb565,
    primitives: &'a [DrawPrimitive],
}

impl Iterator for PrimitivePixels<'_> {
    type Item = Rgb565;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.pixel_count {
            return None;
        }

        let offset_x = self.index % self.width;
        let offset_y = self.index / self.width;
        self.index += 1;

        let point_x = self.x0 + offset_x as i32;
        let point_y = self.y0 + offset_y as i32;
        let mut color = self.background;

        for primitive in self.primitives {
            match *primitive {
                DrawPrimitive::LineSegment(line_segment)
                    if point_covered_by_segment(point_x, point_y, line_segment) =>
                {
                    color = line_segment.color;
                }
                DrawPrimitive::Ellipse(ellipse)
                    if point_covered_by_ellipse(point_x, point_y, &ellipse) =>
                {
                    color = ellipse.color;
                }
                _ => {}
            }
        }

        Some(color)
    }
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

    pub fn draw_line_segments_now(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        segments: &[LineSegment],
    ) -> Result<(), CydDisplayFlushError> {
        let screen_rectangle = Rectangle::new(
            Point::new(0, 0),
            Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
        );
        let bounds = bounds.intersection(&screen_rectangle);
        if bounds.size.width == 0 || bounds.size.height == 0 {
            return Ok(());
        }

        let pixel_count = bounds.size.width as usize * bounds.size.height as usize;
        let pixels = LineSegmentPixels {
            x0: bounds.top_left.x,
            y0: bounds.top_left.y,
            width: bounds.size.width as usize,
            index: 0,
            pixel_count,
            background,
            segments,
        };

        self.display
            .fill_contiguous(&bounds, pixels)
            .map_err(|_| CydDisplayFlushError::FlushFrameBuffer)
    }

    pub fn draw_primitives_now(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        primitives: &[DrawPrimitive],
    ) -> Result<(), CydDisplayFlushError> {
        let screen_rectangle = Rectangle::new(
            Point::new(0, 0),
            Size::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32),
        );
        let bounds = bounds.intersection(&screen_rectangle);
        if bounds.size.width == 0 || bounds.size.height == 0 {
            return Ok(());
        }

        let pixel_count = bounds.size.width as usize * bounds.size.height as usize;
        let pixels = PrimitivePixels {
            x0: bounds.top_left.x,
            y0: bounds.top_left.y,
            width: bounds.size.width as usize,
            index: 0,
            pixel_count,
            background,
            primitives,
        };

        self.display
            .fill_contiguous(&bounds, pixels)
            .map_err(|_| CydDisplayFlushError::FlushFrameBuffer)
    }
}

fn point_covered_by_segment(point_x: i32, point_y: i32, segment: LineSegment) -> bool {
    if segment.width == 0 {
        return false;
    }

    let start_x = i64::from(segment.start.x);
    let start_y = i64::from(segment.start.y);
    let end_x = i64::from(segment.end.x);
    let end_y = i64::from(segment.end.y);
    let point_x = i64::from(point_x);
    let point_y = i64::from(point_y);

    let segment_x = end_x - start_x;
    let segment_y = end_y - start_y;
    let point_from_start_x = point_x - start_x;
    let point_from_start_y = point_y - start_y;
    let segment_len_squared = segment_x * segment_x + segment_y * segment_y;

    // Radius rounds up so width 1 still draws a usable thin line with no gaps.
    let radius = (i64::from(segment.width) + 1) / 2;
    let radius_squared = radius * radius;

    if segment_len_squared == 0 {
        let distance_x = point_x - start_x;
        let distance_y = point_y - start_y;
        return distance_x * distance_x + distance_y * distance_y <= radius_squared;
    }

    const PROJECTION_SCALE: i64 = 1024;
    let projection = (point_from_start_x * segment_x + point_from_start_y * segment_y)
        * PROJECTION_SCALE
        / segment_len_squared;
    let projection = projection.clamp(0, PROJECTION_SCALE);

    let closest_x = start_x + (segment_x * projection) / PROJECTION_SCALE;
    let closest_y = start_y + (segment_y * projection) / PROJECTION_SCALE;
    let distance_x = point_x - closest_x;
    let distance_y = point_y - closest_y;

    distance_x * distance_x + distance_y * distance_y <= radius_squared
}

fn point_covered_by_ellipse(point_x: i32, point_y: i32, ellipse: &Ellipse) -> bool {
    let dx = (point_x - ellipse.center.x) as f32;
    let dy = (point_y - ellipse.center.y) as f32;
    let (ax, ay) = ellipse.axis_a;
    let (bx, by) = ellipse.axis_b;

    // Solve ellipse membership via the 2×2 inverse: inside if ||A⁻¹(p-center)||² ≤ 1,
    // rewritten without division as u²+v² ≤ det² for the filled case.
    let u = by * dx - bx * dy;
    let v = ax * dy - ay * dx;
    let det = ax * by - bx * ay;
    let dist_sq = u * u + v * v;
    let det_sq = det * det;

    if ellipse.filled {
        return dist_sq <= det_sq;
    }

    if ellipse.stroke_width == 0 || ellipse.radius <= 0.0 {
        return false;
    }

    let r = ellipse.radius;
    let half_w = ellipse.stroke_width as f32 * 0.5;
    let outer_scale = (r + half_w) / r;
    let inner_scale = if r > half_w { (r - half_w) / r } else { 0.0 };

    dist_sq <= det_sq * outer_scale * outer_scale && dist_sq > det_sq * inner_scale * inner_scale
}
