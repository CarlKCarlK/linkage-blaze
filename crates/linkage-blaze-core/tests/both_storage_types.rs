#![cfg(feature = "alloc")]

use linkage_blaze_core::{
    LinkageBuf, LinkageFixed, Rgb888, Vec3, WebColors, linkage, linkage_buf, linkage_file,
    linkage_fixed,
};

mod common_linkage_tests;
use common_linkage_tests::assert_linkages_equivalent;

linkage_file! { clock_hands { file: "linkages/clock.lb.rs" } }

// Armatron application linkages — mirroring the shared example-core linkage data
linkage_file! {
    camera_control { file: "linkages/camera_control.lb.rs" }
}
linkage_file! {
    grid9x9 { file: "linkages/grid_9x9.lb.rs" }
}
linkage_file! {
    armatron1 { file: "linkages/armatron1.lb.rs" }
}
const POSE_TOLERANCE: f32 = 1e-5;

// TODO0API Named binary intermediate for the camera and grid inputs.
const CAMERA_AND_GRID: LinkageFixed<
    { camera_control::DOF + grid9x9::DOF },
    { camera_control::MARKS + grid9x9::MARKS },
    { camera_control::STEP_COUNT + grid9x9::STEP_COUNT - 1 },
> = camera_control::fixed().combine(grid9x9::view());
// TODO0API The combination reserves one capacity slot for its restore suffix.
const SCENE_WITH_ARM: LinkageFixed<
    { CAMERA_AND_GRID.dof() + armatron1::DOF },
    { CAMERA_AND_GRID.mark_count() + armatron1::MARKS },
    { CAMERA_AND_GRID.step_count() + armatron1::STEP_COUNT },
> = CAMERA_AND_GRID.combine(armatron1::view());
// TODO0API The final fixed combination preserves ownership and uses the annotated output size.
const LINKAGE_FIXED: LinkageFixed<
    { SCENE_WITH_ARM.dof() + armatron1::DOF },
    { SCENE_WITH_ARM.mark_count() + armatron1::MARKS },
    // `combine` skips the ghost arm's `Start`; the other calls each append one step.
    { SCENE_WITH_ARM.step_count() + armatron1::STEP_COUNT - 1 + 3 },
> = SCENE_WITH_ARM
    .restore("scene origin")
    .combine(armatron1::view())
    .pen_color(Rgb888::CSS_RED)
    .sphere_param("close hand", 0.5, 0.0);
// TODO0API Separate arm-tip composition remains a named binary fixed intermediate.
const ARM_TIP_LINKAGE_FIXED: LinkageFixed<
    { camera_control::DOF + armatron1::DOF },
    { camera_control::MARKS + armatron1::MARKS },
    { camera_control::STEP_COUNT + armatron1::STEP_COUNT - 1 },
> = camera_control::fixed().combine(armatron1::view());

#[test]
fn composed_fixed_linkages_have_expected_step_counts() {
    assert_eq!(
        CAMERA_AND_GRID.step_count(),
        camera_control::STEP_COUNT + grid9x9::STEP_COUNT - 1
    );
    assert_eq!(
        SCENE_WITH_ARM.step_count(),
        CAMERA_AND_GRID.step_count() + armatron1::STEP_COUNT - 1
    );
}

#[test]
fn scene_with_arm_fixed_and_buf_are_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let camera_control = camera_control::buf();
    let grid9x9 = grid9x9::buf();
    let armatron1 = armatron1::buf();

    let camera_and_grid: LinkageBuf<
        { camera_control::DOF + grid9x9::DOF },
        { camera_control::MARKS + grid9x9::MARKS },
    > = camera_control.combine(grid9x9.view());
    let scene_with_arm: LinkageBuf<
        { CAMERA_AND_GRID.dof() + armatron1::DOF },
        { CAMERA_AND_GRID.mark_count() + armatron1::MARKS },
    > = camera_and_grid.combine(armatron1.view());

    let params = [0.5_f32; SCENE_WITH_ARM.dof()];
    assert_linkages_equivalent(&SCENE_WITH_ARM.view(), &scene_with_arm.view(), &params)?;
    Ok(())
}

#[test]
fn named_program_buf_matches_fixed() -> Result<(), linkage_blaze_core::Error> {
    let buf = clock_hands::buf();
    assert_linkages_equivalent(&clock_hands::view(), &buf.view(), &[0.25, 0.5])?;
    Ok(())
}

