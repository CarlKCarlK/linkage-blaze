/// Design decision: A linkage must define exactly DOF parameters.
/// Accessing a parameter by index validates it was defined.
use linkage_blaze_core::LinkageFixed;

const LINKAGE: LinkageFixed<3, 0, 16> = LinkageFixed::start()
    .define_param("x", 0.5)
    .define_param("y", 0.5)
    .define_param("z", 0.5)
    .forward(1.0);

#[test]
fn dof_parameter_count() {
    assert_eq!(LINKAGE.dof(), 3);
    assert_eq!(LINKAGE.param_name(0), "x");
    assert_eq!(LINKAGE.param_name(1), "y");
    assert_eq!(LINKAGE.param_name(2), "z");
}

fn main() {}
