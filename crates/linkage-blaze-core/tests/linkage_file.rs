use linkage_blaze_core::{LinkageFixed, linkage, linkage_file, linkage_view};

linkage_file! {
    #[derive(Debug)]
    RepeatedMarks {
        file: "linkages/repeated_marks.lb.rs",
    }
    MultipleParams {
        file: "linkages/multiple_params.lb.rs",
    }
}

const REPEATED_MARKS: LinkageFixed<0, 1, 5> = RepeatedMarks::fixed();
const MULTIPLE_PARAMS: LinkageFixed<2, 0, 3> = MultipleParams::fixed();
const REPEATED_MARKS_VIEW: linkage_blaze_core::LinkageView<'static, 0, 1> =
    linkage_view!(RepeatedMarks::fixed());

#[test]
fn derives_exact_file_metadata() {
    assert_eq!(RepeatedMarks::DOF, 0);
    assert_eq!(RepeatedMarks::MARKS, 1);
    assert_eq!(RepeatedMarks::STEP_COUNT, 5);
    assert_eq!(MultipleParams::DOF, 2);
    assert_eq!(MultipleParams::MARKS, 0);
    assert_eq!(MultipleParams::STEP_COUNT, 3);
    assert_eq!(REPEATED_MARKS_VIEW.len(), 5);
    assert_eq!(REPEATED_MARKS.view().mark_names(), &["origin"]);
    assert_eq!(MULTIPLE_PARAMS.view().dof(), 2);
}

#[cfg(feature = "alloc")]
#[test]
fn fixed_and_buf_use_the_same_file_body() -> Result<(), linkage_blaze_core::Error> {
    let fixed = RepeatedMarks::fixed();
    let buffered = RepeatedMarks::buf();
    assert_eq!(fixed.view().len(), buffered.view().len());
    assert_eq!(fixed.view().mark_names(), buffered.view().mark_names());
    assert!(
        fixed
            .view()
            .final_pose(&[])?
            .position()
            .is_close_to(&buffered.view().final_pose(&[])?.position(), 1e-5)
    );
    Ok(())
}

fn main() {}