#[test]
fn linkage_fixed_include_works_in_function_body() -> Result<(), linkage_blaze_core::Error> {
    let clock = linkage_fixed!("linkages/clock.lb.rs", clock_hands::DOF, clock_hands::MARKS);
    let clock_explicit =
        linkage_fixed!("linkages/clock.lb.rs", clock_hands::DOF, clock_hands::MARKS);

    assert_eq!(clock.view().dof(), clock_hands::DOF);
    assert_eq!(clock_explicit.view().dof(), clock_hands::DOF);
    let params = [0.25_f32, 0.5];
    let p_ref = clock_hands::view().final_pose(&params)?.position();
    let p_const_explicit = clock_explicit.view().final_pose(&params)?.position();
    let p_local = clock.view().final_pose(&params)?.position();
    assert!(p_ref.is_close_to(&p_const_explicit, POSE_TOLERANCE));
    assert!(p_ref.is_close_to(&p_local, POSE_TOLERANCE));
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_include_works() -> Result<(), linkage_blaze_core::Error> {
    let clock = linkage_buf!("linkages/clock.lb.rs", { clock_hands::DOF }, {
        clock_hands::MARKS
    });
    let clock_explicit = linkage_buf!("linkages/clock.lb.rs", { clock_hands::DOF }, {
        clock_hands::MARKS
    });

    assert_eq!(clock.view().dof(), clock_hands::DOF);
    assert_eq!(clock_explicit.view().dof(), clock_hands::DOF);
    let params = [0.25_f32, 0.5];
    let p_fixed = clock_hands::view().final_pose(&params)?.position();
    let p_buf = clock.view().final_pose(&params)?.position();
    assert!(p_fixed.is_close_to(&p_buf, POSE_TOLERANCE));
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn clock_from_file_both_storage_types() -> Result<(), linkage_blaze_core::Error> {
    let buf = clock_hands::buf();
    let params = [0.25, 0.5];
    assert_linkages_equivalent(&clock_hands::view(), &buf.view(), &params)?;
    Ok(())
}

// ── Application-level linkage tests ──────────────────────────────────────────

#[test]
fn clock_hands_fixed_dims() {
    assert_eq!(clock_hands::fixed().step_count(), clock_hands::STEP_COUNT);
    assert_eq!(clock_hands::view().dof(), clock_hands::DOF);
    assert_eq!(clock_hands::view().len(), clock_hands::STEP_COUNT);
}

#[test]
fn clock_hands_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let buf = LinkageBuf::from(&clock_hands::fixed());
    let params = [0.3_f32, 0.7];
    assert_linkages_equivalent(&clock_hands::view(), &buf.view(), &params)?;
    Ok(())
}

#[test]
fn armatron_component_linkages_fixed_dims() {
    assert_eq!(camera_control::view().dof(), camera_control::DOF);
    assert_eq!(camera_control::view().len(), camera_control::STEP_COUNT);
    assert_eq!(grid9x9::view().dof(), grid9x9::DOF);
    assert_eq!(grid9x9::view().len(), grid9x9::STEP_COUNT);
    assert_eq!(armatron1::view().dof(), armatron1::DOF);
    assert_eq!(armatron1::view().len(), armatron1::STEP_COUNT);
    assert_eq!(CAMERA_AND_GRID.dof(), camera_control::DOF + grid9x9::DOF);
    assert_eq!(
        CAMERA_AND_GRID.len(),
        camera_control::STEP_COUNT + grid9x9::STEP_COUNT - 1
    );
    assert_eq!(
        SCENE_WITH_ARM.view().dof(),
        CAMERA_AND_GRID.dof() + armatron1::DOF
    );
    assert_eq!(
        SCENE_WITH_ARM.view().len(),
        CAMERA_AND_GRID.step_count() + armatron1::STEP_COUNT - 1
    );
    assert_eq!(
        LINKAGE_FIXED.view().dof(),
        SCENE_WITH_ARM.dof() + armatron1::DOF
    );
    assert_eq!(
        LINKAGE_FIXED.view().len(),
        SCENE_WITH_ARM.step_count() + armatron1::STEP_COUNT - 1 + 3
    );
    assert_eq!(
        ARM_TIP_LINKAGE_FIXED.view().dof(),
        camera_control::DOF + armatron1::DOF
    );
    assert_eq!(
        ARM_TIP_LINKAGE_FIXED.view().len(),
        camera_control::STEP_COUNT + armatron1::STEP_COUNT - 1
    );
}

#[test]
fn armatron_component_linkages_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let camera_control_buf = camera_control::buf();
    let armatron1_buf = armatron1::buf();

    let vc_params = [0.5_f32, 0.4, 0.6];
    assert_linkages_equivalent(
        &camera_control::view(),
        &camera_control_buf.view(),
        &vc_params,
    )?;

    let arm_params = [0.5_f32, 0.5, 0.0, 0.5, 0.5, 0.5];
    assert_linkages_equivalent(&armatron1::view(), &armatron1_buf.view(), &arm_params)?;
    Ok(())
}

#[test]
fn armatron_grid_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let grid_buf = grid9x9::buf();
    let params: [f32; grid9x9::DOF] = [];
    assert_linkages_equivalent(&grid9x9::view(), &grid_buf.view(), &params)?;
    Ok(())
}

