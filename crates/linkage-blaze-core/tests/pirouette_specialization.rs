#[cfg(feature = "alloc")]
use linkage_blaze_core::LinkageBuf;
use linkage_blaze_core::{DrawItem, LinkageFixed, Pose, Vec3, linkage, linkage_fixed};

// Pirouette BVH sample: 132 DOF (one per motion-capture channel), 4 mark slots,
// 537 steps.  The path crosses into the mocap crate's samples directory.
const PIROUETTE: LinkageFixed<132, 4, 537> =
    linkage_fixed!("../../linkage-blaze-mocap/samples/pirouette.lb.rs");

// Freeze l_shin_yrotation first (DOF 132 → 131), then retain the four joints
// of interest (DOF 131 → 4).  Retained param order follows the original linkage:
//   0: abdomen_xrotation  (defined first among the four)
//   1: head_yrotation
//   2: r_shldr_zrotation
//   3: l_shldr_zrotation
const PIROUETTE_BODY: LinkageFixed<4, 4, 537> = PIROUETTE
    .freeze_param_name::<131>("l_shin_yrotation", 57.6)
    .retain_param_names(&[
        "head_yrotation",
        "abdomen_xrotation",
        "l_shldr_zrotation",
        "r_shldr_zrotation",
    ]);

// Full peephole pipeline in const: strip zeros, merge adjacent same-type fixed
// steps.  N shrinks from 537 → 381; merge is a no-op here (no adjacent same-type
// fixed steps remain after stripping), but is included to demonstrate the pipeline.
// This must evaluate identically to PIROUETTE_BODY at every parameter value.
const PIROUETTE_BODY_OPT: LinkageFixed<4, 4, 381> = PIROUETTE_BODY
    .strip_fixed_noops::<381>()
    .merge_adjacent_fixed::<381>();

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

#[test]
fn pirouette_body_evaluates_without_alloc_storage() {
    let params = [0.5, 0.5, 0.5, 0.5];
    let view = PIROUETTE_BODY.view();
    let pose = view.final_pose(&params);
    assert_pose_finite(pose);

    let mut item_count = 0;
    for draw_item in view.draw_items(&params) {
        item_count += 1;
        match draw_item {
            DrawItem::Stroke(stroke_segment) => {
                assert_pose_finite(stroke_segment.start());
                assert_pose_finite(stroke_segment.end());
            }
            DrawItem::Disk(disk_item) => {
                assert_pose_finite(disk_item.pose());
                assert!(disk_item.radius().is_finite());
            }
            DrawItem::Ring(ring_item) => {
                assert_pose_finite(ring_item.pose());
                assert!(ring_item.radius().is_finite());
            }
            DrawItem::Sphere(sphere_item) => {
                assert_pose_finite(sphere_item.pose());
                assert!(sphere_item.radius().is_finite());
            }
        }
    }

    assert!(item_count > 0);
}

#[test]
fn pirouette_body_matches_full_linkage_with_frozen_defaults() {
    let body_params = [0.62, 0.37, 0.81, 0.18];
    let mut full_params = full_pirouette_defaults();
    let full_view = PIROUETTE.view();

    full_params[full_view.param_index("l_shin_yrotation", 0)] = 0.54;
    full_params[full_view.param_index("abdomen_xrotation", 0)] = body_params[0];
    full_params[full_view.param_index("head_yrotation", 0)] = body_params[1];
    full_params[full_view.param_index("r_shldr_zrotation", 0)] = body_params[2];
    full_params[full_view.param_index("l_shldr_zrotation", 0)] = body_params[3];

    let body_view = PIROUETTE_BODY.view();
    assert_pose_close(
        full_view.final_pose(&full_params),
        body_view.final_pose(&body_params),
        1e-3,
    );

    let mut full_items = full_view.draw_items(&full_params);
    let mut body_items = body_view.draw_items(&body_params);
    let mut item_count = 0;
    loop {
        match (full_items.next(), body_items.next()) {
            (Some(full_item), Some(body_item)) => {
                item_count += 1;
                assert_draw_item_close(full_item, body_item, 1e-3);
            }
            (None, None) => break,
            (Some(_), None) => panic!("specialized linkage emitted fewer draw items"),
            (None, Some(_)) => panic!("specialized linkage emitted more draw items"),
        }
    }

    assert!(item_count > 0);
}

