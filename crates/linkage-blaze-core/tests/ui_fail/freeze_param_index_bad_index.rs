use linkage_blaze_core::LinkageFixed;

const LINKAGE: LinkageFixed<1, 0, 4> = LinkageFixed::start()
    .define_param("angle", 0.5)
    .yaw_param("angle", -90.0, 90.0);

const BAD: LinkageFixed<0, 0, 4> = LINKAGE.freeze_param_index(9, 0.0);

fn main() {
    let _ = BAD;
}