#[test]
fn armatron_combined_linkages_fixed_and_buf_equivalent() -> Result<(), linkage_blaze_core::Error> {
    let full_buf = LinkageBuf::from(&LINKAGE_FIXED);
    let rk_buf = LinkageBuf::from(&ARM_TIP_LINKAGE_FIXED);

    let full_params = [0.5_f32; LINKAGE_FIXED.dof()];
    let rk_params = [0.5_f32; ARM_TIP_LINKAGE_FIXED.dof()];

    assert_eq!(full_buf.view().dof(), LINKAGE_FIXED.dof());
    assert_eq!(full_buf.view().len(), LINKAGE_FIXED.view().len());
    assert_linkages_equivalent(&LINKAGE_FIXED.view(), &full_buf.view(), &full_params)?;
    assert_linkages_equivalent(&ARM_TIP_LINKAGE_FIXED.view(), &rk_buf.view(), &rk_params)?;
    Ok(())
}

#[test]
fn armatron_full_scene_linkage_built_with_buf() -> Result<(), linkage_blaze_core::Error> {
    let armatron1 = armatron1::buf();
    let camera_control = camera_control::buf();
    let grid9x9 = grid9x9::buf();

    // TODO00PI Buffer combination consumes the left owner and copies the right view.
    let camera_and_grid: LinkageBuf<
        { camera_control::DOF + grid9x9::DOF },
        { camera_control::MARKS + grid9x9::MARKS },
    > = camera_control.clone().combine(grid9x9.view());
    let scene_with_arm: LinkageBuf<
        { CAMERA_AND_GRID.dof() + armatron1::DOF },
        { CAMERA_AND_GRID.mark_count() + armatron1::MARKS },
    > = camera_and_grid.combine(armatron1.clone().view());

    let linkage = scene_with_arm
        .restore("scene origin")
        .combine(armatron1.view())
        .pen_color(Rgb888::CSS_RED)
        .sphere_param("close hand", 0.5, 0.0);

    let arm_tip_linkage = camera_control.combine(armatron1.view());

    assert_eq!(linkage.view().dof(), LINKAGE_FIXED.dof());
    assert_eq!(linkage.view().len(), LINKAGE_FIXED.view().len());
    assert_eq!(arm_tip_linkage.view().dof(), ARM_TIP_LINKAGE_FIXED.dof());
    assert_eq!(
        arm_tip_linkage.view().len(),
        ARM_TIP_LINKAGE_FIXED.step_count()
    );

    let full_params = [0.5_f32; LINKAGE_FIXED.dof()];
    let rk_params = [0.5_f32; ARM_TIP_LINKAGE_FIXED.dof()];
    assert_linkages_equivalent(&LINKAGE_FIXED.view(), &linkage.view(), &full_params)?;
    assert_linkages_equivalent(
        &ARM_TIP_LINKAGE_FIXED.view(),
        &arm_tip_linkage.view(),
        &rk_params,
    )?;
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
            .is_close_to(&buf_result.position(), POSE_TOLERANCE),
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
    let c: LinkageBuf<2, 0> = a.combine(b.view());

    let params = [0.5, 0.75];
    let final_pose = c.view().final_pose(&params)?;

    assert!(
        final_pose
            .position()
            .is_close_to(&Vec3::from([5.0, 3.75, 0.0]), POSE_TOLERANCE),
        "Combined linkage should produce correct pose: got {:?}",
        final_pose.position()
    );
    Ok(())
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_combine_combines_from_view() -> Result<(), linkage_blaze_core::Error> {
    const FIXED_A: LinkageFixed<1, 0, 8> = LinkageFixed::start()
        .define_param("x", 0.5)
        .forward_param("x", 0.0, 10.0);

    const FIXED_B: LinkageFixed<1, 0, 8> = LinkageFixed::start()
        .define_param("y", 0.75)
        .left_param("y", 0.0, 5.0);

    let buf_a = LinkageBuf::from(&FIXED_A);
    let view_b = FIXED_B.view();

    // TODO0API The output type supplies the combined DOF and mark capacities for `combine`.
    let combined: LinkageBuf<
        { FIXED_A.dof() + FIXED_B.dof() },
        { FIXED_A.mark_count() + FIXED_B.mark_count() },
    > = buf_a.combine(view_b);

    let params = [0.5, 0.75];
    let pose = combined.view().final_pose(&params)?;

    assert!(
        pose.position()
            .is_close_to(&Vec3::from([5.0, 3.75, 0.0]), POSE_TOLERANCE),
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
        .pen_color(Rgb888::CSS_DARK_CYAN)
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
    let combined_arm: LinkageBuf<3, 0> = upper_arm.combine(forearm.view());

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
