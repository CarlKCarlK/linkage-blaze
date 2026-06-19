#![cfg(feature = "alloc")]

use linkage_blaze_core::{LinkageFixed, LinkageBuf, Rgb888};

#[cfg(feature = "alloc")]
#[test]
fn clock_linkage_works_with_both_storage_types() {
    // Define the clock linkage with LinkageFixed
    const CLOCK_FIXED: LinkageFixed<2, 48> = LinkageFixed::start()
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
    let clock_buf = LinkageBuf::start()
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

    // Evaluate both with the same parameters
    let params = [0.5, 0.5]; // hour=0.5, face_spin=0.5
    let fixed_view = CLOCK_FIXED.view();
    let buf_view = clock_buf.view();

    // Compare basic properties
    assert_eq!(fixed_view.dof(), buf_view.dof(), "DOF should match");
    assert_eq!(fixed_view.len(), buf_view.len(), "Step count should match");

    // Compare evaluation results
    let fixed_final = fixed_view.final_pose(&params);
    let buf_final = buf_view.final_pose(&params);

    assert!(
        fixed_final.position().is_close_to(&buf_final.position(), 1e-5),
        "Final pose position should match: fixed={:?}, buf={:?}",
        fixed_final.position(),
        buf_final.position()
    );

    assert!(
        fixed_final.orientation().is_close_to(&buf_final.orientation(), 1e-5),
        "Final pose orientation should match"
    );

    // Count draw items - both should produce the same number
    let fixed_items: Vec<_> = fixed_view.draw_items(&params).collect();
    let buf_items: Vec<_> = buf_view.draw_items(&params).collect();
    assert_eq!(
        fixed_items.len(),
        buf_items.len(),
        "Number of draw items should match"
    );
}

#[cfg(feature = "alloc")]
#[test]
fn armatron_linkage_works_with_both_storage_types() {
    // Define the armatron linkage with LinkageFixed
    const ARM_FIXED: LinkageFixed<6, 32> = LinkageFixed::start()
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
    let arm_buf = LinkageBuf::start()
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

    // Evaluate both with the same parameters
    let params = [0.7, 0.5, 0.2, 1.0, 0.6, 0.0];
    let fixed_view = ARM_FIXED.view();
    let buf_view = arm_buf.view();

    // Compare basic properties
    assert_eq!(fixed_view.dof(), buf_view.dof(), "DOF should match");
    assert_eq!(fixed_view.len(), buf_view.len(), "Step count should match");

    // Compare evaluation results
    let fixed_final = fixed_view.final_pose(&params);
    let buf_final = buf_view.final_pose(&params);

    assert!(
        fixed_final.position().is_close_to(&buf_final.position(), 1e-5),
        "Final pose position should match"
    );

    assert!(
        fixed_final.orientation().is_close_to(&buf_final.orientation(), 1e-5),
        "Final pose orientation should match"
    );

    // Count draw items
    let fixed_items: Vec<_> = fixed_view.draw_items(&params).collect();
    let buf_items: Vec<_> = buf_view.draw_items(&params).collect();
    assert_eq!(
        fixed_items.len(),
        buf_items.len(),
        "Number of draw items should match"
    );
}

#[cfg(feature = "alloc")]
#[test]
fn grid_9x9_linkage_works_with_both_storage_types() {
    // Simple test showing both types can instantiate the same expression
    const GRID_FIXED: LinkageFixed<0, 16> = LinkageFixed::start()
        .pen_color(Rgb888::new(0, 0, 255))
        .disk(1.0)
        .forward(2.0)
        .disk(1.0)
        .forward(2.0)
        .disk(1.0);

    let grid_buf = LinkageBuf::start()
        .pen_color(Rgb888::new(0, 0, 255))
        .disk(1.0)
        .forward(2.0)
        .disk(1.0)
        .forward(2.0)
        .disk(1.0);

    let params = [];
    let fixed_view = GRID_FIXED.view();
    let buf_view = grid_buf.view();

    assert_eq!(fixed_view.dof(), buf_view.dof());
    assert_eq!(fixed_view.len(), buf_view.len());

    // Both should produce draw items
    let fixed_items: Vec<_> = fixed_view.draw_items(&params).collect();
    let buf_items: Vec<_> = buf_view.draw_items(&params).collect();
    assert_eq!(fixed_items.len(), buf_items.len());
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
