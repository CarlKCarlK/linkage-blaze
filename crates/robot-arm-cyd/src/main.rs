#![no_std]
#![no_main]

// todo00 if flash read or write fails, do we want to panic or just require calibration each time?
// todo00 can/should there be a mode to share spi and cs pins?
// todo00 do we need/want any of these "Delay::new().delay_millis(1);"

use core::convert::Infallible;

use device_envoy_esp::{
    button::{Button as _, ButtonEsp, PressedTo},
    flash_block::{FlashBlock as _, FlashBlockEsp},
};
use embedded_graphics::{
    Drawable,
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Primitive, RgbColor, Size},
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
};
use esp_backtrace as _;
use esp_hal::{Config, delay::Delay, time::Instant};
use robot_arm_core::cyd::{CydSim, FrameBuffer, TouchInputEvent, TouchInputOutcome};

mod display;
mod touch;

use display::{CydDisplay, CydDisplayInitError};
use touch::{CydTouch, CydTouchInitError, RawTouchEvent};

esp_bootloader_esp_idf::esp_app_desc!();

#[derive(Clone, Copy)]
struct RawPoint {
    x: u16,
    y: u16,
}

#[derive(Clone, Copy)]
enum CydTouchEvent {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up,
}

impl CydTouchEvent {
    fn to_touch_input_event(self) -> TouchInputEvent {
        match self {
            CydTouchEvent::Down { x, y } => TouchInputEvent::Down { x, y },
            CydTouchEvent::Move { x, y } => TouchInputEvent::Move { x, y },
            CydTouchEvent::Up => TouchInputEvent::Up,
        }
    }
}

struct CydStatic {
    frame_buffer: &'static mut FrameBuffer,
}

// todo000 can we make this buffer part of the display? (may no longer apply)
struct Cyd {
    // todo000 combine with the display? (may no longer apply)
    display: CydDisplay,
    touch: CydTouch,
    calibration_config: Option<CalibrationConfig>,
    calibration_flash_block: FlashBlockEsp,
    calibration_button: ButtonEsp<'static>,
    frame_buffer: &'static mut FrameBuffer,
}

struct CalibratedCyd<'a> {
    cyd: &'a mut Cyd,
    calibration_config: CalibrationConfig,
}

struct RuntimeState {
    previous_tick: Instant,
    previous_frame_flush: Instant,
    should_flush: bool,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct CalibrationConfig {
    ax: f32,
    bx: f32,
    cx: f32,
    ay: f32,
    by: f32,
    cy: f32,
}

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
    DrawTouchCursor,
    FlushFrameBuffer,
}

