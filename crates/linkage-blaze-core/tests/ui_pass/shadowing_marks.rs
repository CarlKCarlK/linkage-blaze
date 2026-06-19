/// Design decision: Shadowing marks within a single linkage.
/// Redefining a mark updates which position .restore() uses (last-definition-wins).
use linkage_blaze_core::Linkage;

const SHADOWING_MARKS: Linkage<0, 10> = Linkage::start()
    .mark("wrist")
    .forward(1.0)
    .mark("wrist") // allowed: redefine the mark
    .forward(2.0)
    .restore("wrist"); // uses the 2nd mark("wrist"), not the 1st

fn main() {}
