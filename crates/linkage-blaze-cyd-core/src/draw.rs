use embedded_graphics::{pixelcolor::Rgb565, prelude::Point, primitives::Rectangle};
use linkage_blaze_core::{DrawItem2d, DrawItem3d, Projection};
use micromath::F32Ext;

#[derive(Clone, Copy, Debug)]
pub struct LineSegment {
    pub start: Point,
    pub end: Point,
    pub width: u16,
    pub color: Rgb565,
}

#[derive(Clone, Copy, Debug)]
struct PreparedBounds {
    left: i32,
    top: i32,
    right_exclusive: i32,
    bottom_exclusive: i32,
}

impl PreparedBounds {
    fn contains(&self, point_x: i32, point_y: i32) -> bool {
        self.contains_x(point_x) && self.contains_y(point_y)
    }

    fn contains_x(&self, point_x: i32) -> bool {
        self.left <= point_x && point_x < self.right_exclusive
    }

    fn contains_y(&self, point_y: i32) -> bool {
        self.top <= point_y && point_y < self.bottom_exclusive
    }
}

#[derive(Clone, Copy, Debug)]
struct PreparedPrimitive {
    bounds: PreparedBounds,
    color: Rgb565,
    kind: PreparedKind,
}

#[derive(Clone, Copy, Debug)]
enum PreparedKind {
    Line(PreparedLine),
    Ellipse(PreparedEllipse),
}

#[derive(Clone, Copy, Debug)]
struct PreparedLine {
    start_x: i64,
    start_y: i64,
    end_x: i64,
    end_y: i64,
    segment_x: i64,
    segment_y: i64,
    segment_len_squared: i64,
    radius_squared: i64,
    radius_squared_times_len_squared: i64,
}

#[derive(Clone, Copy, Debug)]
struct PreparedEllipse {
    center_x: i32,
    center_y: i32,
    quadratic_xx: f32,
    quadratic_xy: f32,
    quadratic_yy: f32,
    outer_limit: f32,
}

impl PreparedPrimitive {
    // todo: review the color (Rgb888 -> Rgb565) and number (f32 -> i32/u16)
    // conversions threaded through here and `DrawItem2d`. Some may be
    // happening later (per primitive, per frame) than strictly needed — see if
    // any can move once, earlier, or be dropped entirely.
    fn from_projected(item: &DrawItem2d) -> Option<Self> {
        match *item {
            DrawItem2d::Stroke {
                start,
                end,
                color,
                pixel_width,
            } => {
                let start = point_from_f32(start);
                let end = point_from_f32(end);
                let width = pixel_width_u16(pixel_width);
                if start == end {
                    return None;
                }
                let start_x = start.x;
                let start_y = start.y;
                let end_x = end.x;
                let end_y = end.y;
                let segment_x = i64::from(end_x - start_x);
                let segment_y = i64::from(end_y - start_y);
                let segment_len_squared = segment_x * segment_x + segment_y * segment_y;
                let radius = (i64::from(width) + 1) / 2;
                let radius_squared = radius * radius;

                let min_x = start_x.min(end_x) - radius as i32;
                let max_x = start_x.max(end_x) + radius as i32;
                let min_y = start_y.min(end_y) - radius as i32;
                let max_y = start_y.max(end_y) + radius as i32;

                Some(PreparedPrimitive {
                    bounds: PreparedBounds {
                        left: min_x,
                        top: min_y,
                        right_exclusive: max_x + 1,
                        bottom_exclusive: max_y + 1,
                    },
                    color: Rgb565::from(color),
                    kind: PreparedKind::Line(PreparedLine {
                        start_x: i64::from(start_x),
                        start_y: i64::from(start_y),
                        end_x: i64::from(end_x),
                        end_y: i64::from(end_y),
                        segment_x,
                        segment_y,
                        segment_len_squared,
                        radius_squared,
                        radius_squared_times_len_squared: radius_squared
                            .checked_mul(segment_len_squared)
                            .expect("prepared line radius and length product must fit in i64"),
                    }),
                })
            }
            DrawItem2d::Ellipse {
                center,
                axis_a,
                axis_b,
                color,
            } => Self::prepare_ellipse(
                point_from_f32(center),
                axis_a,
                axis_b,
                ellipse_bound_radius(axis_a, axis_b),
                Rgb565::from(color),
            ),
            DrawItem2d::Circle {
                center,
                pixel_radius,
                color,
            } => Self::prepare_ellipse(
                point_from_f32(center),
                (pixel_radius, 0.0),
                (0.0, pixel_radius),
                pixel_radius,
                Rgb565::from(color),
            ),
        }
    }

