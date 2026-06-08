#![no_std]
#![forbid(unsafe_code)]
#![doc = "No-allocation robot arm simulation and math primitives."]

//todo00 allow splits/DAGs in the models.

#[cfg(test)]
extern crate std;


/// A step in the robot arm linkage description.
///
/// - v0 = tail → nose (forward), v1 = right → left, v2 = belly → back
#[derive(Debug)]
pub enum Step<P> {
    /// Reset to the origin with the identity orientation.
    Start,
    /// Rotate around v2 (belly → back): turn left/right.
    Yaw(Arg<P>),
    /// Rotate around v1 (right → left): nose up/down.
    Pitch(Arg<P>),
    /// Rotate around v0 (tail → nose): right side down.
    Roll(Arg<P>),
    /// Advance along v0 by the given distance.
    Move(Arg<P>),
}

/// Runtime model parameters.
///
/// Angle fields are stored in radians. Distance fields are stored in linkage units.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub lower_hand: f32,
    pub bend_elbow: f32,
    pub close_hand: f32,
    pub lower_arm: f32,
    pub spin_wrist: f32,
    pub spin_hand: f32,
}

impl Params {
    /// Create model parameters from user-facing degree values and distances.
    pub const fn from_degrees(
        lower_hand: f32,
        bend_elbow: f32,
        close_hand: f32,
        lower_arm: f32,
        spin_wrist: f32,
        spin_hand: f32,
    ) -> Self {
        Self {
            lower_hand: degrees_to_radians(lower_hand),
            bend_elbow: degrees_to_radians(bend_elbow),
            close_hand,
            lower_arm: degrees_to_radians(lower_arm),
            spin_wrist: degrees_to_radians(spin_wrist),
            spin_hand: degrees_to_radians(spin_hand),
        }
    }
}

/// A fixed argument or a runtime parameter accessor.
#[derive(Debug)]
pub enum Arg<P> {
    Fixed(f32),
    Param(fn(&P) -> f32),
}

impl<P> Arg<P> {
    fn resolve(&self, params: &P) -> f32 {
        match self {
            Self::Fixed(value) => *value,
            Self::Param(accessor) => accessor(params),
        }
    }
}

/// A fixed-size linkage description.
pub struct Linkage<P, const N: usize> {
    steps: [Step<P>; N],
    len: usize,
}

impl<P, const N: usize> Linkage<P, N> {
    /// Start a fixed-size linkage with an implicit origin row.
    pub const fn start() -> Self {
        assert!(N > 0, "linkage must have room for the implicit start step");
        Self {
            steps: [const { Step::Start }; N],
            len: 1,
        }
    }

    /// Return the linkage steps in evaluation order.
    #[must_use]
    pub const fn steps(&self) -> &[Step<P>; N] {
        &self.steps
    }

    /// Return the number of linkage steps, including the implicit start step.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Return true when the linkage has no steps.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Add a yaw step from a user-facing angle in degrees.
    pub const fn yaw(self, degrees: f32) -> Self {
        self.push(Step::Yaw(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a yaw step from a runtime parameter.
    pub const fn yaw_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Yaw(Arg::Param(accessor)))
    }

    /// Add a pitch step from a user-facing angle in degrees.
    pub const fn pitch(self, degrees: f32) -> Self {
        self.push(Step::Pitch(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a pitch step from a runtime parameter.
    pub const fn pitch_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Pitch(Arg::Param(accessor)))
    }

    /// Add a roll step from a user-facing angle in degrees.
    pub const fn roll(self, degrees: f32) -> Self {
        self.push(Step::Roll(Arg::Fixed(degrees_to_radians(degrees))))
    }

    /// Add a roll step from a runtime parameter.
    pub const fn roll_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Roll(Arg::Param(accessor)))
    }

    /// Add a fixed forward move step.
    pub const fn forward(self, distance: f32) -> Self {
        self.push(Step::Move(Arg::Fixed(distance)))
    }

    /// Add a move step from a runtime parameter.
    pub const fn move_param(self, accessor: fn(&P) -> f32) -> Self {
        self.push(Step::Move(Arg::Param(accessor)))
    }

    const fn push(mut self, step: Step<P>) -> Self {
        assert!(self.len < N, "linkage has more steps than N");
        self.steps[self.len] = step;
        self.len += 1;
        self
    }

    /// Create a simulation iterator for this linkage.
    pub fn simulate<'a>(&'a self, params: &'a P) -> Simulate<'a, P, N> {
        Simulate::new(self, params)
    }
}

