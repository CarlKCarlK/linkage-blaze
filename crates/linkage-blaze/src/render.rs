//! Evaluated three-dimensional drawing geometry and projection helpers.

use super::{Pose, Vec3};
use embedded_graphics::prelude::Point;

use super::Rgb888;

/// A drawable pen-down forward segment produced by a linkage.
#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub(crate) start: Pose,
    pub(crate) end: Pose,
    pub(crate) color: Rgb888,
    pub(crate) width: f32,
}

impl Stroke {
    /// Return the segment start pose.
    #[must_use]
    pub const fn start(self) -> Pose {
        self.start
    }
    /// Return the segment end pose.
    #[must_use]
    pub const fn end(self) -> Pose {
        self.end
    }
    /// Return the segment pen color.
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }
    /// Return the segment pen width.
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }
}

/// A disk emitted while evaluating a linkage.
#[derive(Clone, Copy, Debug)]
pub struct Disk {
    pub(crate) pose: Pose,
    pub(crate) radius: f32,
    pub(crate) color: Rgb888,
}

impl Disk {
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
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
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }
}

/// A 3D draw item produced by a linkage: a line stroke, a filled disk, or a sphere.
#[derive(Clone, Copy, Debug)]
pub enum Item3d {
    Stroke(Stroke),
    Disk(Disk),
    Sphere(Sphere),
}

impl Item3d {
    /// Project this 3D draw item through `projection` into pixel-space.
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

/// Maps world-space geometry to pixel space for a particular view.
///
/// The rotation maps world axes onto camera axes: row 0 is depth, row 1 is the
/// source of screen X, and row 2 is the source of screen Y. Named constructors
/// provide orthographic front/top views and a perspective front view.
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
    /// Orthographic front view, looking along negative X.
    pub const fn front_orthographic(target_origin: Point, scale: f32) -> Self {
        Self {
            rotation: NEG_X_BASIS,
            target_origin,
            scale,
            focal: None,
        }
    }

    /// Orthographic top view, looking along negative Z.
    pub const fn top_orthographic(target_origin: Point, scale: f32) -> Self {
        Self {
            rotation: NEG_Z_BASIS,
            target_origin,
            scale,
            focal: None,
        }
    }

    /// Perspective front view, looking along negative X.
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

    /// Project a world-space direction vector scaled by `radius`.
    pub fn project_dir(&self, pose: Pose, world_dir: Vec3, radius: f32) -> (f32, f32) {
        let factor = self.depth_factor(self.world_to_camera(pose.position())[0]);
        let direction = self.world_to_camera(world_dir);
        let scaled_radius = radius * self.scale * factor;
        (-direction[1] * scaled_radius, -direction[2] * scaled_radius)
    }

    /// Scale a world-space sphere radius to pixel-space.
    pub fn project_radius(&self, pose: Pose, radius: f32) -> f32 {
        radius * self.scale * self.depth_factor(self.world_to_camera(pose.position())[0])
    }

    /// Scale a world-space stroke width to pixel-space.
    pub fn project_width(&self, width: f32) -> f32 {
        (width * self.scale).max(1.0)
    }
}
