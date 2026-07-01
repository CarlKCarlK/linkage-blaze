//! Parked "controlled knob" highlighting code.
//!
//! This example's game loop used to paint two knobs green to show which
//! params a joystick was driving (see `linkage-blaze-armatron-c6`'s
//! `JoystickControlMode`). This crate has no joystick, so the highlighting
//! was always static and never toggled — it was removed from
//! `armatron/main.rs` to stop the yellow-vs-green distinction from implying
//! knobs are interactive/ignored based on joystick state that doesn't exist
//! here. Kept in case a future input source (joystick, encoder, etc.) wants
//! the same highlighting.
//!
//! To restore: reintroduce a `controlled_knobs: [ControlledKnob; 2]` value
//! threaded through `armatron`, `draw_armatron`, and `draw_sliders`, and swap
//! the plain `fill_style(SIM_YELLOW)` knob draws back to
//! `knob_fill_style(&controlled_knobs, ControlledKnob::Param(...))`.

#![allow(dead_code)]

use embedded_graphics::{pixelcolor::Rgb565, primitives::PrimitiveStyle};

use super::{GREEN, SIM_YELLOW, fill_style};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlledKnob {
    Param(usize),
}

fn knob_fill_style(
    controlled_knobs: &[ControlledKnob; 2],
    knob: ControlledKnob,
) -> PrimitiveStyle<Rgb565> {
    if controlled_knobs[0] == knob || controlled_knobs[1] == knob {
        fill_style(GREEN)
    } else {
        fill_style(SIM_YELLOW)
    }
}
