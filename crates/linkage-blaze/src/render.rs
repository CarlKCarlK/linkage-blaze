//! Evaluated three-dimensional drawing geometry and projection helpers.
//!
//! [`Item3d`] values are produced by [`crate::LinkageView::draw_items_3d`].
//! Use [`Item3d::project`] with a [`Projection`] to get a pixel-space
//! [`Item2d`], which draws onto any embedded-graphics
//! [`DrawTarget`]. You can also
//! inspect either item's payload directly in another renderer.
//!
//! # Rendering evaluated items
//!
//! ```rust
//! # use embedded_graphics::{mock_display::MockDisplay, prelude::*};
//! # use linkage_blaze::{LinkageFixed, Rgb888, Vec3};
//! # use linkage_blaze::render::{Item2d, Item3d, Projection};
//! # fn main() -> Result<(), linkage_blaze::Error> {
//! const LINKAGE: LinkageFixed<0, 0, 5> = LinkageFixed::start()
//!     .forward(1.0)
//!     .disk(0.25)
//!     .sphere(0.5);
//! let projection = Projection::front_orthographic(Point::new(0, 0), 10.0);
//! let view = LINKAGE.view();
//! let mut items = view.draw_items_3d(&[])?;
//! match items.next() {
//!     Some(Item3d::Stroke(stroke)) => {
//!         let item_2d = Item3d::Stroke(stroke).project(&projection);
//!         let _ = (stroke.start(), stroke.end(), stroke.color(), stroke.width());
//!         // Draw onto any embedded-graphics target whose color is `Rgb888`
//!         // (use `.color_converted()` for other color types).
//!         let mut display = MockDisplay::<Rgb888>::new();
//!         display.set_allow_out_of_bounds_drawing(true);
//!         item_2d.draw(&mut display).unwrap_or_else(|infallible| match infallible {});
//!         if let Item2d::Stroke { start, end, .. } = item_2d {
//!             let _ = (start, end);
//!         }
//!     }
//!     _ => return Err(linkage_blaze::Error::EmptyLinkage),
//! }
//! match items.next() {
//!     Some(Item3d::Disk(disk)) => {
//!         let _ = (disk.pose(), disk.radius(), disk.color());
//!     }
//!     _ => return Err(linkage_blaze::Error::EmptyLinkage),
//! }
//! match items.next() {
//!     Some(Item3d::Sphere(sphere)) => {
//!         let _ = (sphere.pose(), sphere.radius(), sphere.color());
//!     }
//!     _ => return Err(linkage_blaze::Error::EmptyLinkage),
//! }
//! let _screen_direction = projection.project_dir(
//!     linkage_blaze::Pose::start(),
//!     Vec3::from([1.0, 0.0, 0.0]),
//!     1.0,
//! );
//! # Ok(())
//! # }
//! ```

use super::{Pose, Rgb888, Vec3};
use embedded_graphics::{
    Pixel,
    prelude::{DrawTarget, Drawable, Point, Primitive},
    primitives::{Circle, Line, PrimitiveStyle},
};

/// A drawable pen-down movement emitted while evaluating a linkage.
#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub(crate) start: Pose,
    pub(crate) end: Pose,
    pub(crate) color: Rgb888,
    pub(crate) width: f32,
}

impl Stroke {
    /// Return the pose at the start of the segment.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub const fn start(self) -> Pose {
        self.start
    }
    /// Return the pose at the end of the segment.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub const fn end(self) -> Pose {
        self.end
    }
    /// Return the segment color.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }
    /// Return the segment width in linkage units.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }
}

/// A filled disk emitted while evaluating a linkage.
#[derive(Clone, Copy, Debug)]
pub struct Disk {
    pub(crate) pose: Pose,
    pub(crate) radius: f32,
    pub(crate) color: Rgb888,
}

impl Disk {
    /// Return the disk center pose.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    /// Return the disk radius in linkage units.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    /// Return the disk color.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub const fn color(self) -> Rgb888 {
        self.color
    }
}

/// A sphere emitted while evaluating a linkage.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub(crate) pose: Pose,
    pub(crate) radius: f32,
    pub(crate) color: Rgb888,
}

impl Sphere {
    /// Return the sphere center pose.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    /// Return the sphere radius in linkage units.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    /// Return the sphere color.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub const fn color(self) -> Rgb888 {
        self.color
    }
}