impl From<device_envoy_esp::Error> for MainError {
    fn from(_error: device_envoy_esp::Error) -> Self {
        MainError::Flash
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

impl From<CydTouchInitError> for MainError {
    fn from(error: CydTouchInitError) -> Self {
        match error {
            CydTouchInitError::ConfigureTouchSpi => MainError::ConfigureTouchSpi,
            CydTouchInitError::CreateTouchSpiDevice => MainError::CreateTouchSpiDevice,
        }
    }
}

impl CydStatic {
    fn new() -> Self {
        Self {
            frame_buffer: FrameBuffer::static_new(),
        }
    }
}

impl Cyd {
    fn new(
        cyd_static: CydStatic,
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        touch_spi: impl esp_hal::spi::master::Instance + 'static,
        touch_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        touch_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        touch_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        touch_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        touch_irq_pin: impl esp_hal::gpio::InputPin + 'static,
        calibration_flash_block: FlashBlockEsp,
        calibration_button: ButtonEsp<'static>,
    ) -> Result<Cyd, MainError> {
        let mut calibration_flash_block = calibration_flash_block;
        let calibration_config = if calibration_button.is_pressed() {
            None
        } else {
            calibration_flash_block.load::<CalibrationConfig>()?
        };

        Ok(Cyd {
            display: CydDisplay::new(
                display_spi,
                display_sck_pin,
                display_mosi_pin,
                display_miso_pin,
                display_cs_pin,
                display_dc_pin,
                display_rst_pin,
                display_backlight_pin,
            )?,
            touch: CydTouch::new(
                touch_spi,
                touch_sck_pin,
                touch_mosi_pin,
                touch_miso_pin,
                touch_cs_pin,
                touch_irq_pin,
            )?,
            calibration_config,
            calibration_flash_block,
            calibration_button,
            frame_buffer: cyd_static.frame_buffer,
        })
    }

    fn recalibration_requested(&self) -> bool {
        self.calibration_button.is_pressed()
    }

    fn save_calibration(
        &mut self,
        calibration_config: &CalibrationConfig,
    ) -> Result<(), MainError> {
        Ok(self.calibration_flash_block.save(calibration_config)?)
    }

    fn flush(&mut self) -> Result<(), MainError> {
        flush_full_frame(
            self.display.display_mut(),
            self.frame_buffer,
            CydSim::WIDTH_U16,
            CydSim::HEIGHT_U16,
        )
    }

    fn calibrate(&mut self) -> Result<(), MainError> {
        let mut calibration_index = 0;
        let mut calibration_points = [RawPoint { x: 0, y: 0 }; 4];
        let mut calibration_screen_dirty = true;

        esp_println::println!("cal: tap corners in order UL -> UR -> LR -> LL");
        esp_println::println!("cal: next tap UL");

        loop {
            if calibration_screen_dirty {
                self.frame_buffer.clear(Rgb565::BLACK);
                if let Some(calibration_corner) = calibration_corner_for_index(calibration_index) {
                    draw_calibration_cross(
                        self.frame_buffer,
                        calibration_corner,
                        CydSim::WIDTH_U16,
                        CydSim::HEIGHT_U16,
                    )?;
                }
                self.flush()?;
                calibration_screen_dirty = false;
            }

            if self.recalibration_requested() {
                calibration_index = 0;
                calibration_points = [RawPoint { x: 0, y: 0 }; 4];
                calibration_screen_dirty = true;
                esp_println::println!("cal: calibration button pressed, restarting calibration");
                esp_println::println!("cal: next tap UL");
                continue;
            }

            if let Some(raw_touch_event) = self.touch.read_raw_touch_event() {
                if let RawTouchEvent::Down { raw_x, raw_y } = raw_touch_event {
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
                            calibration_screen_dirty = true;
                            continue;
                        }

                        let calibration_config = compute_calibration_four_point(
                            calibration_points,
                            CydSim::WIDTH_U16,
                            CydSim::HEIGHT_U16,
                        );
                        self.save_calibration(&calibration_config)?;

                        esp_println::println!(
                            "cal: done ax={:.5} bx={:.5} cx={:.1} ay={:.5} by={:.5} cy={:.1}",
                            calibration_config.ax,
                            calibration_config.bx,
                            calibration_config.cx,
                            calibration_config.ay,
                            calibration_config.by,
                            calibration_config.cy
                        );
                        esp_println::println!("cal: controls enabled with computed calibration");

                        self.calibration_config = Some(calibration_config);
                        return Ok(());
                    }
                }
            }

            Delay::new().delay_millis(1);
        }
    }

    fn ensure_calibration(&mut self) -> Result<(CalibratedCyd<'_>, bool), MainError> {
        let mut just_calibrated = false;

        if self.recalibration_requested() {
            self.calibration_config = None;
        }

        if self.calibration_config.is_none() {
            self.calibrate()?;
            just_calibrated = true;
        }

        let calibration_config = self
            .calibration_config
            .expect("ensure_calibration must leave Cyd calibrated");

        Ok((
            CalibratedCyd {
                cyd: self,
                calibration_config,
            },
            just_calibrated,
        ))
    }
}

impl CalibratedCyd<'_> {
    fn request_calibration(&mut self) {
        self.cyd.calibration_config = None;
    }

    fn read_touch_event(&mut self) -> Option<CydTouchEvent> {
        let raw_touch_event = self.cyd.touch.read_raw_touch_event()?;

        Some(match raw_touch_event {
            RawTouchEvent::Down { raw_x, raw_y } => {
                let (x, y) = map_raw_to_screen(
                    raw_x,
                    raw_y,
                    self.calibration_config,
                    CydSim::WIDTH_U16 as f32,
                    CydSim::HEIGHT_U16 as f32,
                );
                CydTouchEvent::Down { x, y }
            }
            RawTouchEvent::Move { raw_x, raw_y } => {
                let (x, y) = map_raw_to_screen(
                    raw_x,
                    raw_y,
                    self.calibration_config,
                    CydSim::WIDTH_U16 as f32,
                    CydSim::HEIGHT_U16 as f32,
                );
                CydTouchEvent::Move { x, y }
            }
            RawTouchEvent::Up => CydTouchEvent::Up,
        })
    }

    fn frame_buffer_mut(&mut self) -> &mut FrameBuffer {
        self.cyd.frame_buffer
    }

    fn flush(&mut self) -> Result<(), MainError> {
        self.cyd.flush()
    }
}

