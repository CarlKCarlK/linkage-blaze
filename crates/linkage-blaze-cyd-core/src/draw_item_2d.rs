use embedded_graphics::{
    Drawable,
    primitives::{Circle, Line, Primitive, PrimitiveStyle},
};
use linkage_blaze_core::{
    DrawItem3d, PixelTarget, PixelTargetAdapter, Projection, Rgb888, fill_ellipse_pixels, pixel_put,
};

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
