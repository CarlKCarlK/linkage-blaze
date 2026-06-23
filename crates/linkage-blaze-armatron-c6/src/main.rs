#![no_std]
#![no_main]

// todo00 issues: touch not connected
// todo00 need cyd mode with just one spi (and switch between 2 uses and 2 freq)
// todo00 merge all common parts of this and esp32 classic version
// todo00 vscode doesn't show right rust analyzer concerns
// todo00 in general, a good cyd display device abstraction needs to have orientation (and gamma?)
// todo00 in a perfect world, those controls on the cyd-sim (sliders, etc) would be modular.

use core::convert::Infallible;

use device_envoy_esp::button::{Button as _, ButtonEsp, PressedTo};
use embassy_time::Instant;
use esp_backtrace as _;
use esp_hal::{
    Config,
    analog::adc::{Adc, AdcConfig, Attenuation},
    delay::Delay,
};
use linkage_blaze_armatron_core::{ControlledKnob, CydSim, TickOut};
use linkage_blaze_cyd::{
    Cyd, CydDisplayConfig, CydError, CydStatic, PixelBuffer, RectBuffer, SCREEN_HEIGHT,
    SCREEN_WIDTH,
};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();

const JOYSTICK_VRX_MIN: u16 = 2160;
const JOYSTICK_VRX_CENTER: u16 = 3827;
const JOYSTICK_VRX_MAX: u16 = 4095;
const JOYSTICK_VRY_MIN: u16 = 2160;
const JOYSTICK_VRY_CENTER: u16 = 3741;
const JOYSTICK_VRY_MAX: u16 = 4095;
const JOYSTICK_DEADZONE: f32 = 0.03;
const FULL_RANGE_SECONDS_AT_MAX_SPEED: f32 = 5.0;
const MAX_PARAM_SPEED_PER_SECOND: f32 = 1.0 / FULL_RANGE_SECONDS_AT_MAX_SPEED;
const PARAM_BASE_YAW: &str = "x/y view";
const PARAM_BASE_PITCH: &str = "z";
const PARAM_RAISE_HAND: &str = "raise hand";
const PARAM_BEND_ELBOW: &str = "bend elbow";
const PARAM_HAND_WIDTH: &str = "close hand";
const PARAM_LOWER_ARM: &str = "lower arm";
const PARAM_SPIN_WHOLE_ARM: &str = "spin whole";
const PARAM_SPIN_HAND: &str = "spin hand";
const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

type ScreenBuffer = RectBuffer<SCREEN_WIDTH, SCREEN_HEIGHT, SCREEN_PIXELS>;

#[derive(Clone, Copy, Debug)]
enum JoystickControlMode {
    HandAndElbow,
    WholeSpinAndLowerArm,
    HandOpenAndWristSpin,
    View,
}

impl JoystickControlMode {
    fn next(self) -> Self {
        match self {
            JoystickControlMode::HandAndElbow => JoystickControlMode::WholeSpinAndLowerArm,
            JoystickControlMode::WholeSpinAndLowerArm => JoystickControlMode::HandOpenAndWristSpin,
            JoystickControlMode::HandOpenAndWristSpin => JoystickControlMode::View,
            JoystickControlMode::View => JoystickControlMode::HandAndElbow,
        }
    }

    fn label(self) -> &'static str {
        match self {
            JoystickControlMode::HandAndElbow => "hand + elbow",
            JoystickControlMode::WholeSpinAndLowerArm => "whole_spin + lower_arm",
            JoystickControlMode::HandOpenAndWristSpin => "hand_open + wrist_spin",
            JoystickControlMode::View => "view",
        }
    }

    fn highlighted_param_names(self) -> (&'static str, &'static str) {
        match self {
            JoystickControlMode::HandAndElbow => (PARAM_RAISE_HAND, PARAM_BEND_ELBOW),
            JoystickControlMode::WholeSpinAndLowerArm => (PARAM_SPIN_WHOLE_ARM, PARAM_LOWER_ARM),
            JoystickControlMode::HandOpenAndWristSpin => (PARAM_HAND_WIDTH, PARAM_SPIN_HAND),
            JoystickControlMode::View => (PARAM_BASE_PITCH, PARAM_BASE_YAW),
        }
    }
}

