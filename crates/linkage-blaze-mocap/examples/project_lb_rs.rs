use std::{env, fs, process};

use linkage_blaze_core::{DrawItem3d, LinkageBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "project_lb_rs".to_string());
    let input_path = args
        .next()
        .ok_or_else(|| format!("usage: {program} <input.lb.rs>"))?;
    if args.next().is_some() {
        return Err(format!("usage: {program} <input.lb.rs>"));
    }

    let source = fs::read_to_string(&input_path)
        .map_err(|error| format!("failed to read `{input_path}`: {error}"))?;
    let linkage = LinkageBuf::<256, 64>::from_lb_rs(&source)
        .map_err(|error| format!("failed to parse `{input_path}`: {error}"))?;
    let mut params = [0.5; 256];
    for (param_index, param) in linkage.view().params().iter().enumerate() {
        params[param_index] = param.default();
    }

    for (segment_index, draw_item) in linkage.view().draw_items(&params).enumerate() {
        let DrawItem3d::Stroke(segment) = draw_item else {
            continue;
        };
        let start = segment.start().position();
        let end = segment.end().position();
        println!(
            "{segment_index:02}: start=({:8.3}, {:8.3}, {:8.3}) end=({:8.3}, {:8.3}, {:8.3})",
            start[0], start[1], start[2], end[0], end[1], end[2]
        );
    }

    Ok(())
}
