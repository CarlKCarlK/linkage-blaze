/// Design decision: A linkage must define exactly DOF parameters.
/// param_len() should always equal dof() after construction.
use linkage_blaze_core::LinkageFixed;

const LINKAGE: LinkageFixed<3, 16> = LinkageFixed::start()
    .define_param("x", 0.5)
    .define_param("y", 0.5)
    .define_param("z", 0.5)
    .forward(1.0);

#[test]
fn dof_equals_param_len() {
    assert_eq!(LINKAGE.dof(), LINKAGE.param_len());
    assert_eq!(LINKAGE.dof(), 3);
    assert_eq!(LINKAGE.param_len(), 3);
}

fn main() {}
