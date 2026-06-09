#![forbid(unsafe_code)]

use robot_arm_core::Linkage;
use wasm_bindgen::prelude::wasm_bindgen;

pub use robot_arm_core as core;

//todo0000 reconsider all these constant definitions.
const DOF_COUNT: usize = 6;
const POINT_COUNT: usize = 24;

const LINKAGE0: Linkage<DOF_COUNT, POINT_COUNT> = Linkage::start()
    .yaw(90.0)
    .yaw_param(4, 180.0, -180.0) // spin whole arm
    .pitch(90.0)
    .forward(2.5)
    .pitch(-90.0)
    .pitch_param(3, 30.0, 0.0) // lower arm
    .forward(3.0)
    .yaw_param(1, 90.0, -90.0) // bend elbow
    .forward(3.0)
    .pitch_param(0, 90.0, -90.0) // lower hand
    .forward(1.0)
    .roll_param(5, 180.0, -180.0) // spin hand
    .forward(0.5)
    .yaw(90.0)
    .move_param(2, 0.0, 0.5) // close hand
    .yaw(-90.0)
    .forward(1.0)
    .yaw(180.0)
    .forward(1.0)
    .yaw(90.0)
    .move_param(2, 0.0, 1.0) // close hand
    .yaw(90.0)
    .forward(1.0);

#[wasm_bindgen]
pub fn linkage0_dof_count() -> usize {
    DOF_COUNT
}

#[wasm_bindgen]
pub fn linkage0_point_count() -> usize {
    POINT_COUNT
}

#[wasm_bindgen]
pub fn linkage0_points(params: Vec<f32>) -> Vec<f32> {
    assert!(params.len() == DOF_COUNT, "expected 6 params");

    let params = [
        params[0], // lower hand
        params[1], // bend elbow
        params[2], // close hand
        params[3], // lower arm
        params[4], // spin whole arm
        params[5], // spin hand
    ];

    let mut points = Vec::with_capacity(POINT_COUNT * 3);
    for pose in LINKAGE0.poses(&params) {
        points.push(pose.position[0]);
        points.push(pose.position[1]);
        points.push(pose.position[2]);
    }
    points
}