/// 3D position [x, y, z].
pub type Vec3 = [f32; 3];

// 3×3 rotation matrix, row-major: mat[row][col].
// Columns are body-frame axes: col 0 = v0 (forward), col 1 = v1 (left), col 2 = v2 (up/back).
type Mat3 = [[f32; 3]; 3];

const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

const fn degrees_to_radians(degrees: f32) -> f32 {
    degrees * (core::f32::consts::PI / 180.0)
}

fn mat_mul(a: Mat3, b: Mat3) -> Mat3 {
    let mut out = [[0.0f32; 3]; 3];
    for row in 0..3 {
        for col in 0..3 {
            for k in 0..3 {
                out[row][col] += a[row][k] * b[k][col];
            }
        }
    }
    out
}

// Rotation matrices match the Excel SWITCH formulas exactly.
// Yaw  = Rz: [[c,-s,0],[s,c,0],[0,0,1]]
// Pitch = Ry (Excel convention): [[c,0,-s],[0,1,0],[s,0,c]]
// Roll  = Rx: [[1,0,0],[0,c,-s],[0,s,c]]
fn rotation_matrix<P>(step: &Step<P>, params: &P) -> Mat3 {
    let radians = match step {
        Step::Yaw(arg) | Step::Pitch(arg) | Step::Roll(arg) => arg.resolve(params),
        Step::Start | Step::Move(_) => return IDENTITY,
    };
    let cos = libm::cosf(radians);
    let sin = libm::sinf(radians);
    match step {
        Step::Yaw(_) => [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]],
        Step::Pitch(_) => [[cos, 0.0, -sin], [0.0, 1.0, 0.0], [sin, 0.0, cos]],
        Step::Roll(_) => [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]],
        Step::Start | Step::Move(_) => IDENTITY,
    }
}

struct Turtle {
    orientation: Mat3,
    position: Vec3,
}

impl Turtle {
    fn new() -> Self {
        Self {
            orientation: IDENTITY,
            position: [0.0, 0.0, 0.0],
        }
    }

    fn apply<P>(&mut self, step: &Step<P>, params: &P) {
        match step {
            Step::Start => {
                *self = Self::new();
            }
            Step::Move(arg) => {
                let dist = arg.resolve(params);
                // advance along v0 = col 0 of orientation
                self.position[0] += dist * self.orientation[0][0];
                self.position[1] += dist * self.orientation[1][0];
                self.position[2] += dist * self.orientation[2][0];
            }
            _ => {
                self.orientation = mat_mul(self.orientation, rotation_matrix(step, params));
            }
        }
    }
}

/// Iterator over joint positions produced by simulating a linkage.
///
/// Yields the [`Step::Start`] position `[0,0,0]` and then the position after each [`Step::Move`].
pub struct Simulate<'a, P, const N: usize> {
    linkage: &'a Linkage<P, N>,
    params: &'a P,
    index: usize,
    turtle: Turtle,
}

impl<'a, P, const N: usize> Simulate<'a, P, N> {
    /// Create a new simulation iterator for the given linkage.
    pub fn new(linkage: &'a Linkage<P, N>, params: &'a P) -> Self {
        Self {
            linkage,
            params,
            index: 0,
            turtle: Turtle::new(),
        }
    }
}

