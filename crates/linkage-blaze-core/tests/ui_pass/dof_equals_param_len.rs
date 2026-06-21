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
    let view = LINKAGE.view();
    assert_eq!(view.param(0).name(), "x");
    assert_eq!(view.param(1).name(), "y");
    assert_eq!(view.param(2).name(), "z");
}

fn main() {}
