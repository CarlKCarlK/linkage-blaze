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
use heapless;
use micromath::F32Ext;
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation as MipiOrientation, Rotation},
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

#[derive(Clone, Copy, Debug)]
enum PreparedPrimitive {
    Line {
        bounds: Rectangle,
        start_x: i32,
        start_y: i32,
        segment_x: i32,
        segment_y: i32,
        segment_len_squared: i64,
        radius_squared: i64,
        color: Rgb565,
    },
    Ellipse {
        bounds: Rectangle,
        center_x: i32,
        center_y: i32,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        outer_limit: f32,
        inner_limit: f32,
        filled: bool,
        color: Rgb565,
    },
}

impl PreparedPrimitive {
    fn from_draw_primitive(primitive: &DrawPrimitive) -> Option<Self> {
        match *primitive {
            DrawPrimitive::LineSegment(seg) => {
                if seg.width == 0 {
                    return None;
                }
                let start_x = seg.start.x as i32;
                let start_y = seg.start.y as i32;
                let end_x = seg.end.x as i32;
                let end_y = seg.end.y as i32;
                let segment_x = end_x - start_x;
                let segment_y = end_y - start_y;
                let segment_len_squared = (segment_x as i64) * (segment_x as i64)
                    + (segment_y as i64) * (segment_y as i64);
                let radius = ((seg.width as i64) + 1) / 2;
                let radius_squared = radius * radius;

                let min_x = start_x.min(end_x) - radius as i32;
                let max_x = start_x.max(end_x) + radius as i32;
                let min_y = start_y.min(end_y) - radius as i32;
                let max_y = start_y.max(end_y) + radius as i32;
                let bounds = Rectangle::new(
                    Point::new(min_x, min_y),
                    embedded_graphics::prelude::Size::new(
                        (max_x - min_x + 1) as u32,
                        (max_y - min_y + 1) as u32,
                    ),
                );

                Some(PreparedPrimitive::Line {
                    bounds,
                    start_x,
                    start_y,
                    segment_x,
                    segment_y,
                    segment_len_squared,
                    radius_squared,
                    color: seg.color,
                })
            }
            DrawPrimitive::Ellipse(ell) => {
                let det = ell.axis_a.0 * ell.axis_b.1 - ell.axis_a.1 * ell.axis_b.0;
                let det_sq = det * det;

                if det_sq == 0.0 {
                    return None;
                }

                let r = ell.radius;
                let (ax, ay) = ell.axis_a;
                let (bx, by) = ell.axis_b;

                let (outer_limit, inner_limit) = if ell.filled {
                    (det_sq, 0.0)
                } else {
                    let half_w = ell.stroke_width as f32 * 0.5;
                    let outer_scale = (r + half_w) / r;
                    let inner_scale = if r > half_w { (r - half_w) / r } else { 0.0 };
                    (
                        det_sq * outer_scale * outer_scale,
                        det_sq * inner_scale * inner_scale,
                    )
                };

                let bounds = Rectangle::new(
                    Point::new(
                        ell.center.x - r.ceil() as i32 - 1,
                        ell.center.y - r.ceil() as i32 - 1,
                    ),
                    embedded_graphics::prelude::Size::new(
                        (2.0 * r).ceil() as u32 + 2,
                        (2.0 * r).ceil() as u32 + 2,
                    ),
                );

                Some(PreparedPrimitive::Ellipse {
                    bounds,
                    center_x: ell.center.x,
                    center_y: ell.center.y,
                    ax,
                    ay,
                    bx,
                    by,
                    outer_limit,
                    inner_limit,
                    filled: ell.filled,
                    color: ell.color,
                })
            }
        }
    }
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
    primitives: &'a [PreparedPrimitive],
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
        let point = Point::new(point_x, point_y);