impl RuntimeState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            previous_tick: now,
            previous_frame_flush: now,
            should_flush: true,
        }
    }

    fn reset_after_calibration(&mut self) {
        let now = Instant::now();
        self.previous_tick = now;
        self.previous_frame_flush = now;
        self.should_flush = true;
    }

    fn tick_dt_seconds(&mut self) -> f32 {
        let now = Instant::now();
        let dt_seconds = (now - self.previous_tick).as_micros() as f32 / 1_000_000.0;
        self.previous_tick = now;
        dt_seconds
    }

    fn tick_dt_seconds_after_calibration(&mut self, just_calibrated: bool) -> f32 {
        if just_calibrated {
            // Reset timing after calibration so the next dt is measured from now.
            self.reset_after_calibration();
        }
        self.tick_dt_seconds()
    }

    fn frame_dt_seconds(&mut self) -> f32 {
        let now = Instant::now();
        let dt_seconds = (now - self.previous_frame_flush).as_micros() as f32 / 1_000_000.0;
        self.previous_frame_flush = now;
        dt_seconds
    }

    fn request_flush(&mut self) {
        self.should_flush = true;
    }

    fn clear_flush_request(&mut self) {
        self.should_flush = false;
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

    let mut cyd_sim = CydSim::new(); // todo000 review this

    let [calibration_flash_block] = FlashBlockEsp::new_array::<1>(p.FLASH)?;
    let calibration_button = ButtonEsp::new(p.GPIO0, PressedTo::Ground);

    let cyd_static = CydStatic::new();
    let mut cyd = Cyd::new(
        cyd_static,              // CYD static framebuffer storage
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

    let mut runtime_state = RuntimeState::new();

    loop {
        // Keep runtime gated on an active calibration; this may trigger the calibration flow.
        let (mut cyd, just_calibrated) = cyd.ensure_calibration()?;
        // If calibration just ran, reset timing and then advance simulation time.
        let dt_seconds = runtime_state.tick_dt_seconds_after_calibration(just_calibrated);

        // A kinematics update changed sim state, so schedule a frame flush.
        if cyd_sim.tick_reverse_kinematics(dt_seconds) {
            runtime_state.request_flush();
        }

        // Convert calibrated touch input into simulator interactions.
        if let Some(cyd_touch_event) = cyd.read_touch_event() {
            let touch_input_event = cyd_touch_event.to_touch_input_event();

            match cyd_sim.handle_touch_input_event(touch_input_event) {
                TouchInputOutcome::Unchanged => {}
                TouchInputOutcome::Changed => {
                    runtime_state.request_flush();
                }
                TouchInputOutcome::CalibrationRequested => {
                    esp_println::println!("cal: requested from UI");
                    cyd.request_calibration();
                    continue;
                }
            }
        }

        if runtime_state.should_flush {
            // Render only when state changed or animation advanced.
            let frame_dt_seconds = runtime_state.frame_dt_seconds();
            cyd_sim.set_frame_dt_seconds(frame_dt_seconds);

            cyd_sim.render_to(cyd.frame_buffer_mut());
            cyd.flush()?;
            runtime_state.clear_flush_request();
        } else {
            Delay::new().delay_millis(1);
        }
    }
}

fn solve_3x3(system_matrix: [[f32; 3]; 3], rhs_vector: [f32; 3]) -> (f32, f32, f32) {
    let determinant = system_matrix[0][0]
        * (system_matrix[1][1] * system_matrix[2][2] - system_matrix[1][2] * system_matrix[2][1])
        - system_matrix[0][1]
            * (system_matrix[1][0] * system_matrix[2][2]
                - system_matrix[1][2] * system_matrix[2][0])
        + system_matrix[0][2]
            * (system_matrix[1][0] * system_matrix[2][1]
                - system_matrix[1][1] * system_matrix[2][0]);

    assert!(
        determinant.abs() >= 0.000_001,
        "cal: invalid touch calibration geometry"
    );

    let determinant_ax = rhs_vector[0]
        * (system_matrix[1][1] * system_matrix[2][2] - system_matrix[1][2] * system_matrix[2][1])
        - system_matrix[0][1]
            * (rhs_vector[1] * system_matrix[2][2] - system_matrix[1][2] * rhs_vector[2])
        + system_matrix[0][2]
            * (rhs_vector[1] * system_matrix[2][1] - system_matrix[1][1] * rhs_vector[2]);

    let determinant_bx = system_matrix[0][0]
        * (rhs_vector[1] * system_matrix[2][2] - system_matrix[1][2] * rhs_vector[2])
        - rhs_vector[0]
            * (system_matrix[1][0] * system_matrix[2][2]
                - system_matrix[1][2] * system_matrix[2][0])
        + system_matrix[0][2]
            * (system_matrix[1][0] * rhs_vector[2] - rhs_vector[1] * system_matrix[2][0]);

    let determinant_cx = system_matrix[0][0]
        * (system_matrix[1][1] * rhs_vector[2] - rhs_vector[1] * system_matrix[2][1])
        - system_matrix[0][1]
            * (system_matrix[1][0] * rhs_vector[2] - rhs_vector[1] * system_matrix[2][0])
        + rhs_vector[0]
            * (system_matrix[1][0] * system_matrix[2][1]
                - system_matrix[1][1] * system_matrix[2][0]);

    (
        determinant_ax / determinant,
        determinant_bx / determinant,
        determinant_cx / determinant,
    )
}

fn solve_affine_axis(
    points: [RawPoint; 4],
    screen: [Point; 4],
    map_x_axis: bool,
) -> (f32, f32, f32) {
    let mut sum_xx = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_x = 0.0;
    let mut sum_yy = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xo = 0.0;
    let mut sum_yo = 0.0;
    let mut sum_o = 0.0;

    for sample_index in 0..4 {
        let raw_x = points[sample_index].x as f32;
        let raw_y = points[sample_index].y as f32;
        let output = if map_x_axis {
            screen[sample_index].x as f32
        } else {
            screen[sample_index].y as f32
        };

        sum_xx += raw_x * raw_x;
        sum_xy += raw_x * raw_y;
        sum_x += raw_x;
        sum_yy += raw_y * raw_y;
        sum_y += raw_y;
        sum_xo += raw_x * output;
        sum_yo += raw_y * output;
        sum_o += output;
    }

    let system_matrix = [
        [sum_xx, sum_xy, sum_x],
        [sum_xy, sum_yy, sum_y],
        [sum_x, sum_y, 4.0],
    ];
    let rhs_vector = [sum_xo, sum_yo, sum_o];
    solve_3x3(system_matrix, rhs_vector)
}

fn compute_calibration_four_point(
    points: [RawPoint; 4],
    width: u16,
    height: u16,
) -> CalibrationConfig {
    let ul = calibration_corner_center(CalibrationCorner::UpperLeft, width, height);
    let ur = calibration_corner_center(CalibrationCorner::UpperRight, width, height);
    let lr = calibration_corner_center(CalibrationCorner::LowerRight, width, height);
    let ll = calibration_corner_center(CalibrationCorner::LowerLeft, width, height);
    let screen_targets = [ul, ur, lr, ll];

    let (ax, bx, cx) = solve_affine_axis(points, screen_targets, true);
    let (ay, by, cy) = solve_affine_axis(points, screen_targets, false);

    CalibrationConfig {
        ax,
        bx,
        cx,
        ay,
        by,
        cy,
    }
}

fn map_raw_to_screen(
    raw_x: u16,
    raw_y: u16,
    calibration: CalibrationConfig,
    width: f32,
    height: f32,
) -> (f32, f32) {
    let raw_x = raw_x as f32;
    let raw_y = raw_y as f32;

    let mapped_x = calibration.ax * raw_x + calibration.bx * raw_y + calibration.cx;
    let mapped_y = calibration.ay * raw_x + calibration.by * raw_y + calibration.cy;

    let mapped_x = mapped_x.clamp(0.0, (width - 1.0).max(0.0));
    let mapped_y = mapped_y.clamp(0.0, (height - 1.0).max(0.0));

    (mapped_x, mapped_y)
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
    frame_buffer: &mut FrameBuffer,
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
        .draw(frame_buffer)
        .map_err(|_| MainError::DrawCalibrationCross)?;
    Line::new(top, bottom)
        .into_styled(CALIBRATION_CROSS_STYLE)
        .draw(frame_buffer)
        .map_err(|_| MainError::DrawCalibrationCross)?;

    Circle::new(
        Point::new(
            center.x - CALIBRATION_CENTER_DOT_RADIUS,
            center.y - CALIBRATION_CENTER_DOT_RADIUS,
        ),
        (CALIBRATION_CENTER_DOT_RADIUS * 2 + 1) as u32,
    )
    .into_styled(CALIBRATION_CENTER_DOT_STYLE)
    .draw(frame_buffer)
    .map_err(|_| MainError::DrawCalibrationCenterDot)?;

    Ok(())
}

fn flush_full_frame(
    display: &mut impl DrawTarget<Color = Rgb565>,
    frame_buffer: &FrameBuffer,
    width: u16,
    height: u16,
) -> Result<(), MainError> {
    let full_screen = Rectangle::new(Point::new(0, 0), Size::new(width as u32, height as u32));
    if display
        .fill_contiguous(&full_screen, frame_buffer.pixels().iter().copied())
        .is_err()
    {
        return Err(MainError::FlushFrameBuffer);
    }

    Ok(())
}
