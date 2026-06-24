use embedded_graphics::{
    Drawable,
    primitives::{Circle, Line, Primitive, PrimitiveStyle},
};
use linkage_blaze_core::{
    DrawSurface, LinkageFixed, PixelTarget, PixelTargetAdapter, Pose, Projection, Rgb888, Vec3,
    WebColors, fill_ellipse_pixels, linkage, linkage_fixed, pixel_put, to_point,
};

// todo000 this should be hard coded in the reader and then read a as const after that. It should not be here.
const BALLET_DOF: usize = 132;

// todo00 audit the existing numeric color backlog and add approximate color-name comments.
// todo000 every numeric color should have a comment telling what it is. (and named colors are better)
pub const BACKGROUND: Rgb888 = Rgb888::new(10, 28, 36); // very dark blue-green
const FIGURE_COLOR: Rgb888 = Rgb888::CSS_ANTIQUE_WHITE;
pub const TEXT: Rgb888 = Rgb888::CSS_LIGHT_STEEL_BLUE;

// todo000 these could be OK, but there are a lot of them. Can't some be done via math?
pub const STATUS_BAND_HEIGHT: i32 = 20;
pub const BALLET_CENTER_X: i32 = 84;
pub const BALLET_BASELINE_Y: i32 = 300;
pub const BALLET_SCALE: f32 = 1.575;

// todo0000 interesting.
pub const BALLET: LinkageFixed<BALLET_DOF, 6, 540> = {
    const INNER: LinkageFixed<BALLET_DOF, 6, 538> =
        linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");
    LinkageFixed::<0, 0, 3>::start()
        .pen_color(FIGURE_COLOR)
        .pen_width(3.2)
        .combine(INNER)
};

/// Orthographic projection for the ballet renderer.
/// View: looking along -X; screen_x ← -world_Y, screen_y ← -world_Z.
pub struct BalletProjection;

impl Projection for BalletProjection {
    fn project_pos(&self, pose: Pose) -> (f32, f32) {
        let p = pose.position();
        (
            BALLET_CENTER_X as f32 - p[1] * BALLET_SCALE,
            BALLET_BASELINE_Y as f32 - p[2] * BALLET_SCALE,
        )
    }

    fn project_dir(&self, _pose: Pose, world_dir: Vec3, radius: f32) -> (f32, f32) {
        let r = radius * BALLET_SCALE;
        (-world_dir[1] * r, -world_dir[2] * r)
    }

    fn project_radius(&self, _pose: Pose, radius: f32) -> f32 {
        radius * BALLET_SCALE
    }

    fn project_width(&self, width: f32) -> f32 {
        width * BALLET_SCALE
    }
}

// todo0000 move out of here.
/// Wraps a [`PixelTarget`] as a [`DrawSurface`] using ballet-style drawing.
pub struct BalletSurface<'a, T: PixelTarget>(pub &'a mut T);

impl<T: PixelTarget> DrawSurface for BalletSurface<'_, T> {
    fn stroke(&mut self, start: (f32, f32), end: (f32, f32), color: Rgb888, pixel_width: f32) {
        let width = (pixel_width + 0.5) as u32;
        Line::new(to_point(start), to_point(end))
            .into_styled(PrimitiveStyle::with_stroke(color, width.max(1)))
            .draw(&mut PixelTargetAdapter(self.0))
            .unwrap();
    }

    fn filled_ellipse(
        &mut self,
        center: (f32, f32),
        axis_a: (f32, f32),
        axis_b: (f32, f32),
        color: Rgb888,
    ) {
        let target = &mut *self.0;
        fill_ellipse_pixels(center, axis_a, axis_b, |x, y| pixel_put(target, x, y, color));
    }

    fn filled_circle(&mut self, center: (f32, f32), pixel_radius: f32, color: Rgb888) {
        let diameter = ((pixel_radius * 2.0) + 0.5) as u32;
        Circle::with_center(to_point(center), diameter.max(1))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut PixelTargetAdapter(self.0))
            .unwrap();
    }
}
