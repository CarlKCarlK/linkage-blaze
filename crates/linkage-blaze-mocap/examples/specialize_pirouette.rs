use std::{fs, process};

use linkage_blaze_core::{LinkageFixed, linkage, linkage_fixed};

const FULL: LinkageFixed<132, 4, 537> =
    linkage_fixed!("../samples/pirouette.lb.rs", 132, 4, 537);

const BODY: LinkageFixed<4, 4, 537> = FULL.retain_params(&[
    "head_yrotation",
    "abdomen_xrotation",
    "l_shldr_zrotation",
    "r_shldr_zrotation",
]);

fn main() {
    let out_path = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/pirouette_body.lb.rs");
    let lb_rs = BODY.view().to_lb_rs();
    if let Err(error) = fs::write(out_path, &lb_rs) {
        eprintln!("failed to write `{out_path}`: {error}");
        process::exit(1);
    }
    println!("wrote {out_path} ({} DOF)", BODY.view().dof());
}
