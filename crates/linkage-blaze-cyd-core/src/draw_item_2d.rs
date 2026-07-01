use embedded_graphics::{
    Drawable,
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::{IntoStorage, Point, Size},
    primitives::Rectangle,
    primitives::{Circle, Line, Primitive, PrimitiveStyle},
};
use linkage_blaze_core::{
    DrawItem3d, PixelTarget, PixelTargetAdapter, Projection, Rgb888, fill_ellipse_pixels,
    pixel_put, pixel_put_565,
};

/// A statically-stored RGB565 bitmap in row-major order.
#[derive(Clone, Copy, Debug)]
pub struct Image565View {
    pixels: &'static [u16],
    size: Size,
}

impl Image565View {
    /// Create a bitmap descriptor from row-major RGB565 pixels.
    ///
    /// Panics if `pixels.len() != size.width * size.height`.
    #[must_use]
    pub fn new(pixels: &'static [u16], size: Size) -> Self {
        let pixel_count = size.width as usize * size.height as usize;
        assert!(
            pixels.len() == pixel_count,
            "Image565View pixels must match width * height"
        );
        Self { pixels, size }
    }

    #[must_use]
    pub const fn size(&self) -> Size {
        self.size
    }

    #[must_use]
    pub fn pixel_at(&self, point: Point) -> Rgb565 {
        assert!(
            point.x >= 0 && point.y >= 0,
            "Image565View pixel coordinate must be non-negative"
        );
        let position_x = point.x as usize;
        let position_y = point.y as usize;
        assert!(
            position_x < self.size.width as usize && position_y < self.size.height as usize,
            "Image565View pixel coordinate must be inside the bitmap"
        );
        let index = position_y * self.size.width as usize + position_x;
        Rgb565::from(RawU16::new(self.pixels[index]))
    }

    #[must_use]
    pub fn contains(&self, rectangle: Rectangle) -> bool {
        let left = rectangle.top_left.x;
        let top = rectangle.top_left.y;
        let right = left
            .checked_add(rectangle.size.width as i32)
            .expect("bitmap source right edge must fit in i32");
        let bottom = top
            .checked_add(rectangle.size.height as i32)
            .expect("bitmap source bottom edge must fit in i32");
        left >= 0
            && top >= 0
            && right <= self.size.width as i32
            && bottom <= self.size.height as i32
    }
}

/// A rectangular piece of a statically-stored RGB565 bitmap.
#[derive(Clone, Copy, Debug)]
pub struct BitmapItem565 {
    bitmap: Image565View,
    source: Rectangle,
    top_left: Point,
}

impl BitmapItem565 {
    /// Create a bitmap draw item.
    ///
    /// `source` is in bitmap coordinates. `top_left` is the output location of
    /// the source rectangle's top-left corner.
    #[must_use]
    pub fn new(bitmap: Image565View, source: Rectangle, top_left: Point) -> Self {
        assert!(
            bitmap.contains(source),
            "BitmapItem565 source rectangle must be inside the bitmap"
        );
        Self {
            bitmap,
            source,
            top_left,
        }
    }

    #[must_use]
    pub const fn bitmap(&self) -> Image565View {
        self.bitmap
    }

    #[must_use]
    pub const fn source(&self) -> Rectangle {
        self.source
    }

    #[must_use]
    pub const fn top_left(&self) -> Point {
        self.top_left
    }

    #[must_use]
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.top_left, self.source.size)
    }

    #[must_use]
    pub fn pixel_at(&self, point: Point) -> Rgb565 {
        let source_point = self.source.top_left + (point - self.top_left);
        self.bitmap.pixel_at(source_point)
    }
}

