#![cfg(feature = "alloc")]

use linkage_blaze_core::{
    LinkageBuf, LinkageFixed, Rgb888, WebColors, linkage, linkage_buf, linkage_combine,
    linkage_extend, linkage_fixed, linkage_program, linkage_with_joint_spheres,
};

mod common_linkage_tests;
use common_linkage_tests::assert_linkages_equivalent;

linkage_program! { ClockHands { file: "linkages/clock.lb.rs", dof: 2, marks: 2 } }

// Armatron application linkages — mirroring the shared example-core linkage data
linkage_program! {
    CameraControl { file: "linkages/camera_control.lb.rs", dof: 3, marks: 1 }
    Grid9x9 { file: "linkages/grid_9x9.lb.rs", dof: 0, marks: 1 }
    Armatron1 { file: "linkages/armatron1.lb.rs", dof: 6, marks: 1 }
}
const CAMERA_AND_GRID: LinkageFixed<3, 2, 88> =
    linkage_combine!(CameraControl::fixed(), Grid9x9::fixed());
const ARMATRON1_WITH_JOINTS: LinkageFixed<6, 1, 45> =
    linkage_with_joint_spheres!(Armatron1::fixed(), 0.15);
linkage_program! {
    SceneWithArm {
        program: linkage_combine!(
            CameraControl::fixed(),
            Grid9x9::fixed(),
            linkage_with_joint_spheres!(Armatron1::fixed(), 0.15),
        ),
        dof: 9,
        marks: 3,
    }
}
const ARMATRON_LINKAGE0: LinkageFixed<9, 3, 132> = SceneWithArm::fixed();
const ARMATRON_LINKAGE0_RESTORED: LinkageFixed<9, 3, 133> =
    linkage_extend!(SceneWithArm::fixed(); .restore("scene origin"));
const ARMATRON_LINKAGE: LinkageFixed<15, 4, 159> = linkage_extend!(
    linkage_combine!(ARMATRON_LINKAGE0_RESTORED, Armatron1::fixed());
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0)
);
const ARMATRON_RK_LINKAGE: LinkageFixed<9, 2, 32> =
    linkage_combine!(CameraControl::fixed(), Armatron1::fixed());

const MEASURED_CLOCK: LinkageFixed<2, 2, 46> = ClockHands::fixed();

const DERIVED_COMBINATION: LinkageFixed<3, 2, 88> =
    linkage_combine!(CameraControl::fixed(), Grid9x9::fixed());
const VARIADIC_COMBINATION: LinkageFixed<9, 3, 132> = linkage_combine!(
    CameraControl::fixed(),
    Grid9x9::fixed(),
    linkage_with_joint_spheres!(Armatron1::fixed(), 0.15),
);
const DERIVED_JOINTS: LinkageFixed<6, 1, 45> =
    linkage_with_joint_spheres!(Armatron1::fixed(), 0.15);

const CLOCK_FIXED: LinkageFixed<2, 2, 46> = ClockHands::fixed();
const CLOCK_FIXED_EXPLICIT: LinkageFixed<2, 2, 46> = ClockHands::fixed();

#[test]
fn derived_fixed_macros_preserve_exact_sizes() {
    assert_eq!(DERIVED_COMBINATION.step_count(), 88);
    assert_eq!(
        VARIADIC_COMBINATION.step_count(),
        ARMATRON_LINKAGE0.step_count()
    );
    assert_eq!(DERIVED_JOINTS.step_count(), 45);
}

#[test]
fn variadic_combination_matches_left_associative_nesting() -> Result<(), linkage_blaze_core::Error>
{
    let params = [0.5_f32; 9];
    assert_linkages_equivalent(&VARIADIC_COMBINATION, &ARMATRON_LINKAGE0, &params)?;
    Ok(())
}

#[test]
fn named_program_buf_matches_fixed() -> Result<(), linkage_blaze_core::Error> {
    let buf = ClockHands::buf();
    assert_linkages_equivalent(&ClockHands::fixed(), &buf, &[0.25, 0.5])?;
    Ok(())
}