impl<P, const N: usize> Iterator for Simulate<'_, P, N> {
    type Item = Vec3;

    fn next(&mut self) -> Option<Vec3> {
        loop {
            if self.index >= self.linkage.len {
                return None;
            }
            let step = &self.linkage.steps[self.index];
            self.index += 1;
            self.turtle.apply(step, self.params);
            if matches!(step, Step::Start | Step::Move(_)) {
                return Some(self.turtle.position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Linkage, Params, Step};
    use core::convert::Infallible;
    use embedded_graphics::{
        draw_target::DrawTarget,
        geometry::{OriginDimensions, Size},
        pixelcolor::Rgb888,
        prelude::*,
        primitives::{Circle, Line, PrimitiveStyle},
    };
    use png::{BitDepth, ColorType, Encoder};
    use std::{
        boxed::Box,
        error::Error,
        format,
        fs,
        fs::File,
        io::BufWriter,
        path::{Path, PathBuf},
        println,
        time::{SystemTime, UNIX_EPOCH},
    };

    const EXCEL_PARAMS: Params = Params::from_degrees(
        -45.15793644,
        -45.26102633,
        0.249940215,
        -0.036069163,
        90.0,
        180.0,
    );

    fn lower_hand(params: &Params) -> f32 {
        params.lower_hand
    }

    fn bend_elbow(params: &Params) -> f32 {
        params.bend_elbow
    }

    fn close_hand(params: &Params) -> f32 {
        params.close_hand
    }

    fn lower_arm(params: &Params) -> f32 {
        params.lower_arm
    }

    fn spin_wrist(params: &Params) -> f32 {
        params.spin_wrist
    }

    fn spin_hand(params: &Params) -> f32 {
        params.spin_hand
    }

    const LINKAGE: Linkage<Params, 24> = Linkage::start()
        .yaw(90.0)
        .yaw_param(lower_hand)
        .pitch(90.0)
        .forward(2.5)
        .pitch(-90.0)
        .pitch(0.0)
        .forward(3.0)
        .yaw_param(lower_arm)
        .forward(3.0)
        .pitch_param(bend_elbow)
        .forward(1.0)
        .roll(180.0)
        .forward(0.5)
        .yaw_param(spin_wrist)
        .move_param(close_hand)
        .yaw(-90.0)
        .forward(1.0)
        .yaw_param(spin_hand)
        .forward(1.0)
        .yaw(90.0)
        .forward(0.5)
        .yaw(90.0)
        .forward(1.0);

    #[test]
    fn test_linkage_structure() {
        assert_eq!(LINKAGE.len(), 24);
        assert!(matches!(LINKAGE.steps()[0], Step::Start));
        assert!(matches!(LINKAGE.steps()[12], Step::Roll(_)));
        assert!(matches!(LINKAGE.steps()[23], Step::Move(_)));
    }

    #[test]
    fn test_simulate_yields_initial_position() {
        let first = LINKAGE.simulate(&EXCEL_PARAMS).next().unwrap();
        assert_vec3_approx_eq(first, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_simulate_position_count() {
        // 10 Move steps → 11 positions (initial + one per Move)
        assert_eq!(LINKAGE.simulate(&EXCEL_PARAMS).count(), 11);
    }

    #[test]
    fn test_simulate_first_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(1), [0.0, 0.0, 2.5]);
    }

    #[test]
    fn test_simulate_second_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(2), [2.12716, 2.115, 2.5]);
    }

    #[test]
    fn test_simulate_third_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(3), [4.25565, 4.23, 2.5]);
    }

    #[test]
    fn test_simulate_fourth_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(4), [4.75565, 4.726, 1.79]);
    }

    #[test]
    fn test_simulate_fifth_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(5), [5.00475, 4.974, 1.435]);
    }

    #[test]
    fn test_simulate_last_move_matches_excel() {
        assert_vec3_approx_eq(position_after_move(10), [5.32801, 5.647, 0.724]);
    }

    #[test]
    fn test_linkage_png_matches_expected() -> Result<(), Box<dyn Error>> {
        let canvas = draw_linkage_xy_canvas();
        assert_png_matches_expected("linkage_xy.png", &canvas)
    }

    fn assert_vec3_approx_eq(actual: [f32; 3], expected: [f32; 3]) {
        let close_enough = actual
            .iter()
            .zip(expected.iter())
            .all(|(x, y)| (x - y).abs() < 1e-3);
        assert!(
            close_enough,
            "expected ({:.5},{:.5},{:.5}), got ({:.5},{:.5},{:.5})",
            expected[0], expected[1], expected[2], actual[0], actual[1], actual[2]
        );
    }

    fn position_after_move(move_index: usize) -> [f32; 3] {
        LINKAGE.simulate(&EXCEL_PARAMS).nth(move_index).unwrap()
    }

    const CANVAS_WIDTH: usize = 300;
    const CANVAS_HEIGHT: usize = 300;
    const CANVAS_PIXELS: usize = CANVAS_WIDTH * CANVAS_HEIGHT;
    const WORLD_MIN: f32 = -10.0;
    const WORLD_MAX: f32 = 10.0;
    const EXCEL_BLUE: Rgb888 = Rgb888::new(21, 96, 130);

    struct Canvas {
        pixels: [Rgb888; CANVAS_PIXELS],
    }

    impl Canvas {
        fn new() -> Self {
            Self {
                pixels: [Rgb888::WHITE; CANVAS_PIXELS],
            }
        }

        fn rgb_bytes(&self) -> [u8; CANVAS_PIXELS * 3] {
            let mut bytes = [0u8; CANVAS_PIXELS * 3];
            let mut pixel_index = 0;
            while pixel_index < CANVAS_PIXELS {
                let pixel = self.pixels[pixel_index];
                let byte_index = pixel_index * 3;
                bytes[byte_index] = pixel.r();
                bytes[byte_index + 1] = pixel.g();
                bytes[byte_index + 2] = pixel.b();
                pixel_index += 1;
            }
            bytes
        }
    }

    impl DrawTarget for Canvas {
        type Color = Rgb888;
        type Error = Infallible;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            for Pixel(point, color) in pixels {
                if point.x < 0 || point.y < 0 {
                    continue;
                }
                let x_index = point.x as usize;
                let y_index = point.y as usize;
                if x_index >= CANVAS_WIDTH || y_index >= CANVAS_HEIGHT {
                    continue;
                }
                self.pixels[y_index * CANVAS_WIDTH + x_index] = color;
            }
            Ok(())
        }
    }

    impl OriginDimensions for Canvas {
        fn size(&self) -> Size {
            Size::new(CANVAS_WIDTH as u32, CANVAS_HEIGHT as u32)
        }
    }

    fn draw_linkage_xy_canvas() -> Canvas {
        let mut canvas = Canvas::new();
        let mut positions = LINKAGE.simulate(&EXCEL_PARAMS);
        let mut previous = positions.next().expect("linkage must include start");

        draw_point(&mut canvas, previous);
        for position in positions {
            draw_segment(&mut canvas, previous, position);
            draw_point(&mut canvas, position);
            previous = position;
        }

        canvas
    }

    fn draw_segment(canvas: &mut Canvas, from: [f32; 3], to: [f32; 3]) {
        Line::new(world_to_point(from), world_to_point(to))
            .into_styled(PrimitiveStyle::with_stroke(EXCEL_BLUE, 2))
            .draw(canvas)
            .expect("line draw must succeed");
    }

    fn draw_point(canvas: &mut Canvas, position: [f32; 3]) {
        let center = world_to_point(position);
        let top_left = Point::new(center.x - 2, center.y - 2);
        Circle::new(top_left, 4)
            .into_styled(PrimitiveStyle::with_fill(EXCEL_BLUE))
            .draw(canvas)
            .expect("point draw must succeed");
    }

    fn world_to_point(position: [f32; 3]) -> Point {
        let x = world_to_pixel(position[0]);
        let y = (CANVAS_HEIGHT - 1) as i32 - world_to_pixel(position[1]);
        Point::new(x, y)
    }

    fn world_to_pixel(value: f32) -> i32 {
        let normalized = (value - WORLD_MIN) / (WORLD_MAX - WORLD_MIN);
        (normalized * ((CANVAS_WIDTH - 1) as f32)).round() as i32
    }

    fn assert_png_matches_expected(filename: &str, canvas: &Canvas) -> Result<(), Box<dyn Error>> {
        let expected_path = expected_png_path(filename);
        if std::env::var_os("ROBOT_ARM_UPDATE_PNGS").is_some() {
            write_png(&expected_path, canvas)?;
            println!("updated PNG at {}", expected_path.display());
            return Ok(());
        }

        if !expected_path.exists() {
            return Err(format!(
                "expected PNG is missing at {}; rerun with ROBOT_ARM_UPDATE_PNGS=1 to create it",
                expected_path.display()
            )
            .into());
        }

        let output_path = temp_output_path(filename);
        write_png(&output_path, canvas)?;

        let expected_bytes = fs::read(&expected_path)?;
        let actual_bytes = fs::read(&output_path)?;
        let _ = fs::remove_file(&output_path);
        assert_eq!(
            expected_bytes,
            actual_bytes,
            "PNG bytes differ; rerun with ROBOT_ARM_UPDATE_PNGS=1 to accept the new image"
        );
        Ok(())
    }

    fn write_png(path: &Path, canvas: &Canvas) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let mut encoder = Encoder::new(writer, CANVAS_WIDTH as u32, CANVAS_HEIGHT as u32);
        encoder.set_color(ColorType::Rgb);
        encoder.set_depth(BitDepth::Eight);
        let mut png_writer = encoder.write_header()?;
        png_writer.write_image_data(&canvas.rgb_bytes())?;
        Ok(())
    }

    fn expected_png_path(filename: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("assets");
        path.push(filename);
        path
    }

    fn temp_output_path(filename: &str) -> PathBuf {
        let unix_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be valid")
            .as_nanos();
        let process_id = std::process::id();
        let mut path = std::env::temp_dir();
        path.push(format!("{filename}-{process_id}-{unix_time}"));
        path
    }
}
