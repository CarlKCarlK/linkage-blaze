use linkage_blaze_core::{LinkageFixed, linkage, linkage_fixed};

// Pirouette BVH sample: 132 DOF (one per motion-capture channel), 4 mark slots,
// 537 steps.  The path crosses into the mocap crate's samples directory.
const PIROUETTE: LinkageFixed<132, 4, 537> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs", 132, 4, 537);

// Freeze l_shin_yrotation first (DOF 132 → 131), then retain the four joints
// of interest (DOF 131 → 4).  Retained param order follows the original linkage:
//   0: abdomen_xrotation  (defined first among the four)
//   1: head_yrotation
//   2: r_shldr_zrotation
//   3: l_shldr_zrotation
const PIROUETTE_BODY: LinkageFixed<4, 4, 537> = PIROUETTE
    .freeze_param_normalized::<131>("l_shin_yrotation", 0.54)
    .retain_params(&[
        "head_yrotation",
        "abdomen_xrotation",
        "l_shldr_zrotation",
        "r_shldr_zrotation",
    ]);

#[test]
fn pirouette_body_only_has_4_dof() {
    assert_eq!(PIROUETTE_BODY.view().dof(), 4);
}

#[test]
fn pirouette_body_param_names_follow_original_linkage_order() {
    let params = PIROUETTE_BODY.view().params();
    assert_eq!(params[0].name(), "abdomen_xrotation");
    assert_eq!(params[1].name(), "head_yrotation");
    assert_eq!(params[2].name(), "r_shldr_zrotation");
    assert_eq!(params[3].name(), "l_shldr_zrotation");
}
