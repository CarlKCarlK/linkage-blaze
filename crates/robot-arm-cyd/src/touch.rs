#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RawTouchEvent {
    Down { raw_x: u16, raw_y: u16 },
    Move { raw_x: u16, raw_y: u16 },
    Up,
}

const TOUCH_RAW_LOGGING: bool = false;

/// Concrete XPT2046 touch controller input for CYD with shared SPI.
/// Hard-coded for CYD pin: touch IRQ on GPIO36.
pub struct Xpt2046TouchInput<TouchIrq> {
    touch_irq: TouchIrq,
    was_pressed: bool,
}

impl<TouchIrq> Xpt2046TouchInput<TouchIrq>
where
    TouchIrq: embedded_hal::digital::InputPin,
{
    pub fn new(touch_irq: TouchIrq) -> Self {
        Self {
            touch_irq,
            was_pressed: false,
        }
    }

    fn is_pressed(&mut self) -> bool {
        // XPT2046 IRQ is active-low; pressed when low.
        self.touch_irq.is_low().unwrap_or(false)
    }

    fn read_single_axis(
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
        command: u8,
    ) -> u16 {
        // Each axis read is its own CS transaction: assert CS, send command + 2 clock bytes, deassert CS.
        // The XPT2046 requires CS to go high between X and Y reads.
        let tx = [command, 0x00, 0x00];
        let mut rx = [0u8; 3];
        touch_spi_device
            .transfer(&mut rx, &tx)
            .expect("touch axis SPI failed");
        // Response: 1 null bit + 12 data bits in bytes [1] and [2], shift right by 3.
        (((rx[1] as u16) << 8) | (rx[2] as u16)) >> 3
    }

    fn read_single_xy(
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
    ) -> Option<(u16, u16)> {
        // Command byte format: START=1, A2:A0, MODE=0 (12-bit), SER/DFR=0, PD=00.
        // A2:A0=101 -> X+  command = 0xD0
        // A2:A0=001 -> Y+  command = 0x90
        // Two separate SpiDevice calls so CS pulses high between X and Y reads.
        let raw_x = Self::read_single_axis(touch_spi_device, 0xD0);
        let raw_y = Self::read_single_axis(touch_spi_device, 0x90);

        if raw_x > 0 && raw_y > 0 {
            Some((raw_x, raw_y))
        } else {
            None
        }
    }

    fn read_raw_xy(
        &mut self,
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
    ) -> Option<(u16, u16)> {
        // Average 3 samples to reduce noise.
        const SAMPLES: u32 = 3;
        let mut sum_x: u32 = 0;
        let mut sum_y: u32 = 0;
        let mut count: u32 = 0;
        for _ in 0..SAMPLES {
            if let Some((x, y)) = Self::read_single_xy(touch_spi_device) {
                sum_x += x as u32;
                sum_y += y as u32;
                count += 1;
            }
        }
        if count > 0 {
            let avg_x = (sum_x / count) as u16;
            let avg_y = (sum_y / count) as u16;
            if TOUCH_RAW_LOGGING {
                esp_println::println!("touch: raw avg_x={} avg_y={}", avg_x, avg_y);
            }
            Some((avg_x, avg_y))
        } else {
            None
        }
    }

    pub fn read_raw_touch_event(
        &mut self,
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
    ) -> Option<RawTouchEvent> {
        let is_pressed_now = self.is_pressed();

        if is_pressed_now {
            if let Some((raw_x, raw_y)) = self.read_raw_xy(touch_spi_device) {
                let event = if self.was_pressed {
                    RawTouchEvent::Move { raw_x, raw_y }
                } else {
                    RawTouchEvent::Down { raw_x, raw_y }
                };

                self.was_pressed = true;
                return Some(event);
            }
        } else if self.was_pressed {
            self.was_pressed = false;
            return Some(RawTouchEvent::Up);
        }

        None
    }
}
