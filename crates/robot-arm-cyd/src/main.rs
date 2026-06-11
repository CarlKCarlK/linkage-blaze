#![no_std]
#![no_main]

use core::cell::RefCell;

use embedded_graphics::{
    Drawable, Pixel,
    pixelcolor::Rgb565,
    prelude::{DrawTarget, Point, Primitive, RgbColor},
    primitives::{Circle, Line, PrimitiveStyle},
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
static DISPLAY_INTERFACE_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();

const TOUCH_CALIBRATION_MODE: bool = true;

#[derive(Clone, Copy)]
struct RawPoint {
    x: u16,
    y: u16,
}

#[derive(Clone, Copy)]
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

    esp_println::println!("boot: before display spi init");
    let spi_config = spi::master::Config::default()
        .with_frequency(esp_hal::time::Rate::from_hz(40_000_000))
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

    let display_interface_buffer = DISPLAY_INTERFACE_BUFFER.init([0u8; 512]);
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

    esp_println::println!("boot: before display probe write");
    display_backlight.set_high();
    display
        .clear(Rgb565::RED)
        .expect("boot: failed to clear display red");
    esp_println::println!("boot: full-screen red");
    delay.delay_millis(200);
    display
        .clear(Rgb565::GREEN)
        .expect("boot: failed to clear display green");
    esp_println::println!("boot: full-screen green");
    delay.delay_millis(200);
    display
        .clear(Rgb565::BLUE)
        .expect("boot: failed to clear display blue");
    esp_println::println!("boot: full-screen blue");
    delay.delay_millis(200);
    display
        .clear(Rgb565::WHITE)
        .expect("boot: failed to clear display white");
    esp_println::println!("boot: full-screen white");
    delay.delay_millis(100);
    esp_println::println!("boot: after display probe write");

    esp_println::println!("boot: before initial frame flush");
    cyd_sim.touch_up();
    // Keep RK off while validating touch behavior.
    // cyd_sim.start_reverse_kinematics();
    esp_println::println!("boot: RK off, touch test mode");
    if TOUCH_CALIBRATION_MODE {
        frame_buffer.clear(Rgb565::BLACK);
        if let Some(calibration_corner) = calibration_corner_for_index(0) {
            draw_calibration_cross(frame_buffer, calibration_corner, width, height);
        }
    } else {
        cyd_sim.render_to(frame_buffer);
    }
    flush_full_frame(&mut display, frame_buffer, width, height);
    esp_println::println!("boot: after initial frame flush");

    let mut previous_tick = Instant::now();
    let mut alive_tick_count: u32 = 0;
    let mut touch_poll_counter: u32 = 0;
    let mut touch_move_log_counter: u32 = 0;
    let mut last_touch_irq_low = false;
    let mut calibration_prompt_counter: u32 = 0;
    let mut touch_calibration_config: Option<TouchCalibrationConfig> = None;
    let mut calibration_index: usize = 0;
    let mut calibration_points: [RawPoint; 3] = [RawPoint { x: 0, y: 0 }; 3];
    let mut touch_cursor: Option<Point> = None;

    if TOUCH_CALIBRATION_MODE {
        esp_println::println!("cal: tap corners in order UL -> UR -> LL");
        esp_println::println!("cal: next tap UL");
    }
    esp_println::println!("boot: entering app loop");

    let mut previous_frame_flush = Instant::now();

    loop {
        let now = Instant::now();
        let dt_seconds = (now - previous_tick).as_micros() as f32 / 1_000_000.0;
        previous_tick = now;

        let mut should_flush = cyd_sim.tick_reverse_kinematics(dt_seconds);
        let calibration_corner = calibration_corner_for_index(calibration_index);
        let calibration_active = TOUCH_CALIBRATION_MODE && touch_calibration_config.is_none();

        if calibration_active {
            should_flush = true;
        }

        touch_poll_counter = touch_poll_counter.wrapping_add(1);
        let touch_irq_low = touch_input.irq_is_low_for_log();
        if touch_irq_low != last_touch_irq_low {
            esp_println::println!("touch: irq_low={} (state change)", touch_irq_low);
            last_touch_irq_low = touch_irq_low;
        } else if !TOUCH_CALIBRATION_MODE && touch_poll_counter % 1000 == 0 {
            esp_println::println!("touch: heartbeat irq_low={}", touch_irq_low);
        }

        if TOUCH_CALIBRATION_MODE && calibration_index < 3 {
            calibration_prompt_counter = calibration_prompt_counter.wrapping_add(1);
            if calibration_prompt_counter % 1200 == 0 {
                let corner_label = ["UL", "UR", "LL"][calibration_index];
                esp_println::println!("cal: next tap {}", corner_label);
            }
        }

        if let Some(raw_touch_event) = touch_input.read_raw_touch_event(&mut touch_spi_device) {
            match raw_touch_event {
                RawTouchEvent::Down { raw_x, raw_y } => {
                    if TOUCH_CALIBRATION_MODE && calibration_index < 3 {
                        calibration_points[calibration_index] = RawPoint { x: raw_x, y: raw_y };
                        calibration_index += 1;
                        esp_println::println!(
                            "cal: point{} raw_x={} raw_y={}",
                            calibration_index,
                            raw_x,
                            raw_y
                        );
                        if calibration_index < 3 {
                            let corner_label = ["UL", "UR", "LL"][calibration_index];
                            esp_println::println!("cal: next tap {}", corner_label);
                        } else {
                            touch_calibration_config = Some(compute_calibration_three_point(
                                calibration_points,
                                width,
                                height,
                            ));
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
                        esp_println::println!(
                            "touch: down x={:.1} y={:.1} irq_low={}",
                            mapped_x,
                            mapped_y,
                            touch_irq_low
                        );
                        cyd_sim.touch_down(mapped_x, mapped_y);
                        should_flush = true;
                    }
                }
                RawTouchEvent::Move { raw_x, raw_y } => {
                    if let Some(config) = touch_calibration_config {
                        let (mapped_x, mapped_y) =
                            map_raw_to_screen(raw_x, raw_y, config, width as f32, height as f32);
                        touch_cursor = Some(Point::new(mapped_x as i32, mapped_y as i32));
                        touch_move_log_counter = touch_move_log_counter.wrapping_add(1);
                        if touch_move_log_counter % 25 == 0 {
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
                        esp_println::println!("touch: up irq_low={}", touch_irq_low);
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

            if calibration_active {
                frame_buffer.clear(Rgb565::BLACK);
                if let Some(calibration_corner) = calibration_corner {
                    draw_calibration_cross(frame_buffer, calibration_corner, width, height);
                }
            } else {
                cyd_sim.render_to(frame_buffer);
                if let Some(cursor) = touch_cursor {
                    draw_touch_cursor(frame_buffer, cursor);
                }
            }
            flush_full_frame(&mut display, frame_buffer, width, height);
        }

        alive_tick_count = alive_tick_count.wrapping_add(1);
        if !TOUCH_CALIBRATION_MODE && alive_tick_count % 60 == 0 {
            esp_println::println!("boot: alive");
        }

        delay.delay_millis(1);
    }
}

fn solve_affine_axis(
    points: [RawPoint; 3],
    screen: [Point; 3],
    map_x_axis: bool,
) -> (f32, f32, f32) {
    let raw0_x = points[0].x as f32;
    let raw0_y = points[0].y as f32;
    let raw1_x = points[1].x as f32;
    let raw1_y = points[1].y as f32;
    let raw2_x = points[2].x as f32;
    let raw2_y = points[2].y as f32;

    let out0 = if map_x_axis {
        screen[0].x as f32
    } else {
        screen[0].y as f32
    };
    let out1 = if map_x_axis {
        screen[1].x as f32
    } else {
        screen[1].y as f32
    };
    let out2 = if map_x_axis {
        screen[2].x as f32
    } else {
        screen[2].y as f32
    };

    let denominator =
        raw0_x * (raw1_y - raw2_y) + raw1_x * (raw2_y - raw0_y) + raw2_x * (raw0_y - raw1_y);

    assert!(
        denominator.abs() >= 0.001,
        "cal: invalid touch calibration geometry"
    );

    let axis_a = (out0 * (raw1_y - raw2_y) + out1 * (raw2_y - raw0_y) + out2 * (raw0_y - raw1_y))
        / denominator;
    let axis_b = (out0 * (raw2_x - raw1_x) + out1 * (raw0_x - raw2_x) + out2 * (raw1_x - raw0_x))
        / denominator;
    let axis_c = (out0 * (raw1_x * raw2_y - raw2_x * raw1_y)
        + out1 * (raw2_x * raw0_y - raw0_x * raw2_y)
        + out2 * (raw0_x * raw1_y - raw1_x * raw0_y))
        / denominator;

    (axis_a, axis_b, axis_c)
}

fn compute_calibration_three_point(
    points: [RawPoint; 3],
    width: u16,
    height: u16,
) -> TouchCalibrationConfig {
    let ul = calibration_corner_center(CalibrationCorner::UpperLeft, width, height);
    let ur = calibration_corner_center(CalibrationCorner::UpperRight, width, height);
    let ll = calibration_corner_center(CalibrationCorner::LowerLeft, width, height);
    let screen_targets = [ul, ur, ll];

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

fn calibration_corner_for_index(calibration_index: usize) -> Option<CalibrationCorner> {
    match calibration_index {
        0 => Some(CalibrationCorner::UpperLeft),
        1 => Some(CalibrationCorner::UpperRight),
        2 => Some(CalibrationCorner::LowerLeft),
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

fn remap_touch_for_display_rotation(x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
    // Raw mapping now matches display orientation after calibration.
    (x.clamp(0.0, width), y.clamp(0.0, height))
}

fn flush_full_frame(
    display: &mut impl DrawTarget<Color = Rgb565>,
    frame_buffer: &FrameBuffer,
    width: u16,
    _height: u16,
) {
    let width_usize = width as usize;
    let pixel_iter = frame_buffer
        .pixels()
        .iter()
        .enumerate()
        .map(|(pixel_index, color_pixel)| {
            let column = (pixel_index % width_usize) as i32;
            let row = (pixel_index / width_usize) as i32;
            Pixel(
                embedded_graphics::prelude::Point::new(column, row),
                *color_pixel,
            )
        });

    if display.draw_iter(pixel_iter).is_err() {
        panic!("boot: failed to draw framebuffer");
    }
}