#[test]
fn linkage_fixed_include_works_in_function_body() -> Result<(), linkage_blaze_core::Error> {
    let clock = linkage_fixed!("linkages/clock.lb.rs", 2, 2);
    let clock_explicit = linkage_fixed!("linkages/clock.lb.rs", 2, 2);

    assert_eq!(clock.view().dof(), 2);
    assert_eq!(clock_explicit.view().dof(), 2);
    let params = [0.25_f32, 0.5];
    let p_ref = CLOCK_FIXED.view().final_pose(&params)?.position();
    let p_const_explicit = CLOCK_FIXED_EXPLICIT.view().final_pose(&params)?.position();
    let p_local = clock.view().final_pose(&params)?.position();
    assert!(p_ref.is_close_to(&p_const_explicit, 1e-5));
    assert!(p_ref.is_close_to(&p_local, 1e-5));
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_include_works() -> Result<(), linkage_blaze_core::Error> {
    let clock = linkage_buf!("linkages/clock.lb.rs", 2, 2);
    let clock_explicit = linkage_buf!("linkages/clock.lb.rs", 2, 2);

    assert_eq!(clock.view().dof(), 2);
    assert_eq!(clock_explicit.view().dof(), 2);
    let params = [0.25_f32, 0.5];
    let p_fixed = CLOCK_FIXED.view().final_pose(&params)?.position();
    let p_buf = clock.view().final_pose(&params)?.position();
    assert!(p_fixed.is_close_to(&p_buf, 1e-5));
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn clock_from_file_both_storage_types() -> Result<(), linkage_blaze_core::Error> {
    let buf = linkage_buf!("linkages/clock.lb.rs", 2, 2);
    let params = [0.25, 0.5];
    assert_linkages_equivalent(&CLOCK_FIXED, &buf, &params)?;
    Ok(())
}

// ── Application-level linkage tests ──────────────────────────────────────────

#[test]
fn clock_hands_fixed_dims() {
    assert_eq!(ClockHands::STEP_COUNT, 46);
    assert_eq!(MEASURED_CLOCK.step_count(), ClockHands::STEP_COUNT);
    assert_eq!(ClockHands::VIEW.dof(), 2);
    assert_eq!(ClockHands::VIEW.len(), 46);
}

#[test]
fn clock_hands_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let buf = LinkageBuf::from(&ClockHands::fixed());
    let params = [0.3_f32, 0.7];
    assert_linkages_equivalent(&ClockHands::fixed(), &buf, &params)?;
    Ok(())
}

#[test]
fn armatron_component_linkages_fixed_dims() {
    assert_eq!(CameraControl::VIEW.dof(), 3);
    assert_eq!(CameraControl::VIEW.len(), 8);
    assert_eq!(Grid9x9::VIEW.dof(), 0);
    assert_eq!(Grid9x9::VIEW.len(), 81);
    assert_eq!(Armatron1::VIEW.dof(), 6);
    assert_eq!(Armatron1::VIEW.len(), 25);
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
fn armatron_component_linkages_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let camera_control_buf = linkage_buf!("linkages/camera_control.lb.rs", 3, 1);
    let armatron1_buf = linkage_buf!("linkages/armatron1.lb.rs", 6, 1);

    let vc_params = [0.5_f32, 0.4, 0.6];
    assert_linkages_equivalent(&CameraControl::fixed(), &camera_control_buf, &vc_params)?;

    let arm_params = [0.5_f32, 0.5, 0.0, 0.5, 0.5, 0.5];
    assert_linkages_equivalent(&Armatron1::fixed(), &armatron1_buf, &arm_params)?;
    Ok(())
}

#[test]
fn armatron_grid_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let grid_buf = linkage_buf!("linkages/grid_9x9.lb.rs", 0, 1);
    let params: [f32; 0] = [];
    assert_linkages_equivalent(&Grid9x9::fixed(), &grid_buf, &params)?;
    Ok(())
}

#[test]
fn armatron_combined_linkages_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let full_buf = LinkageBuf::from(&ARMATRON_LINKAGE);
    let rk_buf = LinkageBuf::from(&ARMATRON_RK_LINKAGE);

    let full_params = [0.5_f32; 15];
    let rk_params = [0.5_f32; 9];

    assert_eq!(full_buf.view().dof(), 15);
    assert_eq!(full_buf.view().len(), ARMATRON_LINKAGE.view().len());
    assert_linkages_equivalent(&ARMATRON_LINKAGE, &full_buf, &full_params)?;
    assert_linkages_equivalent(&ARMATRON_RK_LINKAGE, &rk_buf, &rk_params)?;
    Ok(())
}

