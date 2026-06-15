#![forbid(unsafe_code)]

use robot_arm_core::{Linkage, Pose, Vec3};
use wasm_bindgen::prelude::wasm_bindgen;

pub use robot_arm_core as core;

const LINKAGE: Linkage<6, 24> = Linkage::start()
    .define_param("lower hand", 0.5)
    .define_param("bend elbow", 0.5)
    .define_param("close hand", 0.5)
    .define_param("lower arm", 0.5)
    .define_param("spin whole arm", 0.5)
    .define_param("spin hand", 0.5)
    .yaw(90.0)
    .yaw_param("spin whole arm", 180.0, -180.0)
    .pitch(90.0)
    .forward(2.5)
    .pitch(-90.0)
    .pitch_param("lower arm", 30.0, 0.0)
    .forward(3.0)
    .yaw_param("bend elbow", 90.0, -90.0)
    .forward(3.0)
    .pitch_param("lower hand", 90.0, -90.0)
    .forward(1.0)
    .roll_param("spin hand", 180.0, -180.0)
    .forward(0.5)
    .yaw(90.0)
    .forward_param("close hand", 0.5, 0.0)
    .yaw(-90.0)
    .forward(1.0)
    .yaw(180.0)
    .forward(1.0)
    .yaw(90.0)
    .forward_param("close hand", 1.0, 0.0)
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
