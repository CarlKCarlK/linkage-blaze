#![forbid(unsafe_code)]

use robot_arm_core::{Linkage, Pose, Vec3};
use wasm_bindgen::prelude::wasm_bindgen;

pub use robot_arm_core as core;

const LINKAGE: Linkage<6, 24> = Linkage::start()
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
    .move_param(2, 0.5, 0.0) // close hand
    .yaw(-90.0)
    .forward(1.0)
    .yaw(180.0)
    .forward(1.0)
    .yaw(90.0)
    .move_param(2, 1.0, 0.0) // close hand
    .yaw(90.0)
    .forward(1.0);

#[wasm_bindgen]
pub fn dof() -> usize {
    LINKAGE.dof()
}

#[wasm_bindgen]
pub fn len() -> usize {
    LINKAGE.len()
}

#[wasm_bindgen]
pub fn linkage_points(params: Vec<f32>) -> Vec<f32> {
    let params = params
        .as_slice()
        .try_into()
        .expect("expected linkage param count");

    LINKAGE
        .poses(params)
        .map(Pose::position)
        .flat_map(Vec3::into_array)
        .collect()
}