#[derive(Debug)]
enum MainError {
    ConfigureDisplaySpi,
    CreateDisplaySpiDevice,
    InitDisplay,
    FlushFrameBuffer,
}

impl From<CydError> for MainError {
    fn from(error: CydError) -> Self {
        match error {
            CydError::DisplayInit(error) => match error {
                linkage_blaze_cyd::CydDisplayInitError::ConfigureDisplaySpi => {
                    MainError::ConfigureDisplaySpi
                }
                linkage_blaze_cyd::CydDisplayInitError::CreateDisplaySpiDevice => {
                    MainError::CreateDisplaySpiDevice
                }
                linkage_blaze_cyd::CydDisplayInitError::InitDisplay => MainError::InitDisplay,
            },
            CydError::DisplayFlush(_) => MainError::FlushFrameBuffer,
            _ => MainError::FlushFrameBuffer,
        }
    }
}

#[esp_hal::main]
fn main() -> ! {
    let err = inner_main().unwrap_err();
    panic!("{err:?}");
}

fn inner_main() -> Result<Infallible, MainError> {
    esp_println::println!("c6: booting");
    let p = esp_hal::init(Config::default());
    {
        let timg0 = esp_hal::timer::timg::TimerGroup::new(p.TIMG0);
        let sw = esp_hal::interrupt::software::SoftwareInterruptControl::new(p.SW_INTERRUPT);
        esp_rtos::start(timg0.timer0, sw.software_interrupt0);
    }
    esp_println::logger::init_logger(log::LevelFilter::Info);
    esp_println::println!("c6: esp_hal init complete");

    esp_println::println!("c6: initializing display");
    // todo00 unify: this app draws into its own full-screen ScreenBuffer, so the
    // Cyd-owned buffer is zero-sized. Look at rendering into the single Cyd-owned
    // buffer via cyd.draw_frame or cyd.frame_mut instead.
    static CYD_STATIC: CydStatic<PixelBuffer<0>> = CydStatic::new();
    let mut cyd = Cyd::new_display_only(
        &CYD_STATIC,
        p.SPI2,
        p.GPIO19,
        p.GPIO18,
        p.GPIO20,
        p.GPIO21,
        p.GPIO4,
        p.GPIO5,
        p.GPIO7,
        CydDisplayConfig::LANDSCAPE,
    )?;
    static SCREEN_BUFFER: StaticCell<ScreenBuffer> = StaticCell::new();
    let screen_buffer = ScreenBuffer::init_static(&SCREEN_BUFFER);
    esp_println::println!("c6: display initialized");

    let mut adc1_config = AdcConfig::new();
    let mut joystick_vrx = adc1_config.enable_pin(p.GPIO0, Attenuation::_11dB);
    let mut joystick_vry = adc1_config.enable_pin(p.GPIO1, Attenuation::_11dB);
    let mut adc1 = Adc::new(p.ADC1, adc1_config);
    let joystick_button = ButtonEsp::new(p.GPIO3, PressedTo::Ground);

    let mut cyd_sim = CydSim::new();
    let raise_hand_param = cyd_sim_param_index(PARAM_RAISE_HAND);
    let bend_elbow_param = cyd_sim_param_index(PARAM_BEND_ELBOW);
    let hand_width_param = cyd_sim_param_index(PARAM_HAND_WIDTH);
    let lower_arm_param = cyd_sim_param_index(PARAM_LOWER_ARM);
    let spin_whole_arm_param = cyd_sim_param_index(PARAM_SPIN_WHOLE_ARM);
    let spin_hand_param = cyd_sim_param_index(PARAM_SPIN_HAND);
    let base_pitch_param = cyd_sim_param_index(PARAM_BASE_PITCH);
    let base_yaw_param = cyd_sim_param_index(PARAM_BASE_YAW);
    let mut raise_hand_value = CydSim::param_default(raise_hand_param);
    let mut bend_elbow_value = CydSim::param_default(bend_elbow_param);
    let mut hand_width_value = CydSim::param_default(hand_width_param);
    let mut lower_arm_value = CydSim::param_default(lower_arm_param);
    let mut spin_whole_arm_value = CydSim::param_default(spin_whole_arm_param);
    let mut spin_hand_value = CydSim::param_default(spin_hand_param);
    let mut z_mix_value = CydSim::param_default(base_pitch_param);
    let mut xy_mix_value = CydSim::param_default(base_yaw_param);
    let mut control_mode = JoystickControlMode::HandAndElbow;
    let (first_knob, second_knob) = highlighted_knobs(control_mode);
    cyd_sim.set_controlled_knobs(first_knob, second_knob);
    let mut sw_was_pressed = false;
    let mut previous_loop_time = Instant::now();

    esp_println::println!(
        "c6: entering game loop with joystick velocity control (full-range in {:.1}s), mode={}",
        FULL_RANGE_SECONDS_AT_MAX_SPEED,
        control_mode.label()
    );
    loop {
        let now = Instant::now();
        let dt_seconds = now
            .saturating_duration_since(previous_loop_time)
            .as_micros() as f32
            / 1_000_000.0;
        previous_loop_time = now;

        let vrx = loop {
            if let Ok(value) = adc1.read_oneshot(&mut joystick_vrx) {
                break value;
            }
        };
        let vry = loop {
            if let Ok(value) = adc1.read_oneshot(&mut joystick_vry) {
                break value;
            }
        };
        let sw_pressed = joystick_button.is_pressed();
        let mut control_mode_changed = false;
        if sw_pressed && !sw_was_pressed {
            control_mode = control_mode.next();
            let (first_knob, second_knob) = highlighted_knobs(control_mode);
            cyd_sim.set_controlled_knobs(first_knob, second_knob);
            control_mode_changed = true;
            esp_println::println!("joy: mode -> {}", control_mode.label());
        }
        sw_was_pressed = sw_pressed;

        let vrx01 = normalize_joystick_centered(
            vrx,
            JOYSTICK_VRX_MIN,
            JOYSTICK_VRX_CENTER,
            JOYSTICK_VRX_MAX,
        );
        let vry01 = normalize_joystick_centered(
            vry,
            JOYSTICK_VRY_MIN,
            JOYSTICK_VRY_CENTER,
            JOYSTICK_VRY_MAX,
        );

        let joystick_x_velocity = joystick_velocity_unit(vrx01) * MAX_PARAM_SPEED_PER_SECOND;
        let joystick_y_velocity = joystick_velocity_unit(vry01) * MAX_PARAM_SPEED_PER_SECOND;

        let joystick_changed = match control_mode {
            JoystickControlMode::HandAndElbow => {
                // Up lowers the hand; left bends the elbow counter-clockwise.
                raise_hand_value =
                    (raise_hand_value - joystick_y_velocity * dt_seconds).clamp(0.0, 1.0);
                bend_elbow_value =
                    (bend_elbow_value + joystick_x_velocity * dt_seconds).clamp(0.0, 1.0);
                cyd_sim.set_param_pair(
                    raise_hand_param,
                    raise_hand_value,
                    bend_elbow_param,
                    bend_elbow_value,
                )
            }
            JoystickControlMode::WholeSpinAndLowerArm => {
                // Left spins the whole arm counter-clockwise; up lowers the whole arm.
                spin_whole_arm_value =
                    (spin_whole_arm_value + joystick_x_velocity * dt_seconds).clamp(0.0, 1.0);
                lower_arm_value =
                    (lower_arm_value - joystick_y_velocity * dt_seconds).clamp(0.0, 1.0);
                cyd_sim.set_param_pair(
                    spin_whole_arm_param,
                    spin_whole_arm_value,
                    lower_arm_param,
                    lower_arm_value,
                )
            }
            JoystickControlMode::HandOpenAndWristSpin => {
                // Left opens the hand; down spins the wrist counter-clockwise.
                hand_width_value =
                    (hand_width_value + joystick_x_velocity * dt_seconds).clamp(0.0, 1.0);
                spin_hand_value =
                    (spin_hand_value - joystick_y_velocity * dt_seconds).clamp(0.0, 1.0);
                cyd_sim.set_param_pair(
                    hand_width_param,
                    hand_width_value,
                    spin_hand_param,
                    spin_hand_value,
                )
            }
            JoystickControlMode::View => {
                // Up moves the view up; left decreases x/y view.
                z_mix_value = (z_mix_value - joystick_y_velocity * dt_seconds).clamp(0.0, 1.0);
                xy_mix_value = (xy_mix_value + joystick_x_velocity * dt_seconds).clamp(0.0, 1.0);
                cyd_sim.set_view_mixes(z_mix_value, xy_mix_value)
            }
        };

        esp_println::println!(
            "joy: mode={} vrx={} ({:.3}) vry={} ({:.3}) sw={} raise_hand={:.3} bend_elbow={:.3} close_hand={:.3} lower_arm={:.3} spin_whole={:.3} spin_hand={:.3} z={:.3} xy={:.3}",
            control_mode.label(),
            vrx,
            vrx01,
            vry,
            vry01,
            if sw_pressed { "pressed" } else { "released" },
            raise_hand_value,
            bend_elbow_value,
            hand_width_value,
            lower_arm_value,
            spin_whole_arm_value,
            spin_hand_value,
            z_mix_value,
            xy_mix_value
        );

        match cyd_sim.tick(now, None) {
            TickOut::Draw => {
                draw(screen_buffer, &cyd_sim);
                cyd.flush(screen_buffer, embedded_graphics::prelude::Point::new(0, 0))?;
            }
            TickOut::Calibrate => {}
            TickOut::Nada => {
                if control_mode_changed || joystick_changed {
                    draw(screen_buffer, &cyd_sim);
                    cyd.flush(screen_buffer, embedded_graphics::prelude::Point::new(0, 0))?;
                }
            }
        }

        Delay::new().delay_millis(50);
    }
}

