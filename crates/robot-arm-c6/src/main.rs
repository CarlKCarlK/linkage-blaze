#![no_std]
#![no_main]

use core::convert::Infallible;

use embassy_time::Instant;
use embedded_graphics::Drawable;
use esp_backtrace as _;
use esp_hal::{
    Config,
    analog::adc::{Adc, AdcConfig, Attenuation},
    delay::Delay,
    gpio::{Input, InputConfig, Pull},
};
use robot_arm_core::cyd::{CydSim, TickOut};

mod display;

use display::{CydDisplay, CydDisplayFlushError, CydDisplayInitError};

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

#[derive(Debug)]
enum MainError {
    ConfigureDisplaySpi,
    CreateDisplaySpiDevice,
    InitDisplay,
    FlushFrameBuffer,
}

impl From<CydDisplayFlushError> for MainError {
    fn from(_: CydDisplayFlushError) -> Self {
        MainError::FlushFrameBuffer
    }
}

impl From<CydDisplayInitError> for MainError {
    fn from(error: CydDisplayInitError) -> Self {
        match error {
            CydDisplayInitError::ConfigureDisplaySpi => MainError::ConfigureDisplaySpi,
            CydDisplayInitError::CreateDisplaySpiDevice => MainError::CreateDisplaySpiDevice,
            CydDisplayInitError::InitDisplay => MainError::InitDisplay,
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
    let mut cyd_display = CydDisplay::new(
        p.SPI2, p.GPIO19, p.GPIO18, p.GPIO20, p.GPIO21, p.GPIO4, p.GPIO5, p.GPIO7,
    )?;
    esp_println::println!("c6: display initialized");

    let mut adc1_config = AdcConfig::new();
    let mut joystick_vrx = adc1_config.enable_pin(p.GPIO0, Attenuation::_11dB);
    let mut joystick_vry = adc1_config.enable_pin(p.GPIO1, Attenuation::_11dB);
    let mut adc1 = Adc::new(p.ADC1, adc1_config);
    let joystick_sw = Input::new(p.GPIO3, InputConfig::default().with_pull(Pull::Up));

    let mut cyd_sim = CydSim::new();
    let mut lower_arm_value = 0.5_f32;
    let mut spin_whole_value = 0.5_f32;
    let mut previous_loop_time = Instant::now();

    esp_println::println!(
        "c6: entering game loop with joystick velocity control (full-range in {:.1}s)",
        FULL_RANGE_SECONDS_AT_MAX_SPEED
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
        let sw_pressed = joystick_sw.is_low();

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

        let lower_arm_velocity = joystick_velocity_unit(vry01) * MAX_PARAM_SPEED_PER_SECOND;
        let spin_whole_velocity = joystick_velocity_unit(vrx01) * MAX_PARAM_SPEED_PER_SECOND;

        lower_arm_value = (lower_arm_value + lower_arm_velocity * dt_seconds).clamp(0.0, 1.0);
        spin_whole_value = (spin_whole_value + spin_whole_velocity * dt_seconds).clamp(0.0, 1.0);

        let joystick_changed =
            cyd_sim.set_lower_arm_and_spin_whole(lower_arm_value, spin_whole_value);

        esp_println::println!(
            "joy: vrx={} ({:.3}) vry={} ({:.3}) sw={} lower_arm={:.3} spin_whole={:.3}",
            vrx,
            vrx01,
            vry,
            vry01,
            if sw_pressed { "pressed" } else { "released" },
            lower_arm_value,
            spin_whole_value
        );

        match cyd_sim.tick(now, None) {
            TickOut::Draw => {
                match cyd_sim.draw(&mut cyd_display) {
                    Ok(()) => {}
                    Err(infallible) => match infallible {},
                }
                cyd_display.flush()?;
            }
            TickOut::Calibrate => {}
            TickOut::Nada => {
                if joystick_changed {
                    match cyd_sim.draw(&mut cyd_display) {
                        Ok(()) => {}
                        Err(infallible) => match infallible {},
                    }
                    cyd_display.flush()?;
                }
            }
        }

        Delay::new().delay_millis(50);
    }
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
