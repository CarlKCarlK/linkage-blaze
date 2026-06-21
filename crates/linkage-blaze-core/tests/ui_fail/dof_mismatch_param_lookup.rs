/// Design decision: Accessing undefined parameters panics at compile time.
/// If dof() > param_len(), accessing parameters beyond param_len() is an error.
use linkage_blaze_core::LinkageFixed;

const LINKAGE: LinkageFixed<3, 0, 8> = LinkageFixed::start()
    .define_param("x", 0.5)
    .define_param("y", 0.5)
    // Missing third parameter definition
    .forward(1.0)
    .param(2);  // error: parameter index must be defined

fn main() {}
