#![no_std]
#![no_main]

// todo00 can/should there be a mode to share spi and cs pins?
// todo00 do we need/want any of these "Delay::new().delay_millis(1);"

use core::convert::Infallible;

use device_envoy_esp::{
    button::{ButtonEsp, PressedTo},
    flash_block::FlashBlockEsp,
};
use embassy_time::Instant;
use embedded_graphics::{
    Drawable,
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Primitive, RgbColor},
    primitives::{Circle, Line, PrimitiveStyle},
};
use esp_backtrace as _;
use esp_hal::{Config, delay::Delay};
use static_cell::StaticCell;

use cyd_esp32::{
    CalibratedCyd, CalibrationConfig, Cyd, CydError, RawPoint, RawTouchEvent, RectBuffer,
    SCREEN_HEIGHT, SCREEN_WIDTH, TouchInputEvent as CydTouchInputEvent,
};
use robot_arm_core::cyd::{CydSim, TickOut, TouchInputEvent};

esp_bootloader_esp_idf::esp_app_desc!();

#[derive(Clone, Copy)]
enum CalibrationCorner {
    UpperLeft,
    UpperRight,
    LowerRight,
    LowerLeft,
}

const CALIBRATION_CROSS_MARGIN: i32 = 28;
const CALIBRATION_CROSS_HALF_SIZE: i32 = 18;
const CALIBRATION_CENTER_DOT_RADIUS: i32 = 3;
const CALIBRATION_CROSS_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_stroke(Rgb565::YELLOW, 4);
const CALIBRATION_CENTER_DOT_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_fill(Rgb565::WHITE);
const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

type ScreenBuffer = RectBuffer<SCREEN_WIDTH, SCREEN_HEIGHT, SCREEN_PIXELS>;

#[derive(Debug)]
enum MainError {
    Flash,
    ConfigureDisplaySpi,
    CreateDisplaySpiDevice,
    ConfigureTouchSpi,
    CreateTouchSpiDevice,
    InitDisplay,
    DrawCalibrationCross,
    DrawCalibrationCenterDot,
    FlushFrameBuffer,
    TouchUnavailable,
    CalibrationUnavailable,
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
                cyd_esp32::CydDisplayInitError::ConfigureDisplaySpi => {
                    MainError::ConfigureDisplaySpi
                }
                cyd_esp32::CydDisplayInitError::CreateDisplaySpiDevice => {
                    MainError::CreateDisplaySpiDevice
                }
                cyd_esp32::CydDisplayInitError::InitDisplay => MainError::InitDisplay,
            },
            CydError::TouchInit(error) => match error {
                cyd_esp32::CydTouchInitError::ConfigureTouchSpi => MainError::ConfigureTouchSpi,
                cyd_esp32::CydTouchInitError::CreateTouchSpiDevice => {
                    MainError::CreateTouchSpiDevice
                }
            },
            CydError::DisplayFlush(_) => MainError::FlushFrameBuffer,
            CydError::TouchUnavailable => MainError::TouchUnavailable,
            CydError::CalibrationUnavailable => MainError::CalibrationUnavailable,
        }
    }
}

#[esp_hal::main]
fn main() -> ! {
    let err = inner_main().unwrap_err();
    panic!("{err:?}");
}

fn inner_main() -> Result<Infallible, MainError> {
    let p = esp_hal::init(Config::default());
    esp_println::logger::init_logger(log::LevelFilter::Info);

    let [calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let calibration_button = ButtonEsp::new(p.GPIO0, PressedTo::Ground);

    // todo0000 make nicer
    static SCREEN_BUFFER: StaticCell<ScreenBuffer> = StaticCell::new();
    let screen_buffer = ScreenBuffer::init_static(&SCREEN_BUFFER);

    let mut cyd = Cyd::new_with_touch(
        p.SPI2,                  // display SPI
        p.GPIO14,                // display SCK
        p.GPIO13,                // display MOSI
        p.GPIO12,                // display MISO
        p.GPIO15,                // display CS
        p.GPIO2,                 // display DC
        p.GPIO4,                 // display reset
        p.GPIO21,                // display backlight
        p.SPI3,                  // touch SPI
        p.GPIO25,                // touch SCK
        p.GPIO32,                // touch MOSI
        p.GPIO39,                // touch MISO
        p.GPIO33,                // touch CS
        p.GPIO36,                // touch IRQ
        calibration_flash_block, // calibration flash block
        calibration_button,      // calibration button
    )?;

    let mut cyd_sim = CydSim::new(); // or CydSim::new_with_fps() for benchmarking
    loop {
        // Keep runtime gated on an active calibration; this may trigger the calibration flow.
        let mut cyd = ensure_calibration(&mut cyd, screen_buffer)?;

        match cyd_sim.tick(Instant::now(), read_touch_input(&mut cyd)?) {
            // 1_886_000 fps if only command
            TickOut::Calibrate => cyd.remove_calibration(),
            TickOut::Draw => {
                // todo0000 make nicer
                draw(screen_buffer, &cyd_sim); // 32.3 fps if only command
                cyd.flush(screen_buffer, Point::new(0, 0))?; // 13.2 fps if only command
            }
            TickOut::Nada => {}
        }
    }
}

fn ensure_calibration<'a>(
    cyd: &'a mut Cyd,
    screen_buffer: &mut ScreenBuffer,
) -> Result<CalibratedCyd<'a>, MainError> {
    if cyd.recalibration_requested() {
        cyd.remove_calibration();
    }

    if cyd.calibration_config().is_none() {
        calibrate(cyd, screen_buffer)?;
    }

    Ok(cyd.ensure_calibration()?)
}