    fn prepare_ellipse(
        center: Point,
        axis_a: (f32, f32),
        axis_b: (f32, f32),
        radius: f32,
        color: Rgb565,
    ) -> Option<Self> {
        let det = axis_a.0 * axis_b.1 - axis_a.1 * axis_b.0;
        let det_squared = det * det;

        if det_squared == 0.0 {
            return None;
        }

        let (ax, ay) = axis_a;
        let (bx, by) = axis_b;
        let quadratic_xx = by * by + ay * ay;
        let quadratic_xy = -2.0 * (by * bx + ay * ax);
        let quadratic_yy = bx * bx + ax * ax;

        let radius = radius.ceil() as i32 + 1;
        let left = center.x - radius;
        let top = center.y - radius;
        let right_exclusive = center.x + radius;
        let bottom_exclusive = center.y + radius;

        Some(PreparedPrimitive {
            bounds: PreparedBounds {
                left,
                top,
                right_exclusive,
                bottom_exclusive,
            },
            color,
            kind: PreparedKind::Ellipse(PreparedEllipse {
                center_x: center.x,
                center_y: center.y,
                quadratic_xx,
                quadratic_xy,
                quadratic_yy,
                outer_limit: det_squared,
            }),
        })
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

pub struct ContiguousPixels<const PIXEL_SOURCE_COUNT: usize> {
    bounds: Rectangle,
    left: i32,
    top: i32,
    right_exclusive: i32,
    bottom_exclusive: i32,
    background: Rgb565,
    primitives: heapless::Vec<PreparedPrimitive, PIXEL_SOURCE_COUNT>,
}

impl<const PIXEL_SOURCE_COUNT: usize> ContiguousPixels<PIXEL_SOURCE_COUNT> {
    /// Project and compile 3D draw items for indexed pixel lookups.
    #[must_use]
    pub fn from_draw_items_3d<I>(
        bounds: Rectangle,
        background: Rgb565,
        draw_items: I,
        projection: &Projection,
    ) -> Self
    where
        I: IntoIterator<Item = DrawItem3d>,
    {
        let mut primitives = heapless::Vec::<PreparedPrimitive, PIXEL_SOURCE_COUNT>::new();
        for draw_item in draw_items {
            let projected_draw_item = draw_item.project(projection);
            if let Some(prepared_primitive) =
                PreparedPrimitive::from_projected(&projected_draw_item)
            {
                primitives
                    .push(prepared_primitive)
                    .expect("draw items fit the prepared primitive capacity");
            }
        }

        Self::new(bounds, background, primitives)
    }

    /// Compile already-projected draw items for indexed pixel lookups.
    #[must_use]
    pub fn from_draw_items_2d<I>(bounds: Rectangle, background: Rgb565, draw_items: I) -> Self
    where
        I: IntoIterator<Item = DrawItem2d>,
    {
        let mut primitives = heapless::Vec::<PreparedPrimitive, PIXEL_SOURCE_COUNT>::new();
        for draw_item in draw_items {
            if let Some(prepared_primitive) = PreparedPrimitive::from_projected(&draw_item) {
                primitives
                    .push(prepared_primitive)
                    .expect("projected draw items fit the prepared primitive capacity");
            }
        }

        Self::new(bounds, background, primitives)
    }

    fn new(
        bounds: Rectangle,
        background: Rgb565,
        primitives: heapless::Vec<PreparedPrimitive, PIXEL_SOURCE_COUNT>,
    ) -> Self {
        let left = bounds.top_left.x;
        let top = bounds.top_left.y;
        let right_exclusive = left
            .checked_add(bounds.size.width as i32)
            .expect("compiled primitive bounds right edge must fit in i32");
        let bottom_exclusive = top
            .checked_add(bounds.size.height as i32)
            .expect("compiled primitive bounds bottom edge must fit in i32");

        Self {
            bounds,
            left,
            top,
            right_exclusive,
            bottom_exclusive,
            background,
            primitives,
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.left >= self.right_exclusive || self.top >= self.bottom_exclusive
    }

    #[must_use]
    pub fn pixel_at(&self, point_x: i32, point_y: i32) -> Rgb565 {
        for primitive in self.primitives.iter().rev() {
            if primitive.covers(point_x, point_y) {
                return primitive.color;
            }
        }

        self.background
    }

    #[must_use]
    pub fn iter(&self) -> ContiguousPixelsIter<'_, PIXEL_SOURCE_COUNT> {
        ContiguousPixelsIter::new(self)
    }
}

impl PreparedPrimitive {
    fn covers(&self, point_x: i32, point_y: i32) -> bool {
        if !self.bounds.contains(point_x, point_y) {
            return false;
        }

        self.covers_inside_bounds(point_x, point_y)
    }

    fn covers_inside_bounds(&self, point_x: i32, point_y: i32) -> bool {
        match self.kind {
            PreparedKind::Line(line) => line.covers(point_x, point_y),
            PreparedKind::Ellipse(ellipse) => ellipse.covers(point_x, point_y),
        }
    }
}

impl PreparedLine {
    fn covers(&self, point_x: i32, point_y: i32) -> bool {
        point_covered_by_prepared_segment_fast(
            point_x,
            point_y,
            self.start_x,
            self.start_y,
            self.end_x,
            self.end_y,
            self.segment_x,
            self.segment_y,
            self.segment_len_squared,
            self.radius_squared,
            self.radius_squared_times_len_squared,
        )
    }
}

impl PreparedEllipse {
    fn covers(&self, point_x: i32, point_y: i32) -> bool {
        point_covered_by_prepared_ellipse(
            point_x,
            point_y,
            self.center_x,
            self.center_y,
            self.quadratic_xx,
            self.quadratic_xy,
            self.quadratic_yy,
            self.outer_limit,
        )
    }
}

pub struct ContiguousPixelsIter<'a, const PIXEL_SOURCE_COUNT: usize> {
    pixels: &'a ContiguousPixels<PIXEL_SOURCE_COUNT>,
    x: i32,
    y: i32,
    active: heapless::Vec<&'a PreparedPrimitive, PIXEL_SOURCE_COUNT>,
}

impl<'a, const PIXEL_SOURCE_COUNT: usize> ContiguousPixelsIter<'a, PIXEL_SOURCE_COUNT> {
    fn new(pixels: &'a ContiguousPixels<PIXEL_SOURCE_COUNT>) -> Self {
        let mut contiguous_pixels_iter = Self {
            pixels,
            x: pixels.left,
            y: pixels.top,
            active: heapless::Vec::new(),
        };
        contiguous_pixels_iter.rebuild_active();
        contiguous_pixels_iter
    }

