use embedded_graphics::prelude::Point;
use serde::{Deserialize, Serialize};

use linkage_blaze_core::cyd::{SCREEN_HEIGHT, SCREEN_WIDTH};

#[derive(Clone, Copy, Debug)]
pub struct RawPoint {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CalibrationConfig {
    ax: f32,
    bx: f32,
    cx: f32,
    ay: f32,
    by: f32,
    cy: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum TouchInputEvent {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up,
}

#[derive(Clone, Copy)]
enum CalibrationCorner {
    UpperLeft,
    UpperRight,
    LowerRight,
    LowerLeft,
}

const CALIBRATION_CROSS_MARGIN: i32 = 28;

impl CalibrationConfig {
    #[must_use]
    pub fn new(ax: f32, bx: f32, cx: f32, ay: f32, by: f32, cy: f32) -> Self {
        Self {
            ax,
            bx,
            cx,
            ay,
            by,
            cy,
        }
    }

    #[must_use]
    pub fn from_four_points(points: [RawPoint; 4]) -> Self {
        compute_calibration_four_point(points, SCREEN_WIDTH as u16, SCREEN_HEIGHT as u16)
    }
}

#[must_use]
pub fn map_raw_to_screen(
    raw_x: u16,
    raw_y: u16,
    calibration_config: CalibrationConfig,
) -> (f32, f32) {
    let raw_x = raw_x as f32;
    let raw_y = raw_y as f32;

    let mapped_x =
        calibration_config.ax * raw_x + calibration_config.bx * raw_y + calibration_config.cx;
    let mapped_y =
        calibration_config.ay * raw_x + calibration_config.by * raw_y + calibration_config.cy;

    let mapped_x = mapped_x.clamp(0.0, (SCREEN_WIDTH as f32 - 1.0).max(0.0));
    let mapped_y = mapped_y.clamp(0.0, (SCREEN_HEIGHT as f32 - 1.0).max(0.0));

    (mapped_x, mapped_y)
}

fn solve_3x3(system_matrix: [[f32; 3]; 3], rhs_vector: [f32; 3]) -> (f32, f32, f32) {
    let determinant = system_matrix[0][0]
        * (system_matrix[1][1] * system_matrix[2][2] - system_matrix[1][2] * system_matrix[2][1])
        - system_matrix[0][1]
            * (system_matrix[1][0] * system_matrix[2][2]
                - system_matrix[1][2] * system_matrix[2][0])
        + system_matrix[0][2]
            * (system_matrix[1][0] * system_matrix[2][1]
                - system_matrix[1][1] * system_matrix[2][0]);

    assert!(
        determinant.abs() >= 0.000_001,
        "invalid touch calibration geometry"
    );

    let determinant_ax = rhs_vector[0]
        * (system_matrix[1][1] * system_matrix[2][2] - system_matrix[1][2] * system_matrix[2][1])
        - system_matrix[0][1]
            * (rhs_vector[1] * system_matrix[2][2] - system_matrix[1][2] * rhs_vector[2])
        + system_matrix[0][2]
            * (rhs_vector[1] * system_matrix[2][1] - system_matrix[1][1] * rhs_vector[2]);

    let determinant_bx = system_matrix[0][0]
        * (rhs_vector[1] * system_matrix[2][2] - system_matrix[1][2] * rhs_vector[2])
        - rhs_vector[0]
            * (system_matrix[1][0] * system_matrix[2][2]
                - system_matrix[1][2] * system_matrix[2][0])
        + system_matrix[0][2]
            * (system_matrix[1][0] * rhs_vector[2] - rhs_vector[1] * system_matrix[2][0]);

    let determinant_cx = system_matrix[0][0]
        * (system_matrix[1][1] * rhs_vector[2] - rhs_vector[1] * system_matrix[2][1])
        - system_matrix[0][1]
            * (system_matrix[1][0] * rhs_vector[2] - rhs_vector[1] * system_matrix[2][0])
        + rhs_vector[0]
            * (system_matrix[1][0] * system_matrix[2][1]
                - system_matrix[1][1] * system_matrix[2][0]);

    (
        determinant_ax / determinant,
        determinant_bx / determinant,
        determinant_cx / determinant,
    )
}

fn solve_affine_axis(
    points: [RawPoint; 4],
    screen: [Point; 4],
    map_x_axis: bool,
) -> (f32, f32, f32) {
    let mut sum_xx = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_x = 0.0;
    let mut sum_yy = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xo = 0.0;
    let mut sum_yo = 0.0;
    let mut sum_o = 0.0;

    for sample_index in 0..4 {
        let raw_x = points[sample_index].x as f32;
        let raw_y = points[sample_index].y as f32;
        let output = if map_x_axis {
            screen[sample_index].x as f32
        } else {
            screen[sample_index].y as f32
        };

        sum_xx += raw_x * raw_x;
        sum_xy += raw_x * raw_y;
        sum_x += raw_x;
        sum_yy += raw_y * raw_y;
        sum_y += raw_y;
        sum_xo += raw_x * output;
        sum_yo += raw_y * output;
        sum_o += output;
    }

    let system_matrix = [
        [sum_xx, sum_xy, sum_x],
        [sum_xy, sum_yy, sum_y],
        [sum_x, sum_y, 4.0],
    ];
    let rhs_vector = [sum_xo, sum_yo, sum_o];
    solve_3x3(system_matrix, rhs_vector)
}

fn compute_calibration_four_point(
    points: [RawPoint; 4],
    width: u16,
    height: u16,
) -> CalibrationConfig {
    let ul = calibration_corner_center(CalibrationCorner::UpperLeft, width, height);
    let ur = calibration_corner_center(CalibrationCorner::UpperRight, width, height);
    let lr = calibration_corner_center(CalibrationCorner::LowerRight, width, height);
    let ll = calibration_corner_center(CalibrationCorner::LowerLeft, width, height);
    let screen_targets = [ul, ur, lr, ll];

    let (ax, bx, cx) = solve_affine_axis(points, screen_targets, true);
    let (ay, by, cy) = solve_affine_axis(points, screen_targets, false);

    CalibrationConfig::new(ax, bx, cx, ay, by, cy)
}

fn calibration_corner_center(
    calibration_corner: CalibrationCorner,
    width: u16,
    height: u16,
) -> Point {
    let width = width as i32;
    let height = height as i32;
    match calibration_corner {
        CalibrationCorner::UpperLeft => {
            Point::new(CALIBRATION_CROSS_MARGIN, CALIBRATION_CROSS_MARGIN)
        }
        CalibrationCorner::UpperRight => Point::new(
            width - 1 - CALIBRATION_CROSS_MARGIN,
            CALIBRATION_CROSS_MARGIN,
        ),
        CalibrationCorner::LowerRight => Point::new(
            width - 1 - CALIBRATION_CROSS_MARGIN,
            height - 1 - CALIBRATION_CROSS_MARGIN,
        ),
        CalibrationCorner::LowerLeft => Point::new(
            CALIBRATION_CROSS_MARGIN,
            height - 1 - CALIBRATION_CROSS_MARGIN,
        ),
    }
}
