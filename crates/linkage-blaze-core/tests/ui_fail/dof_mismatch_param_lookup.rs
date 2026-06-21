/// Design decision: Accessing undefined parameters panics at compile time.
/// A parametric step must reference a defined parameter.
use linkage_blaze_core::LinkageFixed;

const LINKAGE: LinkageFixed<3, 0, 8> = LinkageFixed::start()
    .define_param("x", 0.5)
    .define_param("y", 0.5)
    // Missing third parameter definition
    .forward_param("z", 0.0, 1.0);  // error: unknown parameter name

fn main() {}
