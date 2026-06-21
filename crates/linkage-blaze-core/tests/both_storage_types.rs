#![cfg(feature = "alloc")]

use linkage_blaze_core::{
    LinkageBuf, LinkageFixed, Rgb888, WebColors, linkage, linkage_buf, linkage_fixed,
};

mod common_linkage_tests;
use common_linkage_tests::assert_linkages_equivalent;

// Clock linkage (N=48 matches the clock-classic application)
const CLOCK_HANDS: LinkageFixed<2, 48> = linkage_fixed!("linkages/clock.lb.rs");

// Armatron application linkages — mirroring linkage-blaze-armatron-core consts
const CAMERA_CONTROL: LinkageFixed<3, 8> = linkage_fixed!("linkages/camera_control.lb.rs");
const GRID_9X9: LinkageFixed<0, 81> = linkage_fixed!("linkages/grid_9x9.lb.rs");
const CAMERA_AND_GRID: LinkageFixed<3, 88> = CAMERA_CONTROL.combine(GRID_9X9);
const ARMATRON1: LinkageFixed<6, 25> = linkage_fixed!("linkages/armatron1.lb.rs");
const ARMATRON1_WITH_JOINTS: LinkageFixed<6, 45> = ARMATRON1.with_joint_spheres(0.15);
const ARMATRON_LINKAGE0: LinkageFixed<9, 133> = CAMERA_AND_GRID.combine(ARMATRON1_WITH_JOINTS);
const ARMATRON_LINKAGE: LinkageFixed<15, 159> = ARMATRON_LINKAGE0
    .restore("scene origin")
    .combine(ARMATRON1)
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0);
const ARMATRON_RK_LINKAGE: LinkageFixed<9, 32> = CAMERA_CONTROL.combine(ARMATRON1);

// Existing clock const (N=128, larger buffer used in tests)
const CLOCK_FIXED: LinkageFixed<2, 128> = linkage_fixed!("linkages/clock.lb.rs");
const CLOCK_FIXED_EXPLICIT: LinkageFixed<2, 128> = linkage_fixed!("linkages/clock.lb.rs", 2, 128);

