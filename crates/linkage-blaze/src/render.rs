//! Evaluated three-dimensional drawing geometry and projection helpers.
//!
//! [`LinkageView::draw_items_3d`](crate::LinkageView::draw_items_3d) yields
//! [`Item3d`] values. [`Item3d::project`] with a [`Projection`] turns each one
//! into a pixel-space [`Item2d`], which draws onto any embedded-graphics
//! [`DrawTarget`]. To render some other way, read the 3D data directly from
//! [`Stroke`], [`Disk`], and [`Sphere`].
//!
//! # Example
//!
//! This draws a small robot arm:
//!
#![cfg_attr(
    feature = "doc-images",
    doc = ::embed_doc_image::embed_image!("render_example", "tests/assets/render_example.png")
)]
#![cfg_attr(
    feature = "doc-images",
    doc = "![A small robot arm: gray turntable, blue upper arm, orange forearm, and a crimson two-finger gripper.][render_example]"
)]
//!
//! ```rust
//! # use embedded_graphics::{pixelcolor::{Rgb888, WebColors}, prelude::*};
//! # use linkage_blaze::{LinkageFixed, LinkageView, render::Projection};
//! # struct Display(Vec<Rgb888>);
//! # impl OriginDimensions for Display {
//! #     fn size(&self) -> Size { Size::new(300, 300) }
//! # }
//! # impl DrawTarget for Display {
//! #     type Color = Rgb888;
//! #     type Error = core::convert::Infallible;
//! #     fn draw_iter<I: IntoIterator<Item = Pixel<Rgb888>>>(&mut self, pixels: I) -> Result<(), Self::Error> {
//! #         for Pixel(point, color) in pixels {
//! #             if (0..300).contains(&point.x) && (0..300).contains(&point.y) {
//! #                 self.0[point.y as usize * 300 + point.x as usize] = color;
//! #             }
//! #         }
//! #         Ok(())
//! #     }
//! # }
//! # fn main() -> Result<(), Box<dyn core::error::Error>> {
//! # let mut display = Display(vec![Rgb888::WHITE; 300 * 300]);
//! const ARM: LinkageView<0, 1> = LinkageFixed::<0, 1, 21>::start()
//!     .pitch(-20.0) // tilt the scene toward the camera
//!     .pen_color(Rgb888::CSS_DIM_GRAY)
//!     .disk(2.5) // turntable
//!     .pitch(-75.0)
//!     .pen_width(0.4)
//!     .pen_color(Rgb888::CSS_STEEL_BLUE)
//!     .forward(3.5) // upper arm
//!     .sphere(0.5) // elbow
//!     .yaw(-70.0)
//!     .pen_color(Rgb888::CSS_DARK_ORANGE)
//!     .forward(3.0) // forearm
//!     .sphere(0.45) // wrist
//!     .pen_color(Rgb888::CSS_CRIMSON)
//!     .pen_width(0.25)
//!     .mark("wrist")
//!     .yaw(35.0)
//!     .forward(1.6) // finger
//!     .restore("wrist")
//!     .yaw(-35.0)
//!     .forward(1.6) // finger
//!     .view();
//! let projection = Projection::front_perspective(Point::new(140, 250), 30.0, 20.0);
//! let no_params = []; // ARM has no degrees of freedom
//! for item_3d in ARM.draw_items_3d(&no_params)? {
//!     item_3d.project(&projection).draw(&mut display)?; // any embedded-graphics target
//! }
//! # // Keep the docs' picture honest: these pixels must match the embedded PNG.
//! # let golden = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/render_example.png");
//! # let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(golden)?)).read_info()?;
//! # let mut golden_bytes = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
//! # let frame = reader.next_frame(&mut golden_bytes)?;
//! # let pixel_bytes: Vec<u8> = display.0.iter().flat_map(|color| [color.r(), color.g(), color.b()]).collect();
//! # assert!(pixel_bytes == golden_bytes[..frame.buffer_size()], "pixels differ from {golden}");
//! # Ok(())
//! # }
//! ```

use super::{Pose, Rgb888, Vec3};
use embedded_graphics::{
    Pixel,
    prelude::{DrawTarget, Drawable, Point, Primitive},
    primitives::{Circle, Line, PrimitiveStyle},
};

/// A 3D pen-down line segment, from start to end, emitted while evaluating a
/// linkage.
///
/// To project it to 2D, see [`Item3d::project`].
///
/// # Example
///
/// ```rust
/// # use linkage_blaze::{LinkageFixed, LinkageView, Rgb888, Vec3, WebColors, render::Item3d};
/// # fn main() -> Result<(), linkage_blaze::Error> {
/// const ONE_LINE: LinkageView<0, 0> = LinkageFixed::<0, 0, 5>::start()
///     .pen_width(0.5)
///     .pen_color(Rgb888::CSS_STEEL_BLUE)
///     .forward(2.0)
///     .view();
/// let no_params = []; // ONE_LINE has no degrees of freedom
/// let item_3d = ONE_LINE.draw_items_3d(&no_params)?.next().unwrap(); // the line's one item
/// let Item3d::Stroke(stroke) = item_3d else { panic!("expected a stroke") };
/// assert_eq!(stroke.start().position(), Vec3::from([0.0, 0.0, 0.0]));
/// assert_eq!(stroke.end().position(), Vec3::from([2.0, 0.0, 0.0]));
/// assert_eq!(stroke.color(), Rgb888::CSS_STEEL_BLUE);
/// assert_eq!(stroke.width(), 0.5); // linkage units, before projection
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub(crate) start: Pose,
    pub(crate) end: Pose,
    pub(crate) color: Rgb888,
    pub(crate) width: f32,
}

