use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{Point, Size},
    primitives::Rectangle,
};
use micromath::F32Ext;

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
            DrawPrimitive::LineSegment(segment) => {
                if segment.width == 0 {
                    return None;
                }
                let start_x = segment.start.x;
                let start_y = segment.start.y;
                let end_x = segment.end.x;
                let end_y = segment.end.y;
                let segment_x = end_x - start_x;
                let segment_y = end_y - start_y;
                let segment_len_squared = (segment_x as i64) * (segment_x as i64)
                    + (segment_y as i64) * (segment_y as i64);
                let radius = (i64::from(segment.width) + 1) / 2;
                let radius_squared = radius * radius;

                let min_x = start_x.min(end_x) - radius as i32;
                let max_x = start_x.max(end_x) + radius as i32;
                let min_y = start_y.min(end_y) - radius as i32;
                let max_y = start_y.max(end_y) + radius as i32;
                let bounds = Rectangle::new(
                    Point::new(min_x, min_y),
                    Size::new((max_x - min_x + 1) as u32, (max_y - min_y + 1) as u32),
                );

                Some(PreparedPrimitive::Line {
                    bounds,
                    start_x,
                    start_y,
                    segment_x,
                    segment_y,
                    segment_len_squared,
                    radius_squared,
                    color: segment.color,
                })
            }
            DrawPrimitive::Ellipse(ellipse) => {
                let det = ellipse.axis_a.0 * ellipse.axis_b.1 - ellipse.axis_a.1 * ellipse.axis_b.0;
                let det_squared = det * det;

                if det_squared == 0.0 {
                    return None;
                }

                let radius = ellipse.radius;
                let (ax, ay) = ellipse.axis_a;
                let (bx, by) = ellipse.axis_b;

                let (outer_limit, inner_limit) = if ellipse.filled {
                    (det_squared, 0.0)
                } else {
                    let half_width = ellipse.stroke_width as f32 * 0.5;
                    let outer_scale = (radius + half_width) / radius;
                    let inner_scale = if radius > half_width {
                        (radius - half_width) / radius
                    } else {
                        0.0
                    };
                    (
                        det_squared * outer_scale * outer_scale,
                        det_squared * inner_scale * inner_scale,
                    )
                };

                let bounds = Rectangle::new(
                    Point::new(
                        ellipse.center.x - radius.ceil() as i32 - 1,
                        ellipse.center.y - radius.ceil() as i32 - 1,
                    ),
                    Size::new(
                        (2.0 * radius).ceil() as u32 + 2,
                        (2.0 * radius).ceil() as u32 + 2,
                    ),
                );

                Some(PreparedPrimitive::Ellipse {
                    bounds,
                    center_x: ellipse.center.x,
                    center_y: ellipse.center.y,
                    ax,
                    ay,
                    bx,
                    by,
                    outer_limit,
                    inner_limit,
                    filled: ellipse.filled,
                    color: ellipse.color,
                })
            }
        }
    }

    fn bounds(&self) -> Rectangle {
        match *self {
            PreparedPrimitive::Line { bounds, .. } => bounds,
            PreparedPrimitive::Ellipse { bounds, .. } => bounds,
        }
    }
}

pub(crate) struct LineSegmentPixels<'a> {
    x0: i32,
    y0: i32,
    width: usize,
    index: usize,
    pixel_count: usize,
    background: Rgb565,
    segments: &'a [LineSegment],
}

impl<'a> LineSegmentPixels<'a> {
    #[must_use]
    pub(crate) fn new(bounds: Rectangle, background: Rgb565, segments: &'a [LineSegment]) -> Self {
        Self {
            x0: bounds.top_left.x,
            y0: bounds.top_left.y,
            width: bounds.size.width as usize,
            index: 0,
            pixel_count: bounds.size.width as usize * bounds.size.height as usize,
            background,
            segments,
        }
    }
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

pub(crate) struct PrimitivePixels {
    x0: i32,
    y0: i32,
    width: usize,
    index: usize,
    pixel_count: usize,
    background: Rgb565,
    primitives: heapless::Vec<PreparedPrimitive, 16>,
}

impl PrimitivePixels {
    #[must_use]
    pub(crate) fn new(
        bounds: Rectangle,
        background: Rgb565,
        draw_primitives: &[DrawPrimitive],
    ) -> Self {
        let mut primitives = heapless::Vec::<PreparedPrimitive, 16>::new();
        for draw_primitive in draw_primitives {
            if let Some(prepared_primitive) = PreparedPrimitive::from_draw_primitive(draw_primitive)
            {
                primitives
                    .push(prepared_primitive)
                    .expect("at most 16 prepared primitives");
            }
        }

        Self {
            x0: bounds.top_left.x,
            y0: bounds.top_left.y,
            width: bounds.size.width as usize,
            index: 0,
            pixel_count: bounds.size.width as usize * bounds.size.height as usize,
            background,
            primitives,
        }
    }
}

impl Iterator for PrimitivePixels {
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

        for primitive in &self.primitives {
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
                    color: primitive_color,
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
                        color = primitive_color;
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
                    color: primitive_color,
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
                        color = primitive_color;
                    }
                }
            }
        }

        Some(color)
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
    let distance_squared = u * u + v * v;

    if filled {
        distance_squared <= outer_limit
    } else {
        distance_squared <= outer_limit && distance_squared > inner_limit
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