fn draw(
    screen_buffer: &mut ScreenBuffer,
    drawable: &impl embedded_graphics::Drawable<
        Color = embedded_graphics::pixelcolor::Rgb565,
        Output = (),
    >,
) {
    match drawable.draw(screen_buffer) {
        Ok(()) => {}
        Err(infallible) => match infallible {},
    }
}

fn cyd_sim_param_index(name: &str) -> usize {
    CydSim::param_index(name).expect("CydSim parameter must exist")
}

fn highlighted_knobs(control_mode: JoystickControlMode) -> (ControlledKnob, ControlledKnob) {
    let (first_name, second_name) = control_mode.highlighted_param_names();
    (
        ControlledKnob::Param(cyd_sim_param_index(first_name)),
        ControlledKnob::Param(cyd_sim_param_index(second_name)),
    )
}

fn normalize_joystick_centered(raw: u16, min: u16, center: u16, max: u16) -> f32 {
    if !(min < center && center < max) {
        return 0.0;
    }

    if raw <= min {
        0.0
    } else if raw < center {
        let below_center = (raw - min) as f32 / (center - min) as f32;
        0.5 * below_center
    } else if raw == center {
        0.5
    } else if raw >= max {
        1.0
    } else {
        let above_center = (raw - center) as f32 / (max - center) as f32;
        0.5 + 0.5 * above_center
    }
}

fn joystick_velocity_unit(normalized_centered: f32) -> f32 {
    let signed = (normalized_centered.clamp(0.0, 1.0) - 0.5) * 2.0;
    let magnitude = signed.abs();

    if magnitude <= JOYSTICK_DEADZONE {
        0.0
    } else {
        let scaled = (magnitude - JOYSTICK_DEADZONE) / (1.0 - JOYSTICK_DEADZONE);
        signed.signum() * scaled
    }
}
