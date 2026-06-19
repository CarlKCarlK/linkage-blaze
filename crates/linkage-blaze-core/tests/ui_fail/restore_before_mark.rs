/// Design decision: Mark must be defined before restore.
/// Attempting to restore a mark that will be defined later is a compile error.
use linkage_blaze_core::LinkageFixed;

const RESTORE_BEFORE_MARK: LinkageFixed<0, 10> = LinkageFixed::start()
    .restore("wrist") // error: mark must be defined before restore
    .forward(1.0)
    .mark("wrist");

fn main() {}
