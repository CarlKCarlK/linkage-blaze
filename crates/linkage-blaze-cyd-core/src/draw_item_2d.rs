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

/// A view into a statically-stored RGB565 bitmap, optionally cropped to a
/// sub-rectangle.
///
/// For a full-image view use [`Image565Fixed::view`]; for a cropped view use
/// [`Image565Fixed::view_rect`]. `stride` is the full image width (row step in
/// pixels); `source` is the crop rectangle in image coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Image565View {
    pixels: &'static [u16],
    stride: u32,
    source: Rectangle,
}

impl Image565View {
    /// Full-image view from a raw pixel slice.
    ///
    /// Panics if `pixels.len() != size.width * size.height`.
    #[must_use]
    pub const fn new(pixels: &'static [u16], size: Size) -> Self {
        assert!(
            pixels.len() == size.width as usize * size.height as usize,
            "Image565View pixels must match width * height"
        );
        Self {
            pixels,
            stride: size.width,
            source: Rectangle::new(Point::zero(), size),
        }
    }

    /// Cropped view — `source` is in image coordinates, `stride` is the full
    /// image row width. Prefer [`Image565Fixed::view_rect`] at call sites.
    #[must_use]
    pub(crate) const fn new_cropped(
        pixels: &'static [u16],
        stride: u32,
        source: Rectangle,
    ) -> Self {
        Self { pixels, stride, source }
    }

    #[must_use]
    pub const fn size(&self) -> Size {
        self.source.size
    }

    /// Returns the pixel at `point`, where `point` is in view-local coordinates
    /// (i.e. `(0, 0)` is the top-left of this view, not of the underlying image).
    #[must_use]
    pub fn pixel_at(&self, point: Point) -> Rgb565 {
        assert!(
            point.x >= 0 && point.y >= 0,
            "Image565View pixel coordinate must be non-negative"
        );
        let vx = point.x as usize;
        let vy = point.y as usize;
        assert!(
            vx < self.source.size.width as usize && vy < self.source.size.height as usize,
            "Image565View pixel coordinate must be inside the view"
        );
        let source_x = self.source.top_left.x as usize + vx;
        let source_y = self.source.top_left.y as usize + vy;
        let index = source_y * self.stride as usize + source_x;
        Rgb565::from(RawU16::new(self.pixels[index]))
    }
}

/// A view of a static RGB565 bitmap placed at a specific screen position.
///
/// The source crop is baked into the [`Image565View`]; `top_left` is the
/// output position on screen.
#[derive(Clone, Copy, Debug)]
pub struct BitmapItem565 {
    view: Image565View,
    top_left: Point,
}

impl BitmapItem565 {
    /// Place `view` at `top_left` on screen.
    #[must_use]
    pub const fn new(view: Image565View, top_left: Point) -> Self {
        Self { view, top_left }
    }

    #[must_use]
    pub const fn view(&self) -> Image565View {
        self.view
    }

    #[must_use]
    pub const fn top_left(&self) -> Point {
        self.top_left
    }

    #[must_use]
    pub const fn bounds(&self) -> Rectangle {
        Rectangle::new(self.top_left, self.view.source.size)
    }

    /// Returns the pixel at screen-space `point`.
    #[must_use]
    pub fn pixel_at(&self, point: Point) -> Rgb565 {
        self.view.pixel_at(point - self.top_left)
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
                let size = bitmap_item.view().size();
                let top_left = bitmap_item.top_left();
                for dy in 0..size.height as i32 {
                    for dx in 0..size.width as i32 {
                        let view_point = Point::new(dx, dy);
                        let target_point = top_left + view_point;
                        pixel_put_565(
                            target,
                            target_point.x,
                            target_point.y,
                            bitmap_item.view().pixel_at(view_point).into_storage(),
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
