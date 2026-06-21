/// Design decision: Dangling marks are allowed.
/// A mark that is never restored is valid - marks are optional restore points.
use linkage_blaze_core::LinkageFixed;

const DANGLING_MARKS: LinkageFixed<0, 2, 10> = LinkageFixed::start()
    .mark("checkpoint1")
    .forward(1.0)
    .mark("checkpoint2")
    .forward(2.0)
    // neither checkpoint1 nor checkpoint2 is restored — that's OK

;

fn main() {}
