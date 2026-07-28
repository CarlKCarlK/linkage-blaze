use linkage_blaze_core::linkage_file;

linkage_file! {
    repeated_marks {
        file: "linkages/repeated_marks.lb.rs",
    }
}
linkage_file! {
    multiple_params {
        file: "linkages/multiple_params.lb.rs",
    }
}

const REPEATED_MARKS: repeated_marks::Fixed = repeated_marks::fixed();
const MULTIPLE_PARAMS: multiple_params::Fixed = multiple_params::fixed();
const REPEATED_MARKS_VIEW: repeated_marks::View = repeated_marks::view();

#[test]
fn derives_exact_file_metadata() {
    assert_eq!(repeated_marks::DOF, 0);
    assert_eq!(repeated_marks::MARKS, 1);
    assert_eq!(repeated_marks::STEP_COUNT, 5);
    assert_eq!(multiple_params::DOF, 2);
    assert_eq!(multiple_params::MARKS, 0);
    assert_eq!(multiple_params::STEP_COUNT, 3);
    assert_eq!(REPEATED_MARKS_VIEW.len(), 5);
    assert_eq!(REPEATED_MARKS.view().mark_names(), &["origin"]);
    assert_eq!(MULTIPLE_PARAMS.view().dof(), 2);
}

#[cfg(feature = "alloc")]
#[test]
fn fixed_and_buf_use_the_same_file_body() -> Result<(), linkage_blaze_core::Error> {
    let fixed = repeated_marks::fixed();
    let buffered = repeated_marks::buf();
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
