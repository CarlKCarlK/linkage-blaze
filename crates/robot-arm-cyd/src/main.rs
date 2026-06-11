#![no_std]
#![no_main]

use core::cell::RefCell;

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
use embedded_hal_bus::spi::RefCellDevice;
use esp_backtrace as _;
use esp_hal::{
    Config,
    delay::Delay,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    spi,
    time::Instant,
};
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
};
use robot_arm_core::cyd::{CydSim, FrameBuffer};
use static_cell::StaticCell;

mod touch;

use touch::{RawTouchEvent, Xpt2046TouchInput};

esp_bootloader_esp_idf::esp_app_desc!();

static CYD_SIM: StaticCell<CydSim> = StaticCell::new();
static FRAME_BUFFER: StaticCell<FrameBuffer> = StaticCell::new();

const DISPLAY_SPI_HZ: u32 = 60_000_000;
const DISPLAY_INTERFACE_BUFFER_BYTES: usize = 4096;
const FRAME_PROFILE_LOGGING: bool = false;
const TOUCH_LOGGING: bool = false;

static DISPLAY_INTERFACE_BUFFER: StaticCell<[u8; DISPLAY_INTERFACE_BUFFER_BYTES]> =
    StaticCell::new();

#[derive(Clone, Copy)]
struct RawPoint {
    x: u16,
    y: u16,
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct TouchCalibrationConfig {
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
const TOUCH_CURSOR_RADIUS: i32 = 4;
const CALIBRATION_CROSS_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_stroke(Rgb565::YELLOW, 4);
const CALIBRATION_CENTER_DOT_STYLE: PrimitiveStyle<Rgb565> =
    PrimitiveStyle::with_fill(Rgb565::WHITE);
const TOUCH_CURSOR_STYLE: PrimitiveStyle<Rgb565> = PrimitiveStyle::with_fill(Rgb565::CYAN);

#[esp_hal::main]
fn main() -> ! {
    let p = esp_hal::init(Config::default());
    esp_println::logger::init_logger(log::LevelFilter::Info);
    esp_println::println!("boot: after esp_hal::init");
    run_after_init(p)
}

fn run_after_init(p: esp_hal::peripherals::Peripherals) -> ! {
    let mut delay = Delay::new();

    esp_println::println!("boot: before cyd sim init");
    let cyd_sim = CYD_SIM.init_with(CydSim::new);
    esp_println::println!("boot: after cyd sim new");
    esp_println::println!("boot: before frame buffer init");
    let frame_buffer = FRAME_BUFFER.init_with(FrameBuffer::new);
    esp_println::println!("boot: after frame buffer init");
    esp_println::println!("boot: before cyd sim render");
    cyd_sim.render_to(frame_buffer);
    esp_println::println!("boot: after cyd sim render");
    let mut calibration_just_completed: bool = false;
    let width = cyd_sim.width() as u16;
    let height = cyd_sim.height() as u16;
    esp_println::println!("boot: after cyd sim init");

    let boot_button = ButtonEsp::new(p.GPIO0, PressedTo::Ground);

    let [mut touch_calibration_flash_block] =
        FlashBlockEsp::new_array::<1>(p.FLASH).expect("boot: failed to create flash block");
    let mut touch_calibration_config =
        match touch_calibration_flash_block.load::<TouchCalibrationConfig>() {
            Ok(Some(touch_calibration_config)) => {
                if touch_calibration_config_is_finite(touch_calibration_config) {
                    esp_println::println!("cal: loaded valid calibration from flash");
                    Some(touch_calibration_config)
                } else {
                    esp_println::println!("cal: stored calibration failed app validation");
                    None
                }
            }
            Ok(None) => {
                esp_println::println!("cal: no calibration in flash");
                None
            }
            Err(error) => {
                esp_println::println!("cal: failed to load calibration from flash: {:?}", error);
                None
            }
        };

    esp_println::println!("boot: before display spi init");
    let spi_config = spi::master::Config::default()
        .with_frequency(esp_hal::time::Rate::from_hz(DISPLAY_SPI_HZ))
        .with_mode(spi::Mode::_0);
    let spi = spi::master::Spi::new(p.SPI2, spi_config)
        .expect("boot: failed to configure display spi")
        .with_sck(p.GPIO14)
        .with_mosi(p.GPIO13)
        .with_miso(p.GPIO12);
    let spi_bus = RefCell::new(spi);
    esp_println::println!("boot: after display spi init");

    esp_println::println!("boot: before display pin init");
    let display_cs = Output::new(p.GPIO15, Level::High, OutputConfig::default());
    let dc = Output::new(p.GPIO2, Level::Low, OutputConfig::default());
    let rst = Output::new(p.GPIO4, Level::High, OutputConfig::default());
    let touch_cs = Output::new(p.GPIO33, Level::High, OutputConfig::default());
    let touch_irq = Input::new(p.GPIO36, InputConfig::default().with_pull(Pull::Up));
    let mut display_backlight = Output::new(p.GPIO21, Level::High, OutputConfig::default());

    let display_spi_device = RefCellDevice::new_no_delay(&spi_bus, display_cs)
        .expect("boot: failed to create display spi device");

    // Touch has its own dedicated SPI bus (VSPI/SPI3) on GPIO25/32/39.
    let touch_spi_config = spi::master::Config::default()
        .with_frequency(esp_hal::time::Rate::from_hz(2_500_000))
        .with_mode(spi::Mode::_0);
    let touch_spi = spi::master::Spi::new(p.SPI3, touch_spi_config)
        .expect("boot: failed to configure touch spi")
        .with_sck(p.GPIO25)
        .with_mosi(p.GPIO32)
        .with_miso(p.GPIO39);
    let touch_spi_bus = RefCell::new(touch_spi);
    let mut touch_spi_device = RefCellDevice::new_no_delay(&touch_spi_bus, touch_cs)
        .expect("boot: failed to create touch spi device");
    let mut touch_input = Xpt2046TouchInput::new(touch_irq);

    let display_interface_buffer =
        DISPLAY_INTERFACE_BUFFER.init([0u8; DISPLAY_INTERFACE_BUFFER_BYTES]);
    let display_interface = SpiInterface::new(display_spi_device, dc, display_interface_buffer);
    esp_println::println!("boot: after display pin init");

    esp_println::println!("boot: before display init");
    let mut display = Builder::new(ILI9341Rgb565, display_interface)
        .reset_pin(rst)
        .display_size(240, 320)
        .color_order(ColorOrder::Bgr)
        .orientation(
            Orientation::new()
                .rotate(Rotation::Deg90)
                .flip_horizontal()
                .rotate(Rotation::Deg180),
        )
        .init(&mut delay)
        .expect("boot: failed to initialize mipidsi display");
    esp_println::println!("boot: after display init");

    display_backlight.set_high();

    esp_println::println!("boot: before initial frame flush");
    cyd_sim.touch_up();
    // Keep RK off while validating touch behavior.
    // cyd_sim.start_reverse_kinematics();
    esp_println::println!("boot: RK off, touch test mode");
    if touch_calibration_config.is_some() {
        cyd_sim.render_to(frame_buffer);
    } else {
        frame_buffer.clear(Rgb565::BLACK);
        if let Some(calibration_corner) = calibration_corner_for_index(0) {
            draw_calibration_cross(frame_buffer, calibration_corner, width, height);
        }
    }
    flush_full_frame(&mut display, frame_buffer, width, height);
    esp_println::println!("boot: after initial frame flush");

    let mut previous_tick = Instant::now();
    let mut alive_tick_count: u32 = 0;
    let mut touch_poll_counter: u32 = 0;
    let mut touch_move_log_counter: u32 = 0;
    let mut last_touch_irq_low = false;
    let mut calibration_index: usize = 0;
    let mut calibration_points: [RawPoint; 4] = [RawPoint { x: 0, y: 0 }; 4];
    let mut touch_cursor: Option<Point> = None;
    let mut calibration_screen_dirty = false;

    if touch_calibration_config.is_none() {
        esp_println::println!("cal: tap corners in order UL -> UR -> LR -> LL");
        esp_println::println!("cal: next tap UL");
    }
    esp_println::println!("boot: entering app loop");

    let mut previous_frame_flush = Instant::now();
    let mut rendered_frame_count: u32 = 0;
    let mut last_boot_button_pressed = false;

    loop {
        let now = Instant::now();
        let dt_seconds = (now - previous_tick).as_micros() as f32 / 1_000_000.0;
        previous_tick = now;

        let mut should_flush = cyd_sim.tick_reverse_kinematics(dt_seconds);
        let mut calibration_active = touch_calibration_config.is_none();

        if calibration_active && calibration_screen_dirty {
            should_flush = true;
        }

        // Check if BOOT button was pressed (rising edge) to trigger/restart calibration.
        let boot_button_pressed = boot_button.is_pressed();
        if boot_button_pressed && !last_boot_button_pressed {
            if !calibration_active {
                esp_println::println!("cal: BOOT pressed during runtime, entering calibration");
            } else {
                esp_println::println!("cal: BOOT pressed, restarting calibration");
            }
            touch_calibration_config = None;
            calibration_index = 0;
            calibration_points = [RawPoint { x: 0, y: 0 }; 4];
            touch_cursor = None;
            calibration_screen_dirty = true;
            calibration_active = true;
            should_flush = true;
            esp_println::println!("cal: tap corners in order UL -> UR -> LR -> LL");
            esp_println::println!("cal: next tap UL");
        }
        last_boot_button_pressed = boot_button_pressed;

        touch_poll_counter = touch_poll_counter.wrapping_add(1);
        let touch_irq_low = touch_input.irq_is_low_for_log();
        if TOUCH_LOGGING && touch_irq_low != last_touch_irq_low {
            esp_println::println!("touch: irq_low={} (state change)", touch_irq_low);
            last_touch_irq_low = touch_irq_low;
        } else if TOUCH_LOGGING && !calibration_active && touch_poll_counter % 1000 == 0 {
            esp_println::println!("touch: heartbeat irq_low={}", touch_irq_low);
        } else {
            last_touch_irq_low = touch_irq_low;
        }

        if let Some(raw_touch_event) = touch_input.read_raw_touch_event(&mut touch_spi_device) {
            match raw_touch_event {
                RawTouchEvent::Down { raw_x, raw_y } => {
                    if calibration_active && calibration_index < 4 {
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
                        } else {
                            let config =
                                compute_calibration_four_point(calibration_points, width, height);
                            touch_calibration_flash_block
                                .save(&config)
                                .expect("cal: failed to save calibration to flash");
                            touch_calibration_config = Some(config);
                            calibration_just_completed = true;
                            if let Some(config) = touch_calibration_config {
                                esp_println::println!(
                                    "cal: done ax={:.5} bx={:.5} cx={:.1} ay={:.5} by={:.5} cy={:.1}",
                                    config.ax,
                                    config.bx,
                                    config.cx,
                                    config.ay,
                                    config.by,
                                    config.cy
                                );
                                esp_println::println!(
                                    "cal: controls enabled with computed calibration"
                                );
                            }
                        }
                        continue;
                    }

                    if let Some(config) = touch_calibration_config {
                        let (mapped_x, mapped_y) =
                            map_raw_to_screen(raw_x, raw_y, config, width as f32, height as f32);
                        touch_cursor = Some(Point::new(mapped_x as i32, mapped_y as i32));
                        if TOUCH_LOGGING {
                            esp_println::println!(
                                "touch: down x={:.1} y={:.1} irq_low={}",
                                mapped_x,
                                mapped_y,
                                touch_irq_low
                            );
                        }
                        cyd_sim.touch_down(mapped_x, mapped_y);
                        if cyd_sim.take_calibration_request() {
                            cyd_sim.touch_up();
                            touch_calibration_config = None;
                            calibration_index = 0;
                            calibration_points = [RawPoint { x: 0, y: 0 }; 4];
                            touch_cursor = None;
                            calibration_screen_dirty = true;
                            calibration_active = true;
                            esp_println::println!("cal: requested from UI");
                            esp_println::println!("cal: tap corners in order UL -> UR -> LR -> LL");
                            esp_println::println!("cal: next tap UL");
                        }
                        should_flush = true;
                    }
                }
                RawTouchEvent::Move { raw_x, raw_y } => {
                    if let Some(config) = touch_calibration_config {
                        let (mapped_x, mapped_y) =
                            map_raw_to_screen(raw_x, raw_y, config, width as f32, height as f32);
                        touch_cursor = Some(Point::new(mapped_x as i32, mapped_y as i32));
                        if TOUCH_LOGGING {
                            touch_move_log_counter = touch_move_log_counter.wrapping_add(1);
                        }
                        if TOUCH_LOGGING && touch_move_log_counter % 25 == 0 {
                            esp_println::println!(
                                "touch: move x={:.1} y={:.1} irq_low={}",
                                mapped_x,
                                mapped_y,
                                touch_irq_low
                            );
                        }
                        cyd_sim.touch_move(mapped_x, mapped_y);
                        should_flush = true;
                    }
                }
                RawTouchEvent::Up => {
                    if touch_calibration_config.is_some() {
                        if TOUCH_LOGGING {
                            esp_println::println!("touch: up irq_low={}", touch_irq_low);
                        }
                        cyd_sim.touch_up();
                    }
                    touch_cursor = None;
                }
            }
        }

        if calibration_just_completed {
            should_flush = true;
            calibration_just_completed = false;
        }

        if should_flush {
            let frame_dt_seconds = (now - previous_frame_flush).as_micros() as f32 / 1_000_000.0;
            previous_frame_flush = now;
            cyd_sim.set_frame_dt_seconds(frame_dt_seconds);

            let render_start = Instant::now();
            let render_calibration_active = touch_calibration_config.is_none();
            if render_calibration_active {
                frame_buffer.clear(Rgb565::BLACK);
                if let Some(calibration_corner) = calibration_corner_for_index(calibration_index) {
                    draw_calibration_cross(frame_buffer, calibration_corner, width, height);
                }
                calibration_screen_dirty = false;
            } else {
                cyd_sim.render_to(frame_buffer);
                if let Some(cursor) = touch_cursor {
                    draw_touch_cursor(frame_buffer, cursor);
                }
            }
            let flush_start = Instant::now();
            flush_full_frame(&mut display, frame_buffer, width, height);
            let flush_end = Instant::now();

            rendered_frame_count = rendered_frame_count.wrapping_add(1);
            if FRAME_PROFILE_LOGGING && !render_calibration_active && rendered_frame_count % 60 == 0
            {
                let render_ms = (flush_start - render_start).as_micros() as f32 / 1000.0;
                let flush_ms = (flush_end - flush_start).as_micros() as f32 / 1000.0;
                esp_println::println!(
                    "frame: period_ms={:.1} render_ms={:.1} flush_ms={:.1}",
                    frame_dt_seconds * 1000.0,
                    render_ms,
                    flush_ms
                );
            }
        }

        alive_tick_count = alive_tick_count.wrapping_add(1);
        if TOUCH_LOGGING && !calibration_active && alive_tick_count % 60 == 0 {
            esp_println::println!("boot: alive");
        }

        if !should_flush {
            delay.delay_millis(1);
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
) -> TouchCalibrationConfig {
    let ul = calibration_corner_center(CalibrationCorner::UpperLeft, width, height);
    let ur = calibration_corner_center(CalibrationCorner::UpperRight, width, height);
    let lr = calibration_corner_center(CalibrationCorner::LowerRight, width, height);
    let ll = calibration_corner_center(CalibrationCorner::LowerLeft, width, height);
    let screen_targets = [ul, ur, lr, ll];

    let (ax, bx, cx) = solve_affine_axis(points, screen_targets, true);
    let (ay, by, cy) = solve_affine_axis(points, screen_targets, false);

    TouchCalibrationConfig {
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
    calibration: TouchCalibrationConfig,
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

fn touch_calibration_config_is_finite(config: TouchCalibrationConfig) -> bool {
    config.ax.is_finite()
        && config.bx.is_finite()
        && config.cx.is_finite()
        && config.ay.is_finite()
        && config.by.is_finite()
        && config.cy.is_finite()
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
) {
    let center = calibration_corner_center(calibration_corner, width, height);
    let left = Point::new(center.x - CALIBRATION_CROSS_HALF_SIZE, center.y);
    let right = Point::new(center.x + CALIBRATION_CROSS_HALF_SIZE, center.y);
    let top = Point::new(center.x, center.y - CALIBRATION_CROSS_HALF_SIZE);
    let bottom = Point::new(center.x, center.y + CALIBRATION_CROSS_HALF_SIZE);

    Line::new(left, right)
        .into_styled(CALIBRATION_CROSS_STYLE)
        .draw(frame_buffer)
        .expect("boot: failed to draw calibration cross horizontal line");
    Line::new(top, bottom)
        .into_styled(CALIBRATION_CROSS_STYLE)
        .draw(frame_buffer)
        .expect("boot: failed to draw calibration cross vertical line");

    Circle::new(
        Point::new(
            center.x - CALIBRATION_CENTER_DOT_RADIUS,
            center.y - CALIBRATION_CENTER_DOT_RADIUS,
        ),
        (CALIBRATION_CENTER_DOT_RADIUS * 2 + 1) as u32,
    )
    .into_styled(CALIBRATION_CENTER_DOT_STYLE)
    .draw(frame_buffer)
    .expect("boot: failed to draw calibration center dot");
}

fn draw_touch_cursor(frame_buffer: &mut FrameBuffer, cursor: Point) {
    let center = Point::new(cursor.x, cursor.y);
    Circle::new(
        Point::new(
            center.x - TOUCH_CURSOR_RADIUS,
            center.y - TOUCH_CURSOR_RADIUS,
        ),
        (TOUCH_CURSOR_RADIUS * 2 + 1) as u32,
    )
    .into_styled(TOUCH_CURSOR_STYLE)
    .draw(frame_buffer)
    .expect("boot: failed to draw touch cursor");
}

fn flush_full_frame(
    display: &mut impl DrawTarget<Color = Rgb565>,
    frame_buffer: &FrameBuffer,
    width: u16,
    height: u16,
) {
    let full_screen = Rectangle::new(Point::new(0, 0), Size::new(width as u32, height as u32));
    if display
        .fill_contiguous(&full_screen, frame_buffer.pixels().iter().copied())
        .is_err()
    {
        panic!("boot: failed to flush framebuffer");
    }
}
