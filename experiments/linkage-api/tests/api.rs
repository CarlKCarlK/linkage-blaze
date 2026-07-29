use linkage_api_experiment::{LinkageView, Step, linkage, linkage_combine};

const LEFT: LinkageView<'static, 1, 0> = linkage! {
    dof: 1,
    marks: 0;
    .forward(10.0)
    .yaw(30.0)
};

const RIGHT: LinkageView<'static, 2, 1> = linkage! {
    dof: 2,
    marks: 1;
    .forward(5.0)
};

const TAIL: LinkageView<'static, 0, 1> = linkage! {
    dof: 0,
    marks: 1;
    .yaw(-15.0)
};

const COMBINED: LinkageView<'static, 3, 1> = linkage_combine!(LEFT, RIGHT);
const VARIADIC: LinkageView<'static, 3, 2> = linkage_combine!(LEFT, RIGHT, TAIL);

#[test]
fn fluent_linkage_returns_a_promoted_view_without_capacity_in_its_type() {
    assert_eq!(
        LEFT.steps(),
        &[Step::Start, Step::Forward(10.0), Step::Yaw(30.0)]
    );
}

#[test]
fn combine_takes_views_and_returns_a_view() {
    assert_eq!(COMBINED.dof(), 3);
    assert_eq!(COMBINED.marks(), 1);
    assert_eq!(
        COMBINED.steps(),
        &[
            Step::Start,
            Step::Forward(10.0),
            Step::Yaw(30.0),
            Step::Forward(5.0),
        ]
    );
}

#[test]
fn variadic_combine_is_left_associative_and_skips_later_start_steps() {
    assert_eq!(
        VARIADIC.steps(),
        &[
            Step::Start,
            Step::Forward(10.0),
            Step::Yaw(30.0),
            Step::Forward(5.0),
            Step::Yaw(-15.0),
        ]
    );
}
