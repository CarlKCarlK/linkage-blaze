use linkage_blaze_core::LinkageFixed;

const LINKAGE: LinkageFixed<2, 0, 4> = LinkageFixed::start()
    .define_param("Bruce", 0.25)
    .define_param("Bruce", 0.75)
    .yaw_param("Bruce", -90.0, 90.0);

const BAD: LinkageFixed<1, 0, 4> = LINKAGE.freeze_param_name("Bruce", 0.0);

fn main() {
    let _ = BAD;
}
