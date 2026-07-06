#![no_std]

use embedded_graphics::{pixelcolor::Rgb565, prelude::Point, primitives::Rectangle};
use linkage_blaze_core::{DrawItem3d, Projection};
use linkage_blaze_cyd_core::{ContiguousPixels, CydDisplay, DrawItem2d};

/// CYD/linkage projection from core 3D draw items into CYD 2D draw items.
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

/// Compile 3D draw items for indexed pixel lookups by projecting them into the
/// CYD 2D draw-item layer.
#[must_use]
pub fn contiguous_pixels_from_draw_items_3d<const PIXEL_SOURCE_COUNT: usize, I>(
    bounds: Rectangle,
    background: Rgb565,
    draw_items_3d: I,
    projection: &Projection,
) -> ContiguousPixels<PIXEL_SOURCE_COUNT>
where
    I: IntoIterator<Item = DrawItem3d>,
{
    let draw_items_2d = draw_items_3d
        .into_iter()
        .map(|draw_item_3d| draw_item_3d.project(projection));
    ContiguousPixels::from_draw_items_2d(bounds, background, draw_items_2d)
}

/// Linkage/3D helpers layered on top of the 2D [`CydDisplay`] surface.
pub trait CydDisplay3dExt: CydDisplay {
    /// Project and draw 3D draw items immediately inside `bounds`.
    fn draw_items_3d<const PIXEL_SOURCE_COUNT: usize, I>(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        draw_items_3d: I,
        projection: &Projection,
    ) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = DrawItem3d>,
    {
        let contiguous_pixels = self.prepare_draw_items_3d::<PIXEL_SOURCE_COUNT, _>(
            bounds,
            background,
            draw_items_3d,
            projection,
        );
        self.fill_contiguous(contiguous_pixels.bounds(), contiguous_pixels.iter())
    }

    /// Compile 3D draw items for indexed pixel lookups inside `bounds`.
    fn prepare_draw_items_3d<const PIXEL_SOURCE_COUNT: usize, I>(
        &self,
        bounds: Rectangle,
        background: Rgb565,
        draw_items_3d: I,
        projection: &Projection,
    ) -> ContiguousPixels<PIXEL_SOURCE_COUNT>
    where
        I: IntoIterator<Item = DrawItem3d>,
    {
        let bounds = bounds.intersection(&Rectangle::new(Point::zero(), self.screen_size()));
        contiguous_pixels_from_draw_items_3d(bounds, background, draw_items_3d, projection)
    }
}

impl<T: CydDisplay + ?Sized> CydDisplay3dExt for T {}
