use std::{fs, process};

use linkage_blaze_core::{LinkageFixed, linkage, linkage_fixed};

const FULL: LinkageFixed<132, 6, 538> = linkage_fixed!("../samples/pirouette.lb.rs");

const BODY: LinkageFixed<4, 6, 538> = FULL
    .freeze_param_name::<131>("l_shin_yrotation", 57.6)
    .retain_param_names(&[
        "head_yrotation",
        "abdomen_xrotation",
        "l_shldr_zrotation",
        "r_shldr_zrotation",
    ]);

const BODY_OPTIMIZED: LinkageFixed<4, 6, 382> = BODY
    .strip_fixed_noops::<382>()
    .merge_adjacent_fixed::<382>()
    .strip_fixed_noops::<382>();

fn main() {
    let out_path = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/pirouette_body.lb.rs");
    let lb_rs = BODY_OPTIMIZED.view().to_lb_rs();
    if let Err(error) = fs::write(out_path, &lb_rs) {
        eprintln!("failed to write `{out_path}`: {error}");
        process::exit(1);
    }
    println!(
        "wrote {out_path} ({} DOF, {} steps)",
        BODY_OPTIMIZED.view().dof(),
        BODY_OPTIMIZED.view().len(),
    );
}
