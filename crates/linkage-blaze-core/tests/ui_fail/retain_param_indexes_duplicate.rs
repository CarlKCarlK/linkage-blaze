use linkage_blaze_core::LinkageFixed;

const LINKAGE: LinkageFixed<2, 0, 5> = LinkageFixed::start()
    .define_param("angle", 0.5)
    .define_param("dist", 0.5)
    .yaw_param("angle", -90.0, 90.0)
    .forward_param("dist", 0.0, 10.0);

const BAD: LinkageFixed<2, 0, 5> = LINKAGE.retain_param_indexes(&[0, 0]);

fn main() {
    let _ = BAD;
}
