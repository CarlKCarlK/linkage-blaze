#![cfg(feature = "alloc")]

use linkage_blaze_core::{LinkageFixed, LinkageBuf, Rgb888};

mod common_linkage_tests;
use common_linkage_tests::assert_linkages_equivalent;

#[cfg(feature = "alloc")]
#[test]
fn clock_linkage_works_with_both_storage_types() {
    // Define the clock linkage with LinkageFixed
    const LINKAGE_FIXED: LinkageFixed<2, 48> = LinkageFixed::start()
        .define_param("hour", 0.0)
        .define_param("face spin", 0.5)
        .roll_param("face spin", -90.0, 90.0)
        .mark("face")
        .pen_color(Rgb888::new(33, 79, 155))
        .disk(66.0)
        .restore("face")
        .pen_width(4.5)
        .pen_color(Rgb888::new(255, 245, 216))
        .pen_up()
        .mark("ticks")
        .forward(45.0)
        .pen_down()
        .forward(18.0);

    // Create the same linkage with LinkageBuf
    let linkage_buf: LinkageBuf<2> = LinkageBuf::start()
        .define_param("hour", 0.0)
        .define_param("face spin", 0.5)
        .roll_param("face spin", -90.0, 90.0)
        .mark("face")
        .pen_color(Rgb888::new(33, 79, 155))
        .disk(66.0)
        .restore("face")
        .pen_width(4.5)
        .pen_color(Rgb888::new(255, 245, 216))
        .pen_up()
        .mark("ticks")
        .forward(45.0)
        .pen_down()
        .forward(18.0);

    let params = [0.5, 0.5]; // hour=0.5, face_spin=0.5
    assert_linkages_equivalent(&LINKAGE_FIXED, &linkage_buf, &params);
}

#[cfg(feature = "alloc")]
#[test]
fn armatron_linkage_works_with_both_storage_types() {
    // Define the armatron linkage with LinkageFixed
    const LINKAGE_FIXED: LinkageFixed<6, 32> = LinkageFixed::start()
        .define_param("raise hand", 0.5)
        .define_param("bend elbow", 0.5)
        .define_param("close hand", 0.0)
        .define_param("lower arm", 0.5)
        .define_param("spin whole arm", 0.5)
        .define_param("spin hand", 0.5)
        .yaw_param("spin whole arm", 180.0, -180.0)
        .pen_color(Rgb888::new(0, 139, 139))
        .pen_width(0.15)
        .up(2.5)
        .pitch_param("lower arm", -30.0, 0.0)
        .forward(3.0)
        .yaw_param("bend elbow", 90.0, -90.0)
        .forward(3.0)
        .pitch_param("raise hand", 90.0, -90.0)
        .forward(1.0)
        .roll_param("spin hand", -180.0, 180.0)
        .forward(0.5)
        .mark("wrist")
        .yaw(90.0)
        .forward_param("close hand", 0.5, 0.0)
        .left(-1.0);

    // Create the same linkage with LinkageBuf
    let linkage_buf: LinkageBuf<6> = LinkageBuf::start()
        .define_param("raise hand", 0.5)
        .define_param("bend elbow", 0.5)
        .define_param("close hand", 0.0)
        .define_param("lower arm", 0.5)
        .define_param("spin whole arm", 0.5)
        .define_param("spin hand", 0.5)
        .yaw_param("spin whole arm", 180.0, -180.0)
        .pen_color(Rgb888::new(0, 139, 139))
        .pen_width(0.15)
        .up(2.5)
        .pitch_param("lower arm", -30.0, 0.0)
        .forward(3.0)
        .yaw_param("bend elbow", 90.0, -90.0)
        .forward(3.0)
        .pitch_param("raise hand", 90.0, -90.0)
        .forward(1.0)
        .roll_param("spin hand", -180.0, 180.0)
        .forward(0.5)
        .mark("wrist")
        .yaw(90.0)
        .forward_param("close hand", 0.5, 0.0)
        .left(-1.0);

    let params = [0.7, 0.5, 0.2, 1.0, 0.6, 0.0];
    assert_linkages_equivalent(&LINKAGE_FIXED, &linkage_buf, &params);
}

#[cfg(feature = "alloc")]
#[test]
fn grid_9x9_linkage_works_with_both_storage_types() {
    // Simple test showing both types can instantiate the same expression
    const LINKAGE_FIXED: LinkageFixed<0, 16> = LinkageFixed::start()
        .pen_color(Rgb888::new(0, 0, 255))
        .disk(1.0)
        .forward(2.0)
        .disk(1.0)
        .forward(2.0)
        .disk(1.0);

    let linkage_buf: LinkageBuf<0> = LinkageBuf::start()
        .pen_color(Rgb888::new(0, 0, 255))
        .disk(1.0)
        .forward(2.0)
        .disk(1.0)
        .forward(2.0)
        .disk(1.0);

    let params = [];
    assert_linkages_equivalent(&LINKAGE_FIXED, &linkage_buf, &params);
}

