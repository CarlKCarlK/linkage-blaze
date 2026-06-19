/// Design decision: N must be large enough to store all mark names.
/// Exceeding the capacity N for marks is a compile error.
use linkage_blaze_core::Linkage;

const N_TOO_SMALL: Linkage<0, 3> = Linkage::start()
    .mark("mark1")
    .forward(1.0)
    .mark("mark2")
    .forward(1.0)
    .mark("mark3")
    .forward(1.0)
    .mark("mark4"); // error: linkage has more marks than N

fn main() {}
