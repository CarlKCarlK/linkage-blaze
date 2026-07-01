// Helper function to compare LinkageFixed and LinkageBuf behavior.
// Both should produce identical results given the same parameters.

use linkage_blaze_core::Linkage;

pub fn assert_linkages_equivalent<const DOF: usize, const MARKS: usize>(
    linkage_fixed: &impl Linkage<DOF, MARKS>,
    linkage_buf: &impl Linkage<DOF, MARKS>,
    params: &[f32; DOF],
) {
    // Compare basic properties
    assert_eq!(
        linkage_fixed.view().dof(),
        linkage_buf.view().dof(),
        "DOF should match"
    );
    assert_eq!(
        linkage_fixed.view().len(),
        linkage_buf.view().len(),
        "Step count should match"
    );

    // Compare evaluation results
    let fixed_final = linkage_fixed.view().final_pose(params);
    let buf_final = linkage_buf.view().final_pose(params);

    assert!(
        fixed_final
            .position()
            .is_close_to(&buf_final.position(), 1e-5),
        "Final pose position should match: fixed={:?}, buf={:?}",
        fixed_final.position(),
        buf_final.position()
    );

    assert!(
        fixed_final
            .orientation()
            .is_close_to(&buf_final.orientation(), 1e-5),
        "Final pose orientation should match"
    );

    // Count draw items - both should produce the same number
    let fixed_items: Vec<_> = linkage_fixed.view().draw_items_3d(params).collect();
    let buf_items: Vec<_> = linkage_buf.view().draw_items_3d(params).collect();
    assert_eq!(
        fixed_items.len(),
        buf_items.len(),
        "Number of draw items should match"
    );
}