    fn rebuild_active(&mut self) {
        self.active.clear();
        for primitive in self.pixels.primitives.iter().rev() {
            if primitive.bounds.contains_y(self.y) {
                self.active
                    .push(primitive)
                    .expect("active primitive references fit active list capacity");
            }
        }
    }
}

impl<const PIXEL_SOURCE_COUNT: usize> Iterator for ContiguousPixelsIter<'_, PIXEL_SOURCE_COUNT> {
    type Item = Rgb565;

    fn next(&mut self) -> Option<Self::Item> {
        if self.x >= self.pixels.right_exclusive || self.y >= self.pixels.bottom_exclusive {
            return None;
        }

        let mut color = self.pixels.background;
        for primitive in &self.active {
            if !primitive.bounds.contains_x(self.x) {
                continue;
            }
            if primitive.covers_inside_bounds(self.x, self.y) {
                color = primitive.color;
                break;
            }
        }

        self.x += 1;
        if self.x >= self.pixels.right_exclusive {
            self.x = self.pixels.left;
            self.y += 1;
            if self.y < self.pixels.bottom_exclusive {
                self.rebuild_active();
            }
        }

        Some(color)
    }
}

fn point_covered_by_prepared_segment_fast(
    point_x: i32,
    point_y: i32,
    start_x: i64,
    start_y: i64,
    end_x: i64,
    end_y: i64,
    segment_x: i64,
    segment_y: i64,
    segment_len_squared: i64,
    radius_squared: i64,
    radius_squared_times_len_squared: i64,
) -> bool {
    let point_x = i64::from(point_x);
    let point_y = i64::from(point_y);
    let point_from_start_x = point_x - start_x;
    let point_from_start_y = point_y - start_y;

    let dot = point_from_start_x * segment_x + point_from_start_y * segment_y;
    if dot <= 0 {
        return point_from_start_x * point_from_start_x + point_from_start_y * point_from_start_y
            <= radius_squared;
    }

    if dot >= segment_len_squared {
        let distance_x = point_x - end_x;
        let distance_y = point_y - end_y;
        return distance_x * distance_x + distance_y * distance_y <= radius_squared;
    }

    let cross = point_from_start_x * segment_y - point_from_start_y * segment_x;
    cross * cross <= radius_squared_times_len_squared
}

fn point_covered_by_prepared_ellipse(
    point_x: i32,
    point_y: i32,
    center_x: i32,
    center_y: i32,
    quadratic_xx: f32,
    quadratic_xy: f32,
    quadratic_yy: f32,
    outer_limit: f32,
) -> bool {
    let dx = (point_x - center_x) as f32;
    let dy = (point_y - center_y) as f32;
    let distance_squared = quadratic_xx * dx * dx + quadratic_xy * dx * dy + quadratic_yy * dy * dy;

    distance_squared <= outer_limit
}

fn point_from_f32((x, y): (f32, f32)) -> Point {
    Point::new(x as i32, y as i32)
}

fn pixel_width_u16(width: f32) -> u16 {
    ((width + 0.5) as u16).max(1)
}

fn ellipse_bound_radius(axis_a: (f32, f32), axis_b: (f32, f32)) -> f32 {
    (axis_a.0.abs() + axis_b.0.abs()).max(axis_a.1.abs() + axis_b.1.abs())
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
