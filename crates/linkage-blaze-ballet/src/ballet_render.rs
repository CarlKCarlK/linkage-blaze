use linkage_blaze_core::{LinkageFixed, NegXProjection, Rgb888, WebColors, linkage, linkage_fixed};

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
pub const BALLET_PROJECTION: NegXProjection = NegXProjection {
    center_x: BALLET_CENTER_X as f32,
    baseline_y: BALLET_BASELINE_Y as f32,
    scale: BALLET_SCALE,
};