        for primitive in self.primitives {
            if !primitive.bounds().contains(point) {
                continue;
            }

            match *primitive {
                PreparedPrimitive::Line {
                    start_x,
                    start_y,
                    segment_x,
                    segment_y,
                    segment_len_squared,
                    radius_squared,
                    color: prim_color,
                    ..
                } => {
                    if point_covered_by_prepared_segment(
                        point_x,
                        point_y,
                        start_x,
                        start_y,
                        segment_x,
                        segment_y,
                        segment_len_squared,
                        radius_squared,
                    ) {
                        color = prim_color;
                    }
                }
                PreparedPrimitive::Ellipse {
                    center_x,
                    center_y,
                    ax,
                    ay,
                    bx,
                    by,
                    outer_limit,
                    inner_limit,
                    filled,
                    color: prim_color,
                    ..
                } => {
                    if point_covered_by_prepared_ellipse(
                        point_x,
                        point_y,
                        center_x,
                        center_y,
                        ax,
                        ay,
                        bx,
                        by,
                        outer_limit,
                        inner_limit,
                        filled,
                    ) {
                        color = prim_color;
                    }
                }
            }
        }

        Some(color)
    }
}

impl PreparedPrimitive {
    fn bounds(&self) -> Rectangle {
        match *self {
            PreparedPrimitive::Line { bounds, .. } => bounds,
            PreparedPrimitive::Ellipse { bounds, .. } => bounds,
        }
    }
}

pub struct CydDisplay {
    display: CydDisplayDevice,
    screen_size: Size,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Orientation {
    // todo000 later support a full algebra of rotations and flips like the
    // device-envoy 2D LED panel orientation model.
    Landscape,
    Portrait,
    LandscapeInverted,
    PortraitInverted,
}

impl Orientation {
    #[must_use]
    pub const fn width(self) -> usize {
        match self {
            Self::Landscape | Self::LandscapeInverted => SCREEN_WIDTH,
            Self::Portrait | Self::PortraitInverted => SCREEN_HEIGHT,
        }
    }

    #[must_use]
    pub const fn height(self) -> usize {
        match self {
            Self::Landscape | Self::LandscapeInverted => SCREEN_HEIGHT,
            Self::Portrait | Self::PortraitInverted => SCREEN_WIDTH,
        }
    }

    #[must_use]
    pub const fn size(self) -> Size {
        Size::new(self.width() as u32, self.height() as u32)
    }

    #[must_use]
    pub const fn pixels(self) -> usize {
        self.width() * self.height()
    }
}

impl CydDisplay {
    /// Oriented screen size stored at init time.
    #[must_use]
    pub const fn size(&self) -> Size {
        self.screen_size
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
        orientation: Orientation,
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

        let screen_size = orientation.size();
        let display_orientation = match orientation {
            Orientation::Landscape => MipiOrientation::new()
                .rotate(Rotation::Deg90)
                .flip_horizontal()
                .rotate(Rotation::Deg180),
            Orientation::Portrait => MipiOrientation::new()
                .rotate(Rotation::Deg180)
                .flip_horizontal(),
            // todo000 verify on device; provisional 180° rotation of Landscape.
            Orientation::LandscapeInverted => MipiOrientation::new()
                .rotate(Rotation::Deg90)
                .flip_horizontal(),
            // todo000 verify on device; provisional 180° rotation of Portrait.
            Orientation::PortraitInverted => MipiOrientation::new().flip_horizontal(),
        };

        let display = Builder::new(ILI9341Rgb565, interface)
            .reset_pin(rst)
            .display_size(240, 320)
            .color_order(ColorOrder::Bgr)
            .orientation(display_orientation)
            .init(&mut delay)
            .map_err(|_| CydDisplayInitError::InitDisplay)?;

        backlight.set_high();

        Ok(CydDisplay {
            display,
            screen_size,
        })
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

    pub fn clear(&mut self, color: Rgb565) -> Result<(), CydDisplayFlushError> {
        self.fill_rect(Rectangle::new(Point::new(0, 0), self.screen_size), color)
    }

    pub fn fill_rect(
        &mut self,
        rectangle: Rectangle,
        color: Rgb565,
    ) -> Result<(), CydDisplayFlushError> {
        let screen_rectangle = Rectangle::new(Point::new(0, 0), self.screen_size);
        let rectangle = rectangle.intersection(&screen_rectangle);
        if rectangle.size.width == 0 || rectangle.size.height == 0 {
            return Ok(());
        }
        self.display
            .fill_solid(&rectangle, color)
            .map_err(|_| CydDisplayFlushError::FlushFrameBuffer)
    }

    pub fn fill_contiguous<I>(
        &mut self,
        rectangle: Rectangle,
        pixels: I,
    ) -> Result<(), CydDisplayFlushError>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        self.display
            .fill_contiguous(&rectangle, pixels)
            .map_err(|_| CydDisplayFlushError::FlushFrameBuffer)
    }

