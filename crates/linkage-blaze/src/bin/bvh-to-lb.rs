use std::{env, fs, process};

use linkage_blaze::bvh::bvh_to_lb_rs;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "bvh-to-lb".to_string());
    let first_argument = args.next();
    if matches!(first_argument.as_deref(), Some("-h" | "--help")) {
        println!("Usage: {program} <input.bvh> <output.lb.rs> [joint...]");
        println!();
        println!("Convert a BVH motion-capture file to a Linkage Blaze source file.");
        return Ok(());
    }
    let input_path = first_argument
        .ok_or_else(|| format!("usage: {program} <input.bvh> <output.lb.rs> [joint...]"))?;
    let output_path = args
        .next()
        .ok_or_else(|| format!("usage: {program} <input.bvh> <output.lb.rs> [joint...]"))?;
    let mark_joints: Vec<String> = args.collect();
    let mark_joints: Vec<&str> = mark_joints.iter().map(String::as_str).collect();

    let source = fs::read_to_string(&input_path)
        .map_err(|error| format!("failed to read `{input_path}`: {error}"))?;
    let lb_rs = bvh_to_lb_rs::<256, 64>(&source, &mark_joints)
        .map_err(|error| format!("failed to convert `{input_path}`: {error}"))?;
    fs::write(&output_path, lb_rs)
        .map_err(|error| format!("failed to write `{output_path}`: {error}"))?;

    Ok(())
}
