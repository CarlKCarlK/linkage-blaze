use std::{fs, process};

use linkage_blaze_core::{LinkageFixed, linkage, linkage_fixed};

const FULL: LinkageFixed<132, 4, 537> = linkage_fixed!("../samples/pirouette.lb.rs");

const DANCE_UNOPTIMIZED: LinkageFixed<3, 4, 537> = FULL
    .freeze_param_name::<131>("l_shin_yrotation", 57.6)
    .freeze_param_name_at_default::<130>("abdomen_xrotation")
    .retain_param_names(&["head_yrotation", "l_shldr_zrotation", "r_shldr_zrotation"]);

fn main() {
    let out_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../linkage-blaze-dance-classic/src/dance.lb.rs"
    );
    let result = linkage_blaze_core::LinkageBuf::<3, 4>::from(&DANCE_UNOPTIMIZED)
        .strip_fixed_noops()
        .merge_adjacent_fixed()
        .strip_fixed_noops();
    let lb_rs = result.view().to_lb_rs();
    if let Err(error) = fs::write(out_path, &lb_rs) {
        eprintln!("failed to write `{out_path}`: {error}");
        process::exit(1);
    }
    println!(
        "wrote {out_path} ({} DOF, {} steps)",
        result.view().dof(),
        result.view().len(),
    );
}
