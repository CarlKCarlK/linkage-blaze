//! Evaluated three-dimensional drawing geometry and projection helpers.
//!
//! [`Item3d`] values are produced by [`crate::LinkageView::draw_items_3d`].
//! Use [`Item3d::project`] with a [`Projection`] when adapting them to a
//! Device Envoy display, or inspect the payloads directly in another renderer.
//!
//! # Rendering evaluated items
//!
//! ```rust
//! # use embedded_graphics::prelude::Point;
//! # use linkage_blaze::{LinkageFixed, Vec3};
//! # use linkage_blaze::render::Item3d;
//! # use linkage_blaze::render::Projection;
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
//!         let _display_item = Item3d::Stroke(stroke).project(&projection);
//!         let _ = (stroke.start(), stroke.end(), stroke.color(), stroke.width());
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

use super::{Pose, Vec3};
use embedded_graphics::prelude::Point;

use super::Rgb888;

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
    /// Project this item into a Device Envoy 2D display draw item.
    ///
    /// The projection controls the camera orientation, scale, target pixel,
    /// and optional perspective depth scaling.
    /// See the [rendering evaluated items example](#rendering-evaluated-items).
    #[must_use]
    pub fn project(self, projection: &Projection) -> device_envoy_core::cyd::display::DrawItem {
        match self {
            Self::Stroke(stroke) => device_envoy_core::cyd::display::DrawItem::Stroke {
                start: stroke.start().project(projection),
                end: stroke.end().project(projection),
                color: stroke.color(),
                pixel_width: projection.project_width(stroke.width()),
            },
            Self::Disk(disk) => {
                let orientation = disk.pose().orientation();
                device_envoy_core::cyd::display::DrawItem::Ellipse {
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
            Self::Sphere(sphere) => device_envoy_core::cyd::display::DrawItem::Circle {
                center: sphere.pose().project(projection),
                pixel_radius: projection.project_radius(sphere.pose(), sphere.radius()),
                color: sphere.color(),
            },
        }
    }
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