impl Stroke {
    /// Return the pose at the start of the segment.
    /// See the [`Stroke` example](Stroke#example).
    #[must_use]
    pub const fn start(self) -> Pose {
        self.start
    }
    /// Return the pose at the end of the segment.
    /// See the [`Stroke` example](Stroke#example).
    #[must_use]
    pub const fn end(self) -> Pose {
        self.end
    }
    /// Return the segment color.
    /// See the [`Stroke` example](Stroke#example).
    #[must_use]
    pub const fn color(self) -> Rgb888 {
        self.color
    }
    /// Return the segment width in linkage units.
    /// See the [`Stroke` example](Stroke#example).
    #[must_use]
    pub const fn width(self) -> f32 {
        self.width
    }
}

/// A filled 3D disk emitted while evaluating a linkage.
///
/// The disk lies in the plane of its pose's forward and left axes.
///
/// To project it to 2D, see [`Item3d::project`].
///
/// # Example
///
/// ```rust
/// # use linkage_blaze::{LinkageFixed, LinkageView, Rgb888, Vec3, WebColors, render::Item3d};
/// # fn main() -> Result<(), linkage_blaze::Error> {
/// const ONE_DISK: LinkageView<0, 0> = LinkageFixed::<0, 0, 6>::start()
///     .pen_up() // move without drawing
///     .forward(1.0)
///     .pen_color(Rgb888::CSS_SEA_GREEN)
///     .disk(0.75)
///     .view();
/// let no_params = []; // ONE_DISK has no degrees of freedom
/// let item_3d = ONE_DISK.draw_items_3d(&no_params)?.next().unwrap(); // the disk
/// let Item3d::Disk(disk) = item_3d else { panic!("expected a disk") };
/// assert_eq!(disk.pose().position(), Vec3::from([1.0, 0.0, 0.0]));
/// assert_eq!(disk.radius(), 0.75); // linkage units, before projection
/// assert_eq!(disk.color(), Rgb888::CSS_SEA_GREEN);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Disk {
    pub(crate) pose: Pose,
    pub(crate) radius: f32,
    pub(crate) color: Rgb888,
}

impl Disk {
    /// Return the disk center pose.
    /// See the [`Disk` example](Disk#example).
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    /// Return the disk radius in linkage units.
    /// See the [`Disk` example](Disk#example).
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    /// Return the disk color.
    /// See the [`Disk` example](Disk#example).
    pub const fn color(self) -> Rgb888 {
        self.color
    }
}

/// A 3D sphere emitted while evaluating a linkage.
///
/// To project it to 2D, see [`Item3d::project`].
///
/// # Example
///
/// ```rust
/// # use linkage_blaze::{LinkageFixed, LinkageView, Rgb888, Vec3, WebColors, render::Item3d};
/// # fn main() -> Result<(), linkage_blaze::Error> {
/// const ONE_SPHERE: LinkageView<0, 0> = LinkageFixed::<0, 0, 6>::start()
///     .pen_up() // move without drawing
///     .up(2.0)
///     .pen_color(Rgb888::CSS_GOLDENROD)
///     .sphere(0.5)
///     .view();
/// let no_params = []; // ONE_SPHERE has no degrees of freedom
/// let item_3d = ONE_SPHERE.draw_items_3d(&no_params)?.next().unwrap(); // the sphere
/// let Item3d::Sphere(sphere) = item_3d else { panic!("expected a sphere") };
/// assert_eq!(sphere.pose().position(), Vec3::from([0.0, 0.0, 2.0]));
/// assert_eq!(sphere.radius(), 0.5); // linkage units, before projection
/// assert_eq!(sphere.color(), Rgb888::CSS_GOLDENROD);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub(crate) pose: Pose,
    pub(crate) radius: f32,
    pub(crate) color: Rgb888,
}

