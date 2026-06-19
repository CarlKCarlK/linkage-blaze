/// Design decision: Restore requires the mark to be defined.
/// Attempting to restore a mark that was never marked is a compile error.
use linkage_blaze_core::Linkage;

const RESTORE_UNDEFINED: Linkage<0, 10> = Linkage::start()
    .forward(1.0)
    .restore("never_marked"); // error: no mark found with name

fn main() {}
