use crate::{Linkage, Params, Pose, Step, Vec3};
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
    format, fs,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    println,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn assert_vec3_approx_eq(actual: Vec3, expected: Vec3) {
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

pub(super) fn assert_approx_eq(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-6,
        "expected {expected:.6}, got {actual:.6}"
    );
}

// todo0000000 stream line pose creation and comparison. (done with Pose::is_close_to)
pub(super) fn assert_pose_approx_eq(actual: Pose, expected: Pose) {
    assert!(
        actual.is_close_to(&expected, 1e-3),
        "expected {:?}, got {:?}",
        expected,
        actual
    );
}

pub(super) fn position_after_move<const N: usize>(
    linkage: &Linkage<Params, N>,
    params: &Params,
    move_index: usize,
) -> Result<Vec3, Box<dyn Error>> {
    linkage
        .steps()
        .iter()
        .zip(linkage.poses(params))
        .filter_map(|(step, pose)| {
            if matches!(step, Step::Start | Step::Move(_)) {
                Some(pose.position)
            } else {
                None
            }
        })
        .nth(move_index)
        .ok_or_else(|| format!("missing move position at index {move_index}").into())
}

const CANVAS_WIDTH: usize = 300;
const CANVAS_HEIGHT: usize = 300;
const CANVAS_PIXELS: usize = CANVAS_WIDTH * CANVAS_HEIGHT;
const WORLD_MIN: f32 = -10.0;
const WORLD_MAX: f32 = 10.0;
const EXCEL_BLUE: Rgb888 = Rgb888::new(21, 96, 130);

pub(super) struct Canvas {
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

pub(super) fn draw_linkage_xy_canvas<const N: usize>(
    linkage: &Linkage<Params, N>,
    params: &Params,
) -> Canvas {
    let mut canvas = Canvas::new();
    let mut previous: Option<Pose> = None;

    for pose in linkage.poses(params) {
        if let Some(previous_pose) = previous {
            draw_segment(&mut canvas, previous_pose.position, pose.position);
        }
        draw_point(&mut canvas, pose.position);
        previous = Some(pose);
    }

    canvas
}

fn draw_segment(canvas: &mut Canvas, from: Vec3, to: Vec3) {
    let draw_result = Line::new(world_to_point(from), world_to_point(to))
        .into_styled(PrimitiveStyle::with_stroke(EXCEL_BLUE, 2))
        .draw(canvas);
    match draw_result {
        Ok(()) => {}
        Err(infallible) => match infallible {},
    }
}

fn draw_point(canvas: &mut Canvas, position: Vec3) {
    let center = world_to_point(position);
    let top_left = Point::new(center.x - 2, center.y - 2);
    let draw_result = Circle::new(top_left, 4)
        .into_styled(PrimitiveStyle::with_fill(EXCEL_BLUE))
        .draw(canvas);
    match draw_result {
        Ok(()) => {}
        Err(infallible) => match infallible {},
    }
}

fn world_to_point(position: Vec3) -> Point {
    let x = world_to_pixel(position[0]);
    let y = (CANVAS_HEIGHT - 1) as i32 - world_to_pixel(position[1]);
    Point::new(x, y)
}

fn world_to_pixel(value: f32) -> i32 {
    let normalized = (value - WORLD_MIN) / (WORLD_MAX - WORLD_MIN);
    (normalized * ((CANVAS_WIDTH - 1) as f32)).round() as i32
}

pub(super) fn assert_png_matches_expected(
    filename: &str,
    canvas: &Canvas,
) -> Result<(), Box<dyn Error>> {
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
        expected_bytes, actual_bytes,
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
    let unix_time = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => error.duration().as_nanos(),
    };
    let process_id = std::process::id();
    let mut path = std::env::temp_dir();
    path.push(format!("{filename}-{process_id}-{unix_time}"));
    path
}
