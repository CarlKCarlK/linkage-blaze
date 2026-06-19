/// Design decision: Mark must be defined before restore.
/// Attempting to restore a mark that will be defined later is a compile error.
use linkage_blaze_core::Linkage;

const RESTORE_BEFORE_MARK: Linkage<0, 10> = Linkage::start()
    .restore("wrist") // error: mark must be defined before restore
    .forward(1.0)
    .mark("wrist");

fn main() {}