/// Three-dimensional geometry emitted while evaluating a linkage.
#[derive(Clone, Copy, Debug)]
pub enum Item3d {
    /// A pen-down movement represented as a colored segment.
    Stroke(Stroke),
    /// A filled disk at the current pose.
    Disk(Disk),
    /// A sphere centered at the current pose.
    Sphere(Sphere),
}

impl Item3d {
    /// Project this item into a pixel-space [`Item2d`].
    ///
    /// The projection controls the camera orientation, scale, target pixel,
    /// and optional perspective depth scaling.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub fn project(self, projection: &Projection) -> Item2d {
        match self {
            Self::Stroke(stroke) => Item2d::Stroke {
                start: stroke.start().project(projection),
                end: stroke.end().project(projection),
                color: stroke.color(),
                pixel_width: projection.project_width(stroke.width()),
            },
            Self::Disk(disk) => {
                let orientation = disk.pose().orientation();
                Item2d::Ellipse {
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
            Self::Sphere(sphere) => Item2d::Circle {
                center: sphere.pose().project(projection),
                pixel_radius: projection.project_radius(sphere.pose(), sphere.radius()),
                color: sphere.color(),
            },
        }
    }
}

/// Two-dimensional, pixel-space geometry produced by [`Item3d::project`].
///
/// `Item2d` implements embedded-graphics [`Drawable`] with
/// [`Rgb888`] color, so it draws onto any
/// [`DrawTarget`]. For a display with another color type, such as `Rgb565`,
/// draw through `target.color_converted()`.
/// See the [rendering evaluated items example](#rendering-evaluated-items).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item2d {
    /// A line stroke from `start` to `end` with the given pixel width.
    Stroke {
        /// Start point in pixel coordinates.
        start: (f32, f32),
        /// End point in pixel coordinates.
        end: (f32, f32),
        /// Stroke color.
        color: Rgb888,
        /// Stroke width in pixels.
        pixel_width: f32,
    },
    /// A filled ellipse, such as a projected disk.
    ///
    /// The ellipse is the locus of `center + s·axis_a + t·axis_b` with `s²+t² ≤ 1`.
    Ellipse {
        /// Center in pixel coordinates.
        center: (f32, f32),
        /// First radius vector, measured in pixels.
        axis_a: (f32, f32),
        /// Second radius vector, measured in pixels.
        axis_b: (f32, f32),
        /// Fill color.
        color: Rgb888,
    },
    /// A filled circle, such as a projected sphere.
    Circle {
        /// Center in pixel coordinates.
        center: (f32, f32),
        /// Radius in pixels.
        pixel_radius: f32,
        /// Fill color.
        color: Rgb888,
    },
}

impl Drawable for Item2d {
    type Color = Rgb888;
    type Output = ();

    /// Rasterize this item onto `target`.
    ///
    /// Strokes use the embedded-graphics [`Line`] primitive and circles use
    /// [`Circle`]; ellipses are rasterized pixel by pixel. Edge-on (degenerate)
    /// ellipses draw nothing.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        match *self {
            Self::Stroke {
                start,
                end,
                color,
                pixel_width,
            } => {
                let width = ((pixel_width + 0.5) as u32).max(1);
                Line::new(
                    Point::new(start.0 as i32, start.1 as i32),
                    Point::new(end.0 as i32, end.1 as i32),
                )
                .into_styled(PrimitiveStyle::with_stroke(color, width))
                .draw(target)
            }
            Self::Ellipse {
                center,
                axis_a,
                axis_b,
                color,
            } => target
                .draw_iter(ellipse_points(center, axis_a, axis_b).map(|point| Pixel(point, color))),
            Self::Circle {
                center,
                pixel_radius,
                color,
            } => {
                let diameter = (((pixel_radius * 2.0) + 0.5) as u32).max(1);
                Circle::with_center(Point::new(center.0 as i32, center.1 as i32), diameter)
                    .into_styled(PrimitiveStyle::with_fill(color))
                    .draw(target)
            }
        }
    }
}