#[test]
fn armatron_full_scene_linkage_built_with_buf() -> Result<(), linkage_blaze_core::Error> {
    // Each file loaded exactly once; DOF is in the macro, not the binding.
    let armatron1 = linkage_buf!("linkages/armatron1.lb.rs", 6, 1);
    let camera_control = linkage_buf!("linkages/camera_control.lb.rs", 3, 1);
    let grid_9x9 = linkage_buf!("linkages/grid_9x9.lb.rs", 0, 1);

    let camera_and_grid: LinkageBuf<3, 2> = camera_control.combine_ref(grid_9x9.view());
    let linkage0: LinkageBuf<9, 3> =
        camera_and_grid.combine(armatron1.with_joint_spheres_ref(0.15));

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
    assert_linkages_equivalent(&ARMATRON_LINKAGE, &full_linkage, &full_params)?;
    assert_linkages_equivalent(&ARMATRON_RK_LINKAGE, &rk_linkage, &rk_params)?;
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn conversion_linkage_fixed_to_buf() -> Result<(), linkage_blaze_core::Error> {
    const FIXED: LinkageFixed<2, 0, 16> = LinkageFixed::start()
        .define_param("x", 0.5)
        .define_param("y", 0.75)
        .forward_param("x", 0.0, 10.0)
        .left_param("y", 0.0, 5.0);

    let buf = LinkageBuf::from(&FIXED);

    let params = [0.5, 0.75];
    let fixed_result = FIXED.view().final_pose(&params)?;
    let buf_result = buf.view().final_pose(&params)?;

    assert!(
        fixed_result
            .position()
            .is_close_to(&buf_result.position(), 1e-5),
        "Converted linkage should produce same results"
    );
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_combine_combines_params_and_steps() -> Result<(), linkage_blaze_core::Error> {
    let a = LinkageBuf::<1, 0>::start()
        .define_param("x", 0.5)
        .forward_param("x", 0.0, 10.0);

    let b = LinkageBuf::<1, 0>::start()
        .define_param("y", 0.75)
        .left_param("y", 0.0, 5.0);

    // The output type supplies the combined DOF and mark capacities for `combine`.
    let c: LinkageBuf<2, 0> = a.combine(b);

    let params = [0.5, 0.75];
    let final_pose = c.view().final_pose(&params)?;

    assert!(
        final_pose
            .position()
            .is_close_to(&linkage_blaze_core::Vec3::from([5.0, 3.75, 0.0]), 1e-5),
        "Combined linkage should produce correct pose: got {:?}",
        final_pose.position()
    );
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_combine_ref_combines_from_view() -> Result<(), linkage_blaze_core::Error> {
    const FIXED_A: LinkageFixed<1, 0, 8> = LinkageFixed::start()
        .define_param("x", 0.5)
        .forward_param("x", 0.0, 10.0);

    const FIXED_B: LinkageFixed<1, 0, 8> = LinkageFixed::start()
        .define_param("y", 0.75)
        .left_param("y", 0.0, 5.0);

    let buf_a = LinkageBuf::from(&FIXED_A);
    let view_b = FIXED_B.view();

    // The output type supplies the combined DOF and mark capacities for `combine_ref`.
    let combined: LinkageBuf<2, 0> = buf_a.combine_ref(view_b);

    let params = [0.5, 0.75];
    let pose = combined.view().final_pose(&params)?;

    assert!(
        pose.position()
            .is_close_to(&linkage_blaze_core::Vec3::from([5.0, 3.75, 0.0]), 1e-5),
        "Extended linkage should produce correct pose"
    );
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn armatron_buf_combine_combines_limbs() -> Result<(), linkage_blaze_core::Error> {
    // Build arm limbs separately as LinkageBuf instances
    // Upper arm: rotate with spin_whole_arm, move forward
    let upper_arm: LinkageBuf<2, 0> = LinkageBuf::start()
        .define_param("spin whole arm", 0.5)
        .define_param("lower arm", 0.5)
        .yaw_param("spin whole arm", 180.0, -180.0)
        .pen_color(Rgb888::new(0, 139, 139))
        .pen_width(0.15)
        .up(2.5)
        .pitch_param("lower arm", -30.0, 0.0)
        .forward(3.0);

    // Forearm: rotate with bend_elbow
    let forearm: LinkageBuf<1, 0> = LinkageBuf::start()
        .define_param("bend elbow", 0.5)
        .yaw_param("bend elbow", 90.0, -90.0)
        .forward(3.0);

    // The output type supplies the combined DOF and mark capacities for `combine`.
    let combined_arm: LinkageBuf<3, 0> = upper_arm.combine(forearm);

    let params = [0.5, 0.5, 0.5]; // spin_whole_arm, lower_arm, bend_elbow
    let pose = combined_arm.view().final_pose(&params)?;

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
    Ok(())
}