#[cfg(feature = "alloc")]
#[test]
fn conversion_linkage_fixed_to_buf() {
    const FIXED: LinkageFixed<2, 16> = LinkageFixed::start()
        .define_param("x", 0.5)
        .define_param("y", 0.75)
        .forward_param("x", 0.0, 10.0)
        .left_param("y", 0.0, 5.0);

    // Convert fixed to buf
    let buf: LinkageBuf<2> = LinkageBuf::from(&FIXED);

    let params = [0.5, 0.75];
    let fixed_result = FIXED.view().final_pose(&params);
    let buf_result = buf.view().final_pose(&params);

    assert!(
        fixed_result.position().is_close_to(&buf_result.position(), 1e-5),
        "Converted linkage should produce same results"
    );
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_append_combines_params_and_steps() {
    // Create two simple LinkageBuf instances
    let a = LinkageBuf::<1>::start()
        .define_param("x", 0.5)
        .forward_param("x", 0.0, 10.0);

    let b = LinkageBuf::<1>::start()
        .define_param("y", 0.75)
        .left_param("y", 0.0, 5.0);

    // Append them to create a combined linkage with DOF=2
    let c: LinkageBuf<2> = a.append::<1, 2>(b);

    // Verify combined linkage has correct DOF and evaluation
    let params = [0.5, 0.75];
    let final_pose = c.view().final_pose(&params);

    // Expected position: forward 5.0 (at x=0.5) then left 3.75 (at y=0.75)
    // Final position should be at approximately (5.0, 3.75, 0.0)
    assert!(
        final_pose.position().is_close_to(&linkage_blaze_core::Vec3::from([5.0, 3.75, 0.0]), 1e-5),
        "Combined linkage should produce correct pose: got {:?}",
        final_pose.position()
    );
}

#[cfg(feature = "alloc")]
#[test]
fn linkage_buf_extend_view_combines_from_view() {
    const FIXED_A: LinkageFixed<1, 8> = LinkageFixed::start()
        .define_param("x", 0.5)
        .forward_param("x", 0.0, 10.0);

    const FIXED_B: LinkageFixed<1, 8> = LinkageFixed::start()
        .define_param("y", 0.75)
        .left_param("y", 0.0, 5.0);

    // Create LinkageBuf from fixed, then extend with a view
    let buf_a: LinkageBuf<1> = LinkageBuf::from(&FIXED_A);
    let view_b = FIXED_B.view();

    let combined: LinkageBuf<2> = buf_a.extend_view::<1, 2>(view_b);

    // Verify the result
    let params = [0.5, 0.75];
    let pose = combined.view().final_pose(&params);

    assert!(
        pose.position().is_close_to(&linkage_blaze_core::Vec3::from([5.0, 3.75, 0.0]), 1e-5),
        "Extended linkage should produce correct pose"
    );
}

/// Test clock with LinkageFixed using shared clock_expr.rs via include!()
///
/// # One Source of Truth via include!()
///
/// The `clock_expr.rs` file defines the complete fluent DSL chain and is included
/// by both LinkageFixed and LinkageBuf tests, ensuring they use identical definitions.
///
/// The trick: `clock_expr.rs` uses a local variable named `start` that gets bound
/// to the appropriate `.start()` call within a block. This makes the included content
/// a complete expression (not a dangling method-chain suffix), allowing include!() to work.
///
/// See: tests/linkages/clock_expr.rs
#[cfg(feature = "alloc")]
#[test]
fn real_clock_fixed() {
    //todo00 Article: explain the "start" trick for sharing include!() content across types.
    // The key insight is that include!() expands to a complete expression when the file
    // uses a bound variable (like "start") that can be different types in different contexts.
    // This eliminates duplication while keeping one source of truth.
    const LINKAGE: LinkageFixed<2, 128> = {
        let start = LinkageFixed::start();
        include!("linkages/clock_expr.rs")
    };

    // Basic validation of the linkage structure
    assert_eq!(LINKAGE.view().dof(), 2);
    assert!(LINKAGE.view().len() > 20, "Clock should have many steps");
}

/// Test clock with LinkageBuf using shared clock_expr.rs via include!()
#[cfg(feature = "alloc")]
#[test]
fn real_clock_buf() {
    let linkage: LinkageBuf<2> = {
        let start = LinkageBuf::start();
        include!("linkages/clock_expr.rs")
    };

    assert_eq!(linkage.view().dof(), 2);
    assert!(linkage.view().len() > 20);
}

/// Test that both clock definitions produce identical results
/// Both use the SAME shared clock_expr.rs via include!() - one source of truth
#[cfg(feature = "alloc")]
#[test]
fn real_clock_definition_works_with_both_storage_types() {
    const LINKAGE_FIXED: LinkageFixed<2, 128> = {
        let start = LinkageFixed::start();
        include!("linkages/clock_expr.rs")
    };

    let linkage_buf: LinkageBuf<2> = {
        let start = LinkageBuf::start();
        include!("linkages/clock_expr.rs")
    };

    let params = [0.25, 0.5]; // hour=0.25 (3 o'clock), face_spin=0.5
    assert_linkages_equivalent(&LINKAGE_FIXED, &linkage_buf, &params);
}

#[cfg(feature = "alloc")]
#[test]
fn armatron_buf_append_combines_limbs() {
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

    // Combine upper and forearm
    let combined_arm: LinkageBuf<3> = upper_arm.append::<1, 3>(forearm);

    // Verify the combined arm produces consistent results
    let params = [0.5, 0.5, 0.5]; // spin_whole_arm, lower_arm, bend_elbow
    let pose = combined_arm.view().final_pose(&params);

    // Should have moved forward 3.0 (from upper_arm) + 3.0 (from forearm) = 6.0 along initial direction
    // Upper arm is at middle position for rotation and pitch
    let steps = combined_arm.view().len();
    // 1 Start + 1 yaw + 1 pen_color + 1 pen_width + 1 up + 1 pitch + 1 forward (from upper_arm)
    // + 1 yaw + 1 forward (from forearm) = 9 steps
    assert!(steps >= 9, "Combined arm should have steps from both limbs, got {}", steps);

    // Final pose should exist and be valid
    let final_position = pose.position();
    assert!(
        final_position[2] >= 2.0, // Should be up by at least 2.5
        "Combined arm should maintain height from upper arm"
    );
}