/// Points inside the ellipse `center + s·axis_a + t·axis_b` with `s²+t² ≤ 1`,
/// in row-major order. Yields nothing for a degenerate (edge-on) ellipse.
fn ellipse_points(
    center: (f32, f32),
    axis_a: (f32, f32),
    axis_b: (f32, f32),
) -> impl Iterator<Item = Point> {
    let (axis_ax, axis_ay) = axis_a;
    let (axis_bx, axis_by) = axis_b;
    let determinant = axis_ax * axis_by - axis_ay * axis_bx;
    let inverse_determinant = 1.0 / determinant;
    let bound_x = (axis_ax.abs() + axis_bx.abs()) as i32 + 1;
    let bound_y = (axis_ay.abs() + axis_by.abs()) as i32 + 1;
    // A degenerate (edge-on) ellipse scans no rows.
    let rows = (determinant.abs() >= 0.5)
        .then_some(-bound_y..=bound_y)
        .into_iter()
        .flatten();
    let center_x = center.0 as i32;
    let center_y = center.1 as i32;
    rows.flat_map(move |local_y| {
        (-bound_x..=bound_x).filter_map(move |local_x| {
            let delta_x = local_x as f32;
            let delta_y = local_y as f32;
            let s = (axis_by * delta_x - axis_bx * delta_y) * inverse_determinant;
            let t = (axis_ax * delta_y - axis_ay * delta_x) * inverse_determinant;
            (s * s + t * t <= 1.0).then(|| Point::new(center_x + local_x, center_y + local_y))
        })
    })
}

/// Camera projection from Linkage Blaze world coordinates to pixel coordinates.
///
/// The rotation maps world axes onto camera axes: row 0 is depth, row 1 is the
/// source of screen X, and row 2 is the source of screen Y. Named constructors
/// provide orthographic front/top views and a perspective front view.
///
/// Use [`front_orthographic`](Self::front_orthographic) for a stable front
/// camera, [`top_orthographic`](Self::top_orthographic) for a top camera, or
/// [`front_perspective`](Self::front_perspective) when depth should affect
/// scale. [`Item3d::project`] and [`crate::Pose::project`] consume the result.
pub struct Projection {
    pub(crate) rotation: super::Mat3,
    pub(crate) target_origin: Point,
    pub(crate) scale: f32,
    /// `None` is orthographic; `Some(focal)` is perspective.
    pub(crate) focal: Option<f32>,
}

const NEG_X_BASIS: super::Mat3 = super::Mat3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
const NEG_Z_BASIS: super::Mat3 = super::Mat3([[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]);

impl Projection {
    /// Create an orthographic front view looking along negative X.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub const fn front_orthographic(target_origin: Point, scale: f32) -> Self {
        Self {
            rotation: NEG_X_BASIS,
            target_origin,
            scale,
            focal: None,
        }
    }

    /// Create an orthographic top view looking along negative Z.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub const fn top_orthographic(target_origin: Point, scale: f32) -> Self {
        Self {
            rotation: NEG_Z_BASIS,
            target_origin,
            scale,
            focal: None,
        }
    }

    /// Create a perspective front view looking along negative X.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub const fn front_perspective(target_origin: Point, scale: f32, focal: f32) -> Self {
        Self {
            rotation: NEG_X_BASIS,
            target_origin,
            scale,
            focal: Some(focal),
        }
    }

    pub(crate) fn world_to_camera(&self, vector: Vec3) -> [f32; 3] {
        let rotation = &self.rotation;
        [
            rotation[0][0] * vector[0] + rotation[0][1] * vector[1] + rotation[0][2] * vector[2],
            rotation[1][0] * vector[0] + rotation[1][1] * vector[1] + rotation[1][2] * vector[2],
            rotation[2][0] * vector[0] + rotation[2][1] * vector[1] + rotation[2][2] * vector[2],
        ]
    }

    pub(crate) fn depth_factor(&self, depth: f32) -> f32 {
        match self.focal {
            None => 1.0,
            Some(focal) => focal / (focal + depth).max(focal * 0.05),
        }
    }

    /// Project a world-space direction, scaled by a world-space radius.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub fn project_dir(&self, pose: Pose, world_dir: Vec3, radius: f32) -> (f32, f32) {
        let factor = self.depth_factor(self.world_to_camera(pose.position())[0]);
        let direction = self.world_to_camera(world_dir);
        let scaled_radius = radius * self.scale * factor;
        (-direction[1] * scaled_radius, -direction[2] * scaled_radius)
    }

    /// Convert a world-space sphere radius to pixels at a pose's depth.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub fn project_radius(&self, pose: Pose, radius: f32) -> f32 {
        radius * self.scale * self.depth_factor(self.world_to_camera(pose.position())[0])
    }

    /// Convert a world-space stroke width to pixels.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    pub fn project_width(&self, width: f32) -> f32 {
        (width * self.scale).max(1.0)
    }
}