#[test]
fn linkage_fixed_include_works_in_function_body() {
    let clock: LinkageFixed<2, 128> = linkage_fixed!("linkages/clock.lb.rs");
    let clock_explicit = linkage_fixed!("linkages/clock.lb.rs", 2, 128);

    assert_eq!(clock.view().dof(), 2);
    assert_eq!(clock_explicit.view().dof(), 2);
    let params = [0.25_f32, 0.5];
    let p_ref = CLOCK_FIXED.view().final_pose(&params).position();
    let p_const_explicit = CLOCK_FIXED_EXPLICIT.view().final_pose(&params).position();
    let p_local = clock.view().final_pose(&params).position();
    assert!(p_ref.is_close_to(&p_const_explicit, 1e-5));
    assert!(p_ref.is_close_to(&p_local, 1e-5));
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_include_works() {
    let clock = linkage_buf!("linkages/clock.lb.rs", 2);
    let clock_explicit = linkage_buf!("linkages/clock.lb.rs", 2);

    assert_eq!(clock.view().dof(), 2);
    assert_eq!(clock_explicit.view().dof(), 2);
    let params = [0.25_f32, 0.5];
    let p_fixed = CLOCK_FIXED.view().final_pose(&params).position();
    let p_buf = clock.view().final_pose(&params).position();
    assert!(p_fixed.is_close_to(&p_buf, 1e-5));
}

#[cfg(feature = "alloc")]
#[test]
fn clock_from_file_both_storage_types() {
    let buf = linkage_buf!("linkages/clock.lb.rs", 2);
    let params = [0.25, 0.5];
    assert_linkages_equivalent(&CLOCK_FIXED, &buf, &params);
}

// ── Application-level linkage tests ──────────────────────────────────────────

#[test]
fn clock_hands_fixed_dims() {
    assert_eq!(CLOCK_HANDS.view().dof(), 2);
    assert_eq!(CLOCK_HANDS.view().len(), 46);
}

#[test]
fn clock_hands_fixed_and_buf_equivalent() {
    let buf = LinkageBuf::from(&CLOCK_HANDS);
    let params = [0.3_f32, 0.7];
    assert_linkages_equivalent(&CLOCK_HANDS, &buf, &params);
}

#[test]
fn armatron_component_linkages_fixed_dims() {
    assert_eq!(CAMERA_CONTROL.view().dof(), 3);
    assert_eq!(CAMERA_CONTROL.view().len(), 8);
    assert_eq!(GRID_9X9.view().dof(), 0);
    assert_eq!(GRID_9X9.view().len(), 81);
    assert_eq!(ARMATRON1.view().dof(), 6);
    assert_eq!(ARMATRON1.view().len(), 25);
    assert_eq!(CAMERA_AND_GRID.view().dof(), 3);
    assert_eq!(CAMERA_AND_GRID.view().len(), 88);
    assert_eq!(ARMATRON1_WITH_JOINTS.view().dof(), 6);
    assert_eq!(ARMATRON1_WITH_JOINTS.view().len(), 45);
    assert_eq!(ARMATRON_LINKAGE0.view().dof(), 9);
    assert_eq!(ARMATRON_LINKAGE0.view().len(), 132);
    assert_eq!(ARMATRON_LINKAGE.view().dof(), 15);
    assert_eq!(ARMATRON_LINKAGE.view().len(), 159);
    assert_eq!(ARMATRON_RK_LINKAGE.view().dof(), 9);
    assert_eq!(ARMATRON_RK_LINKAGE.view().len(), 32);
}

#[test]
fn armatron_component_linkages_fixed_and_buf_equivalent() {
    let camera_control_buf = linkage_buf!("linkages/camera_control.lb.rs", 3);
    let armatron1_buf = linkage_buf!("linkages/armatron1.lb.rs", 6);

    let vc_params = [0.5_f32, 0.4, 0.6];
    assert_linkages_equivalent(&CAMERA_CONTROL, &camera_control_buf, &vc_params);

    let arm_params = [0.5_f32, 0.5, 0.0, 0.5, 0.5, 0.5];
    assert_linkages_equivalent(&ARMATRON1, &armatron1_buf, &arm_params);
}

#[test]
fn armatron_grid_fixed_and_buf_equivalent() {
    let grid_buf = linkage_buf!("linkages/grid_9x9.lb.rs", 0);
    let params: [f32; 0] = [];
    assert_linkages_equivalent(&GRID_9X9, &grid_buf, &params);
}

#[test]
fn armatron_combined_linkages_fixed_and_buf_equivalent() {
    let full_buf = LinkageBuf::from(&ARMATRON_LINKAGE);
    let rk_buf = LinkageBuf::from(&ARMATRON_RK_LINKAGE);

    let full_params = [0.5_f32; 15];
    let rk_params = [0.5_f32; 9];

    assert_eq!(full_buf.view().dof(), 15);
    assert_eq!(full_buf.view().len(), ARMATRON_LINKAGE.view().len());
    assert_linkages_equivalent(&ARMATRON_LINKAGE, &full_buf, &full_params);
    assert_linkages_equivalent(&ARMATRON_RK_LINKAGE, &rk_buf, &rk_params);
}

#[test]
fn armatron_full_scene_linkage_built_with_buf() {
    // Each file loaded exactly once; DOF is in the macro, not the binding.
    let armatron1 = linkage_buf!("linkages/armatron1.lb.rs", 6);
    let camera_control = linkage_buf!("linkages/camera_control.lb.rs", 3);
    let grid_9x9 = linkage_buf!("linkages/grid_9x9.lb.rs", 0);

    let camera_and_grid: LinkageBuf<3> = camera_control.combine_ref(grid_9x9.view());
    let linkage0: LinkageBuf<9> = camera_and_grid.combine(armatron1.with_joint_spheres_ref(0.15));

    let full_linkage = linkage0
        .restore("scene origin")
        .combine_ref(armatron1.view())
        .pen_color(Rgb888::CSS_RED)
        .sphere_param("close hand", 0.5, 0.0);

    let rk_linkage = camera_control.combine(armatron1);

    assert_eq!(full_linkage.view().dof(), 15);
    assert_eq!(full_linkage.view().len(), ARMATRON_LINKAGE.view().len());
    assert_eq!(rk_linkage.view().dof(), 9);
    assert_eq!(rk_linkage.view().len(), ARMATRON_RK_LINKAGE.view().len());

    let full_params = [0.5_f32; 15];
    let rk_params = [0.5_f32; 9];
    assert_linkages_equivalent(&ARMATRON_LINKAGE, &full_linkage, &full_params);
    assert_linkages_equivalent(&ARMATRON_RK_LINKAGE, &rk_linkage, &rk_params);
}

#[cfg(feature = "alloc")]
#[test]
fn conversion_linkage_fixed_to_buf() {
    const FIXED: LinkageFixed<2, 16> = LinkageFixed::start()
        .define_param("x", 0.5)
        .define_param("y", 0.75)
        .forward_param("x", 0.0, 10.0)
        .left_param("y", 0.0, 5.0);

    let buf = LinkageBuf::from(&FIXED);

    let params = [0.5, 0.75];
    let fixed_result = FIXED.view().final_pose(&params);
    let buf_result = buf.view().final_pose(&params);

    assert!(
        fixed_result
            .position()
            .is_close_to(&buf_result.position(), 1e-5),
        "Converted linkage should produce same results"
    );
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_combine_combines_params_and_steps() {
    let a = LinkageBuf::<1>::start()
        .define_param("x", 0.5)
        .forward_param("x", 0.0, 10.0);

    let b = LinkageBuf::<1>::start()
        .define_param("y", 0.75)
        .left_param("y", 0.0, 5.0);

    // todo0000000 what are these right hand side numbers? needed? in best order?
    let c = a.combine(b);

    let params = [0.5, 0.75];
    let final_pose = c.view().final_pose(&params);

    assert!(
        final_pose
            .position()
            .is_close_to(&linkage_blaze_core::Vec3::from([5.0, 3.75, 0.0]), 1e-5),
        "Combined linkage should produce correct pose: got {:?}",
        final_pose.position()
    );
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_combine_ref_combines_from_view() {
    const FIXED_A: LinkageFixed<1, 8> = LinkageFixed::start()
        .define_param("x", 0.5)
        .forward_param("x", 0.0, 10.0);

    const FIXED_B: LinkageFixed<1, 8> = LinkageFixed::start()
        .define_param("y", 0.75)
        .left_param("y", 0.0, 5.0);

    let buf_a = LinkageBuf::from(&FIXED_A);
    let view_b = FIXED_B.view();

    // todo0000000 what are these right hand side numbers? needed? in best order?

    let combined = buf_a.combine_ref(view_b);

    let params = [0.5, 0.75];
    let pose = combined.view().final_pose(&params);

    assert!(
        pose.position()
            .is_close_to(&linkage_blaze_core::Vec3::from([5.0, 3.75, 0.0]), 1e-5),
        "Extended linkage should produce correct pose"
    );
}

#[cfg(feature = "alloc")]
#[test]
fn armatron_buf_combine_combines_limbs() {
    // Build arm limbs separately as LinkageBuf instances
    // Upper arm: rotate with spin_whole_arm, move forward
    let upper_arm: LinkageBuf<2> = LinkageBuf::start()
        .define_param("spin whole arm", 0.5)
        .define_param("lower arm", 0.5)
        .yaw_param("spin whole arm", 180.0, -180.0)
        .pen_color(Rgb888::new(0, 139, 139))
        .pen_width(0.15)
        .up(2.5)
        .pitch_param("lower arm", -30.0, 0.0)
        .forward(3.0);

    // Forearm: rotate with bend_elbow
    let forearm: LinkageBuf<1> = LinkageBuf::start()
        .define_param("bend elbow", 0.5)
        .yaw_param("bend elbow", 90.0, -90.0)
        .forward(3.0);

    // todo0000000 what are these right hand side numbers? needed? in best order?

    let combined_arm = upper_arm.combine(forearm);

    let params = [0.5, 0.5, 0.5]; // spin_whole_arm, lower_arm, bend_elbow
    let pose = combined_arm.view().final_pose(&params);

    let steps = combined_arm.view().len();
    // 1 Start + 1 yaw + 1 pen_color + 1 pen_width + 1 up + 1 pitch + 1 forward (from upper_arm)
    // + 1 yaw + 1 forward (from forearm) = 9 steps
    assert!(
        steps >= 9,
        "Combined arm should have steps from both limbs, got {}",
        steps
    );

    let final_position = pose.position();
    assert!(
        final_position[2] >= 2.0, // Should be up by at least 2.5
        "Combined arm should maintain height from upper arm"
    );
}