fn calibrate(cyd: &mut Cyd, screen_buffer: &mut ScreenBuffer) -> Result<(), MainError> {
    let mut calibration_index = 0;
    let mut calibration_points = [RawPoint { x: 0, y: 0 }; 4];

    esp_println::println!("cal: tap corners in order UL -> UR -> LR -> LL");
    esp_println::println!("cal: next tap UL");
    draw_calibration_screen(cyd, screen_buffer, calibration_index)?;

    loop {
        if cyd.recalibration_requested() {
            calibration_index = 0;
            calibration_points = [RawPoint { x: 0, y: 0 }; 4];
            esp_println::println!("cal: calibration button pressed, restarting calibration");
            esp_println::println!("cal: next tap UL");
            draw_calibration_screen(cyd, screen_buffer, calibration_index)?;
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
                    draw_calibration_screen(cyd, screen_buffer, calibration_index)?;
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

fn draw_calibration_screen(
    cyd: &mut Cyd,
    screen_buffer: &mut ScreenBuffer,
    calibration_index: usize,
) -> Result<(), MainError> {
    screen_buffer.clear(Rgb565::BLACK);
    if let Some(calibration_corner) = calibration_corner_for_index(calibration_index) {
        draw_calibration_cross(
            screen_buffer,
            calibration_corner,
            CydSim::WIDTH_U16,
            CydSim::HEIGHT_U16,
        )?;
    }
    Ok(cyd.flush(screen_buffer, Point::new(0, 0))?)
}

fn read_touch_input(cyd: &mut CalibratedCyd<'_>) -> Result<Option<TouchInputEvent>, MainError> {
    Ok(cyd
        .read_touch_input()?
        .map(|touch_input_event| match touch_input_event {
            CydTouchInputEvent::Down { x, y } => TouchInputEvent::Down { x, y },
            CydTouchInputEvent::Move { x, y } => TouchInputEvent::Move { x, y },
            CydTouchInputEvent::Up => TouchInputEvent::Up,
        }))
}

fn draw(
    screen_buffer: &mut ScreenBuffer,
    drawable: &impl embedded_graphics::Drawable<Color = Rgb565, Output = ()>,
) {
    match drawable.draw(screen_buffer) {
        Ok(()) => {}
        Err(infallible) => match infallible {},
    }
}

fn calibration_corner_for_index(calibration_index: usize) -> Option<CalibrationCorner> {
    match calibration_index {
        0 => Some(CalibrationCorner::UpperLeft),
        1 => Some(CalibrationCorner::UpperRight),
        2 => Some(CalibrationCorner::LowerRight),
        3 => Some(CalibrationCorner::LowerLeft),
        _ => None,
    }
}

fn calibration_corner_center(
    calibration_corner: CalibrationCorner,
    width: u16,
    height: u16,
) -> Point {
    let width = width as i32;
    let height = height as i32;
    match calibration_corner {
        CalibrationCorner::UpperLeft => {
            Point::new(CALIBRATION_CROSS_MARGIN, CALIBRATION_CROSS_MARGIN)
        }
        CalibrationCorner::UpperRight => Point::new(
            width - 1 - CALIBRATION_CROSS_MARGIN,
            CALIBRATION_CROSS_MARGIN,
        ),
        CalibrationCorner::LowerRight => Point::new(
            width - 1 - CALIBRATION_CROSS_MARGIN,
            height - 1 - CALIBRATION_CROSS_MARGIN,
        ),
        CalibrationCorner::LowerLeft => Point::new(
            CALIBRATION_CROSS_MARGIN,
            height - 1 - CALIBRATION_CROSS_MARGIN,
        ),
    }
}

fn draw_calibration_cross(
    target: &mut impl DrawTarget<Color = Rgb565>,
    calibration_corner: CalibrationCorner,
    width: u16,
    height: u16,
) -> Result<(), MainError> {
    let center = calibration_corner_center(calibration_corner, width, height);
    let left = Point::new(center.x - CALIBRATION_CROSS_HALF_SIZE, center.y);
    let right = Point::new(center.x + CALIBRATION_CROSS_HALF_SIZE, center.y);
    let top = Point::new(center.x, center.y - CALIBRATION_CROSS_HALF_SIZE);
    let bottom = Point::new(center.x, center.y + CALIBRATION_CROSS_HALF_SIZE);

    Line::new(left, right)
        .into_styled(CALIBRATION_CROSS_STYLE)
        .draw(target)
        .map_err(|_| MainError::DrawCalibrationCross)?;
    Line::new(top, bottom)
        .into_styled(CALIBRATION_CROSS_STYLE)
        .draw(target)
        .map_err(|_| MainError::DrawCalibrationCross)?;

    Circle::new(
        Point::new(
            center.x - CALIBRATION_CENTER_DOT_RADIUS,
            center.y - CALIBRATION_CENTER_DOT_RADIUS,
        ),
        (CALIBRATION_CENTER_DOT_RADIUS * 2 + 1) as u32,
    )
    .into_styled(CALIBRATION_CENTER_DOT_STYLE)
    .draw(target)
    .map_err(|_| MainError::DrawCalibrationCenterDot)?;

    Ok(())
}
