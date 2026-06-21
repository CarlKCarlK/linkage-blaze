use std::{env, fs, process};

use linkage_blaze_mocap::bvh_to_lb_rs;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "bvh_to_lb_rs".to_string());
    let input_path = args
        .next()
        .ok_or_else(|| format!("usage: {program} <input.bvh> <output.lb.rs>"))?;
    let output_path = args
        .next()
        .ok_or_else(|| format!("usage: {program} <input.bvh> <output.lb.rs>"))?;
    if args.next().is_some() {
        return Err(format!("usage: {program} <input.bvh> <output.lb.rs>"));
    }

    let source = fs::read_to_string(&input_path)
        .map_err(|error| format!("failed to read `{input_path}`: {error}"))?;
    let lb_rs = bvh_to_lb_rs::<256, 64>(&source)
        .map_err(|error| format!("failed to convert `{input_path}`: {error}"))?;
    fs::write(&output_path, lb_rs)
        .map_err(|error| format!("failed to write `{output_path}`: {error}"))?;

    Ok(())
}
