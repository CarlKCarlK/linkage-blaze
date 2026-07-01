#![no_std]
#![no_main]

// todo00 can/should there be a mode to share spi and cs pins?
// todo00 do we need/want any of these "Delay::new().delay_millis(1);"

use core::convert::Infallible;

use device_envoy_esp::{
    button::{ButtonEsp, PressedTo},
    flash_block::FlashBlockEsp,
    init_and_start,
};
use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::delay::Delay;

use linkage_blaze_cyd::{
    CalibrationConfig, CydDevice as _, CydError, CydEsp, CydStaticEsp, DEFAULT_FONT, Orientation,
    RawPoint, RawTouchEvent, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use linkage_blaze_example_core::armatron::{
    ArmatronPlatform, BLACK, ControlledKnob, DOF, WHITE, armatron, calibration_corner_for_index,
    draw_armatron, draw_calibration_cross,
};
use log::info;

esp_bootloader_esp_idf::esp_app_desc!();

#[derive(Debug)]
enum MainError {
    Flash,
    ConfigureDisplaySpi,
    CreateDisplaySpiDevice,
    ConfigureTouchSpi,
    CreateTouchSpiDevice,
    InitDisplay,
    DrawCalibrationCross,
    FlushFrameBuffer,
}

impl From<device_envoy_esp::Error> for MainError {
    fn from(_error: device_envoy_esp::Error) -> Self {
        MainError::Flash
    }
}

impl From<CydError> for MainError {
    fn from(error: CydError) -> Self {
        match error {
            CydError::Flash(_) => MainError::Flash,
            CydError::DisplayInit(error) => match error {
                linkage_blaze_cyd::CydDisplayEspInitError::ConfigureDisplaySpi => {
                    MainError::ConfigureDisplaySpi
                }
                linkage_blaze_cyd::CydDisplayEspInitError::CreateDisplaySpiDevice => {
                    MainError::CreateDisplaySpiDevice
                }
                linkage_blaze_cyd::CydDisplayEspInitError::InitDisplay => MainError::InitDisplay,
            },
            CydError::TouchInit(error) => match error {
                linkage_blaze_cyd::CydTouchEspInitError::ConfigureTouchSpi => {
                    MainError::ConfigureTouchSpi
                }
                linkage_blaze_cyd::CydTouchEspInitError::CreateTouchSpiDevice => {
                    MainError::CreateTouchSpiDevice
                }
            },
            CydError::DisplayFlush(_) => MainError::FlushFrameBuffer,
            CydError::TouchUnavailable => unreachable!("touch always available when calibrated"),
            CydError::CalibrationUnavailable => {
                unreachable!("calibration always present after ensure_calibrated")
            }
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let err = inner_main(spawner).await.unwrap_err();
    panic!("{err:?}");
}

async fn inner_main(_spawner: Spawner) -> Result<Infallible, MainError> {
    init_and_start!(p);
    esp_println::logger::init_logger(log::LevelFilter::Info);
    info!("Starting CYD armatron loop");

    let [calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let calibration_button = ButtonEsp::new(p.GPIO0, PressedTo::Ground);

    static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
    let mut cyd = CydEsp::new(
        &CYD_STATIC,
        p.SPI2,   // display SPI
        p.GPIO14, // display SCK
        p.GPIO13, // display MOSI
        p.GPIO12, // display MISO
        p.GPIO15, // display CS
        p.GPIO2,  // display DC
        p.GPIO4,  // display reset
        p.GPIO21, // display backlight
        Orientation::Landscape,
        BLACK,                   // default background
        WHITE,                   // default foreground
        &DEFAULT_FONT,           // default font
        p.SPI3,                  // touch SPI
        p.GPIO25,                // touch SCK
        p.GPIO32,                // touch MOSI
        p.GPIO39,                // touch MISO
        p.GPIO33,                // touch CS
        p.GPIO36,                // touch IRQ
        calibration_flash_block, // calibration flash block
        calibration_button,      // calibration button
    )?;
    info!("CYD display and touch initialized");

    let mut platform = EspPlatform;
    armatron(&mut cyd, &mut platform)
}

struct EspPlatform;

impl ArmatronPlatform for EspPlatform {
    type CydDevice = CydEsp;
    type Error = MainError;

    fn ensure_calibrated(&mut self, cyd: &mut CydEsp) -> Result<(), MainError> {
        ensure_calibration(cyd)
    }

    fn remove_calibration(&mut self, cyd: &mut CydEsp) {
        cyd.remove_calibration();
    }

    fn draw_and_flush(
        &mut self,
        cyd: &mut CydEsp,
        params: &[f32; DOF],
        target_seed: u8,
        reverse_kinematics_playing: bool,
        show_fps: bool,
        fps: Option<u32>,
        touch_cursor: Option<(f32, f32)>,
        controlled_knobs: &[ControlledKnob; 2],
    ) -> Result<(), MainError> {
        let mut frame = cyd.full_frame_mut();
        match draw_armatron(
            &mut frame,
            params,
            target_seed,
            reverse_kinematics_playing,
            show_fps,
            fps,
            touch_cursor,
            controlled_knobs,
        ) {
            Ok(()) => {}
            Err(infallible) => match infallible {},
        }
        Ok(frame.flush()?)
    }
}

fn ensure_calibration(cyd: &mut CydEsp) -> Result<(), MainError> {
    if cyd.recalibration_requested() {
        cyd.remove_calibration();
    }
    if cyd.calibration_config().is_none() {
        calibrate(cyd)?;
    }
    Ok(())
}

fn calibrate(cyd: &mut CydEsp) -> Result<(), MainError> {
    let mut calibration_index = 0;
    let mut calibration_points = [RawPoint { x: 0, y: 0 }; 4];

    esp_println::println!("cal: tap corners in order UL -> UR -> LR -> LL");
    esp_println::println!("cal: next tap UL");
    draw_calibration_screen(cyd, calibration_index)?;

    loop {
        if cyd.recalibration_requested() {
            calibration_index = 0;
            calibration_points = [RawPoint { x: 0, y: 0 }; 4];
            esp_println::println!("cal: calibration button pressed, restarting calibration");
            esp_println::println!("cal: next tap UL");
            draw_calibration_screen(cyd, calibration_index)?;
            continue;
        }

        if let Some(RawTouchEvent::Down { raw_x, raw_y }) = cyd.read_raw_touch_event() {
            if calibration_index < 4 {
                calibration_points[calibration_index] = RawPoint { x: raw_x, y: raw_y };
                calibration_index += 1;
                esp_println::println!(
                    "cal: point{} raw_x={} raw_y={}",
                    calibration_index,
                    raw_x,
                    raw_y
                );
                if calibration_index < 4 {
                    let corner_label = ["UL", "UR", "LR", "LL"][calibration_index];
                    esp_println::println!("cal: next tap {}", corner_label);
                    draw_calibration_screen(cyd, calibration_index)?;
                    continue;
                }

                let calibration_config = CalibrationConfig::from_four_points(calibration_points);
                cyd.save_calibration(calibration_config)?;
                esp_println::println!("cal: controls enabled with computed calibration");
                return Ok(());
            }
        }

        Delay::new().delay_millis(1);
    }
}

fn draw_calibration_screen(cyd: &mut CydEsp, calibration_index: usize) -> Result<(), MainError> {
    let mut frame = cyd.full_frame_mut();
    frame.fill(CydEsp::rgb565(BLACK));
    if let Some(calibration_corner) = calibration_corner_for_index(calibration_index) {
        draw_calibration_cross(
            &mut frame,
            calibration_corner,
            SCREEN_WIDTH as u16,
            SCREEN_HEIGHT as u16,
        )
        .map_err(|_| MainError::DrawCalibrationCross)?;
    }
    Ok(frame.flush()?)
}