    pub fn draw_line_segments(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        segments: &[LineSegment],
    ) -> Result<(), CydDisplayFlushError> {
        let screen_rectangle = Rectangle::new(Point::new(0, 0), self.screen_size);
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

    pub fn draw_primitives(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        draw_primitives: &[DrawPrimitive],
    ) -> Result<(), CydDisplayFlushError> {
        let screen_rectangle = Rectangle::new(Point::new(0, 0), self.screen_size);
        let bounds = bounds.intersection(&screen_rectangle);
        if bounds.size.width == 0 || bounds.size.height == 0 {
            return Ok(());
        }

        // Prepare primitives (precompute expensive constants, bounds-check)
        let mut prepared = heapless::Vec::<PreparedPrimitive, 16>::new();
        for prim in draw_primitives {
            if let Some(prep) = PreparedPrimitive::from_draw_primitive(prim) {
                prepared.push(prep).expect("at most 16 prepared primitives");
            }
        }

        let pixel_count = bounds.size.width as usize * bounds.size.height as usize;
        let pixels = PrimitivePixels {
            x0: bounds.top_left.x,
            y0: bounds.top_left.y,
            width: bounds.size.width as usize,
            index: 0,
            pixel_count,
            background,
            primitives: &prepared,
        };

        self.display
            .fill_contiguous(&bounds, pixels)
            .map_err(|_| CydDisplayFlushError::FlushFrameBuffer)
    }
}

fn point_covered_by_prepared_segment(
    point_x: i32,
    point_y: i32,
    start_x: i32,
    start_y: i32,
    segment_x: i32,
    segment_y: i32,
    segment_len_squared: i64,
    radius_squared: i64,
) -> bool {
    if segment_len_squared == 0 {
        let distance_x = (point_x - start_x) as i64;
        let distance_y = (point_y - start_y) as i64;
        return distance_x * distance_x + distance_y * distance_y <= radius_squared;
    }

    const PROJECTION_SCALE: i64 = 1024;
    let point_from_start_x = (point_x - start_x) as i64;
    let point_from_start_y = (point_y - start_y) as i64;
    let projection = (point_from_start_x * (segment_x as i64)
        + point_from_start_y * (segment_y as i64))
        * PROJECTION_SCALE
        / segment_len_squared;
    let projection = projection.clamp(0, PROJECTION_SCALE);

    let closest_x = start_x as i64 + ((segment_x as i64) * projection) / PROJECTION_SCALE;
    let closest_y = start_y as i64 + ((segment_y as i64) * projection) / PROJECTION_SCALE;
    let distance_x = (point_x as i64) - closest_x;
    let distance_y = (point_y as i64) - closest_y;

    distance_x * distance_x + distance_y * distance_y <= radius_squared
}

fn point_covered_by_prepared_ellipse(
    point_x: i32,
    point_y: i32,
    center_x: i32,
    center_y: i32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    outer_limit: f32,
    inner_limit: f32,
    filled: bool,
) -> bool {
    let dx = (point_x - center_x) as f32;
    let dy = (point_y - center_y) as f32;

    let u = by * dx - bx * dy;
    let v = ax * dy - ay * dx;
    let dist_sq = u * u + v * v;

    if filled {
        dist_sq <= outer_limit
    } else {
        dist_sq <= outer_limit && dist_sq > inner_limit
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