/// A pixel-space 2D draw item, ready to draw onto a [`PixelTarget`].
///
/// Obtain one with [`DrawItem3dExt::project`], or construct one directly when
/// you already have pixel-space geometry. All coordinates and sizes are in
/// pixels. The `color` stays [`Rgb888`]; the target performs any conversion
/// (for example to `Rgb565`) at its pixel boundary.
#[derive(Clone, Copy, Debug)]
pub enum DrawItem2d {
    /// A line stroke from `start` to `end` with the given pixel width.
    Stroke {
        start: (f32, f32),
        end: (f32, f32),
        color: Rgb888,
        pixel_width: f32,
    },
    /// A filled, possibly foreshortened, ellipse (a projected disk).
    ///
    /// The ellipse is the locus of `center + s·axis_a + t·axis_b` with `s²+t² ≤ 1`.
    Ellipse {
        center: (f32, f32),
        axis_a: (f32, f32),
        axis_b: (f32, f32),
        color: Rgb888,
    },
    /// A filled circle (a projected sphere).
    Circle {
        center: (f32, f32),
        pixel_radius: f32,
        color: Rgb888,
    },
    /// A rectangular piece of a statically-stored RGB565 bitmap.
    Bitmap(BitmapItem565),
}

impl DrawItem2d {
    /// Draw this item onto a [`PixelTarget`].
    ///
    /// Strokes use the embedded-graphics [`Line`] primitive and circles use
    /// [`Circle`]; the general projected ellipse is rasterized with
    /// [`fill_ellipse_pixels`].
    pub fn draw<T: PixelTarget>(&self, target: &mut T) {
        match *self {
            DrawItem2d::Stroke {
                start,
                end,
                color,
                pixel_width,
            } => {
                let width = ((pixel_width + 0.5) as u32).max(1);
                Line::new(
                    embedded_graphics::prelude::Point::new(start.0 as i32, start.1 as i32),
                    embedded_graphics::prelude::Point::new(end.0 as i32, end.1 as i32),
                )
                .into_styled(PrimitiveStyle::with_stroke(color, width))
                .draw(&mut PixelTargetAdapter(target))
                .expect("drawing onto a PixelTargetAdapter is Infallible");
            }
            DrawItem2d::Ellipse {
                center,
                axis_a,
                axis_b,
                color,
            } => {
                fill_ellipse_pixels(center, axis_a, axis_b, |position_x, position_y| {
                    pixel_put(target, position_x, position_y, color);
                });
            }
            DrawItem2d::Circle {
                center,
                pixel_radius,
                color,
            } => {
                let diameter = (((pixel_radius * 2.0) + 0.5) as u32).max(1);
                Circle::with_center(
                    embedded_graphics::prelude::Point::new(center.0 as i32, center.1 as i32),
                    diameter,
                )
                .into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut PixelTargetAdapter(target))
                .expect("drawing onto a PixelTargetAdapter is Infallible");
            }
            DrawItem2d::Bitmap(bitmap_item) => {
                let source = bitmap_item.source();
                let top_left = bitmap_item.top_left();
                for source_y in 0..source.size.height as i32 {
                    for source_x in 0..source.size.width as i32 {
                        let source_point = source.top_left + Point::new(source_x, source_y);
                        let target_point = top_left + Point::new(source_x, source_y);
                        pixel_put_565(
                            target,
                            target_point.x,
                            target_point.y,
                            bitmap_item.bitmap().pixel_at(source_point).into_storage(),
                        );
                    }
                }
            }
        }
    }
}

/// CYD-layer projection from core 3D draw items into CYD 2D draw items.
pub trait DrawItem3dExt {
    /// Project this 3D/linkage-space item through `projection` into pixel-space.
    #[must_use]
    fn project(self, projection: &Projection) -> DrawItem2d;
}

impl DrawItem3dExt for DrawItem3d {
    fn project(self, projection: &Projection) -> DrawItem2d {
        match self {
            DrawItem3d::Stroke(stroke) => DrawItem2d::Stroke {
                start: stroke.start().project(projection),
                end: stroke.end().project(projection),
                color: stroke.color(),
                pixel_width: projection.project_width(stroke.width()),
            },
            DrawItem3d::Disk(disk) => {
                let orientation = disk.pose().orientation();
                DrawItem2d::Ellipse {
                    center: disk.pose().project(projection),
                    axis_a: projection.project_dir(
                        disk.pose(),
                        orientation.forward(),
                        disk.radius(),
                    ),
                    axis_b: projection.project_dir(disk.pose(), orientation.left(), disk.radius()),
                    color: disk.color(),
                }
            }
            DrawItem3d::Sphere(sphere) => DrawItem2d::Circle {
                center: sphere.pose().project(projection),
                pixel_radius: projection.project_radius(sphere.pose(), sphere.radius()),
                color: sphere.color(),
            },
        }
    }
}