impl Sphere {
    /// Return the sphere center pose.
    /// See the [`Sphere` example](Sphere#example).
    #[must_use]
    pub const fn pose(self) -> Pose {
        self.pose
    }
    #[must_use]
    /// Return the sphere radius in linkage units.
    /// See the [`Sphere` example](Sphere#example).
    pub const fn radius(self) -> f32 {
        self.radius
    }
    #[must_use]
    /// Return the sphere color.
    /// See the [`Sphere` example](Sphere#example).
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
    /// See the [module example](crate::render#example).
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
/// See the [module example](crate::render#example).
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
    /// See the [module example](crate::render#example).
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
///
/// # Example
///
/// ```rust
/// # use embedded_graphics::prelude::Point;
/// # use linkage_blaze::{Mat3, Pose, Vec3, render::Projection};
/// let near = Pose::new(Mat3::IDENTITY, Vec3::from([0.0, 2.0, 3.0]));
/// let far = Pose::new(Mat3::IDENTITY, Vec3::from([20.0, 2.0, 3.0]));
///
/// // Front view: world +Y is screen left, world +Z is screen up.
/// let front = Projection::front_orthographic(Point::new(100, 100), 10.0);
/// assert_eq!(near.project(&front), (80.0, 70.0));
/// assert_eq!(far.project(&front), (80.0, 70.0)); // depth doesn't matter
///
/// // Top view: world +Y is screen left, world +X is screen up.
/// let top = Projection::top_orthographic(Point::new(100, 100), 10.0);
/// assert_eq!(near.project(&top), (80.0, 100.0));
///
/// // Perspective: farther poses shrink toward the target origin.
/// let perspective = Projection::front_perspective(Point::new(100, 100), 10.0, 20.0);
/// assert_eq!(far.project(&perspective), (90.0, 85.0));
/// assert_eq!(perspective.project_radius(far, 1.0), 5.0);
///
/// // A world direction in pixels: 2 units of +Z is 20 pixels up the screen.
/// assert_eq!(front.project_dir(near, Vec3::from([0.0, 0.0, 1.0]), 2.0), (0.0, -20.0));
/// // Stroke widths scale but never drop below one pixel.
/// assert_eq!(front.project_width(0.5), 5.0);
/// assert_eq!(front.project_width(0.01), 1.0);
/// ```
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
    /// See the [`Projection` example](Projection#example).
    pub const fn front_orthographic(target_origin: Point, scale: f32) -> Self {
        Self {
            rotation: NEG_X_BASIS,
            target_origin,
            scale,
            focal: None,
        }
    }

    /// Create an orthographic top view looking along negative Z.
    /// See the [`Projection` example](Projection#example).
    pub const fn top_orthographic(target_origin: Point, scale: f32) -> Self {
        Self {
            rotation: NEG_Z_BASIS,
            target_origin,
            scale,
            focal: None,
        }
    }

    /// Create a perspective front view looking along negative X.
    /// See the [`Projection` example](Projection#example).
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
    /// See the [`Projection` example](Projection#example).
    pub fn project_dir(&self, pose: Pose, world_dir: Vec3, radius: f32) -> (f32, f32) {
        let factor = self.depth_factor(self.world_to_camera(pose.position())[0]);
        let direction = self.world_to_camera(world_dir);
        let scaled_radius = radius * self.scale * factor;
        (-direction[1] * scaled_radius, -direction[2] * scaled_radius)
    }

    /// Convert a world-space sphere radius to pixels at a pose's depth.
    /// See the [`Projection` example](Projection#example).
    pub fn project_radius(&self, pose: Pose, radius: f32) -> f32 {
        radius * self.scale * self.depth_factor(self.world_to_camera(pose.position())[0])
    }

    /// Convert a world-space stroke width to pixels.
    /// See the [`Projection` example](Projection#example).
    pub fn project_width(&self, width: f32) -> f32 {
        (width * self.scale).max(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Projection;
    use crate::{
        LinkageFixed, LinkageView, Rgb888,
        test_helpers::{Canvas, assert_png_matches_expected},
    };
    use embedded_graphics::{
        pixelcolor::WebColors,
        prelude::{Drawable, Point},
    };
    use std::{boxed::Box, error::Error as StdError};

    /// The robot arm from the module example. Keep the two in sync: the module
    /// doctest checks its own pixels against the PNG this test writes.
    const ARM: LinkageView<0, 1> = LinkageFixed::<0, 1, 21>::start()
        .pitch(-20.0) // tilt the scene toward the camera
        .pen_color(Rgb888::CSS_DIM_GRAY)
        .disk(2.5) // turntable
        .pitch(-75.0)
        .pen_width(0.4)
        .pen_color(Rgb888::CSS_STEEL_BLUE)
        .forward(3.5) // upper arm
        .sphere(0.5) // elbow
        .yaw(-70.0)
        .pen_color(Rgb888::CSS_DARK_ORANGE)
        .forward(3.0) // forearm
        .sphere(0.45) // wrist
        .pen_color(Rgb888::CSS_CRIMSON)
        .pen_width(0.25)
        .mark("wrist")
        .yaw(35.0)
        .forward(1.6) // finger
        .restore("wrist")
        .yaw(-35.0)
        .forward(1.6) // finger
        .view();

    #[test]
    fn module_example_matches_expected_png() -> Result<(), Box<dyn StdError>> {
        let mut canvas = Canvas::new();
        let projection = Projection::front_perspective(Point::new(140, 250), 30.0, 20.0);
        let no_params = []; // ARM has no degrees of freedom
        for item_3d in ARM.draw_items_3d(&no_params)? {
            let Ok(()) = item_3d.project(&projection).draw(&mut canvas);
        }
        assert_png_matches_expected("render_example.png", &canvas)
    }
}