#[test]
fn pirouette_body_optimized_matches_original() {
    // PIROUETTE_BODY_OPT has fewer steps but must evaluate identically to
    // PIROUETTE_BODY at every input.
    let full = PIROUETTE_BODY.view();
    let opt = PIROUETTE_BODY_OPT.view();

    assert_eq!(opt.dof(), full.dof());

    for params in [
        [0.0, 0.0, 0.0, 0.0_f32],
        [0.5, 0.5, 0.5, 0.5],
        [1.0, 1.0, 1.0, 1.0],
        [0.25, 0.75, 0.1, 0.9],
    ] {
        assert_pose_close(full.final_pose(&params), opt.final_pose(&params), 1e-4);

        let mut n = 0;
        for (u, o) in full.draw_items(&params).zip(opt.draw_items(&params)) {
            assert_draw_item_close(u, o, 1e-4);
            n += 1;
        }
        assert!(n > 0);
    }
}

#[cfg(feature = "alloc")]
#[test]
fn pirouette_body_const_opt_matches_buf_opt() {
    // Build the same pipeline through LinkageBuf and verify identical evaluation.
    let buf_result = LinkageBuf::<4, 4>::from(&PIROUETTE_BODY)
        .strip_fixed_noops()
        .merge_adjacent_fixed()
        .strip_fixed_noops();

    assert_eq!(buf_result.view().len(), PIROUETTE_BODY_OPT.view().len());

    for params in [
        [0.0, 0.0, 0.0, 0.0_f32],
        [0.5, 0.5, 0.5, 0.5],
        [1.0, 1.0, 1.0, 1.0],
        [0.62, 0.37, 0.81, 0.18],
    ] {
        assert_pose_close(
            PIROUETTE_BODY_OPT.view().final_pose(&params),
            buf_result.view().final_pose(&params),
            1e-4,
        );
    }
}

#[cfg(feature = "alloc")]
#[test]
fn pirouette_fixed_and_buf_freeze_retain_produce_same_result() {
    // Load the full pirouette as a LinkageBuf, apply the same freeze+retain
    // pipeline as the const PIROUETTE_BODY, and verify the two paths agree.
    let buf_body = LinkageBuf::<132, 4>::from(&PIROUETTE)
        .freeze_param_name::<131>("l_shin_yrotation", 57.6)
        .retain_param_names::<4>(&[
            "head_yrotation",
            "abdomen_xrotation",
            "l_shldr_zrotation",
            "r_shldr_zrotation",
        ]);

    assert_eq!(buf_body.view().dof(), PIROUETTE_BODY.view().dof());
    assert_eq!(buf_body.view().len(), PIROUETTE_BODY.view().len());

    for params in [
        [0.0, 0.0, 0.0, 0.0_f32],
        [0.5, 0.5, 0.5, 0.5],
        [1.0, 1.0, 1.0, 1.0],
        [0.62, 0.37, 0.81, 0.18],
    ] {
        assert_pose_close(
            PIROUETTE_BODY.view().final_pose(&params),
            buf_body.view().final_pose(&params),
            1e-4,
        );
    }
}

fn full_pirouette_defaults() -> [f32; 132] {
    let view = PIROUETTE.view();
    let params = view.params();
    let mut values = [0.0; 132];
    for (param_index, param) in params.iter().enumerate() {
        values[param_index] = param.default();
    }
    values
}

fn assert_draw_item_close(left: DrawItem, right: DrawItem, tolerance: f32) {
    match (left, right) {
        (DrawItem::Stroke(left), DrawItem::Stroke(right)) => {
            assert_pose_close(left.start(), right.start(), tolerance);
            assert_pose_close(left.end(), right.end(), tolerance);
            assert!((left.width() - right.width()).abs() <= tolerance);
        }
        (DrawItem::Disk(left), DrawItem::Disk(right)) => {
            assert_pose_close(left.pose(), right.pose(), tolerance);
            assert!((left.radius() - right.radius()).abs() <= tolerance);
        }
        (DrawItem::Ring(left), DrawItem::Ring(right)) => {
            assert_pose_close(left.pose(), right.pose(), tolerance);
            assert!((left.radius() - right.radius()).abs() <= tolerance);
            assert!((left.width() - right.width()).abs() <= tolerance);
        }
        (DrawItem::Sphere(left), DrawItem::Sphere(right)) => {
            assert_pose_close(left.pose(), right.pose(), tolerance);
            assert!((left.radius() - right.radius()).abs() <= tolerance);
        }
        _ => panic!("draw item variants differ"),
    }
}

fn assert_pose_close(left: Pose, right: Pose, tolerance: f32) {
    assert!(left.position().is_close_to(&right.position(), tolerance));
    assert!(
        left.orientation()
            .is_close_to(&right.orientation(), tolerance)
    );
}

fn assert_pose_finite(pose: Pose) {
    assert_vec_finite(pose.position());
    for row in pose.orientation().as_array() {
        for value in row {
            assert!(value.is_finite());
        }
    }
}

fn assert_vec_finite(vec3: Vec3) {
    for value in vec3.as_array() {
        assert!(value.is_finite());
    }
}
