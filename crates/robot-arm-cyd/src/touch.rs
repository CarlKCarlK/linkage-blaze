#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchEvent {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up,
}

pub trait TouchInput {
    fn read_touch_event(
        &mut self,
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
    ) -> Option<TouchEvent>;
}

// TODO0 Calibrate XPT2046 raw-to-screen mapping on hardware with CYD display.
// Current constants are hard-coded placeholders.
const XPT2046_RAW_X_MIN: u16 = 100;
const XPT2046_RAW_X_MAX: u16 = 3900;
const XPT2046_RAW_Y_MIN: u16 = 100;
const XPT2046_RAW_Y_MAX: u16 = 3900;

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

    fn read_raw_xy(
        &mut self,
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
    ) -> Option<(u16, u16)> {
        // XPT2046 command: 0xD1 (read Y+), 0x91 (read X+) with 12-bit resolution.
        let tx_buf = [0xD1u8, 0x00, 0x91, 0x00];
        let mut rx_buf = [0u8; 4];

        touch_spi_device
            .transfer(&mut rx_buf, &tx_buf)
            .expect("touch SPI transaction failed");

        // Extract 12-bit raw values (16-bit words, MSB-first).
        let raw_y = (((rx_buf[0] as u16) << 8) | (rx_buf[1] as u16)) >> 4;
        let raw_x = (((rx_buf[2] as u16) << 8) | (rx_buf[3] as u16)) >> 4;

        if raw_x > 0 && raw_y > 0 {
            Some((raw_x, raw_y))
        } else {
            None
        }
    }

    fn raw_to_screen(raw_x: u16, raw_y: u16) -> (f32, f32) {
        let screen_x = ((raw_x.saturating_sub(XPT2046_RAW_X_MIN) as f32)
            / (XPT2046_RAW_X_MAX - XPT2046_RAW_X_MIN) as f32)
            .clamp(0.0, 1.0)
            * 320.0;

        let screen_y = ((raw_y.saturating_sub(XPT2046_RAW_Y_MIN) as f32)
            / (XPT2046_RAW_Y_MAX - XPT2046_RAW_Y_MIN) as f32)
            .clamp(0.0, 1.0)
            * 240.0;

        (screen_x, screen_y)
    }

    pub fn read_touch_position_for_log(
        &mut self,
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
    ) -> Option<(f32, f32, bool)> {
        let irq_pressed = self.is_pressed();
        self.read_raw_xy(touch_spi_device).map(|(raw_x, raw_y)| {
            let (screen_x, screen_y) = Self::raw_to_screen(raw_x, raw_y);
            (screen_x, screen_y, irq_pressed)
        })
    }
}

impl<TouchIrq> TouchInput for Xpt2046TouchInput<TouchIrq>
where
    TouchIrq: embedded_hal::digital::InputPin,
{
    fn read_touch_event(
        &mut self,
        touch_spi_device: &mut impl embedded_hal::spi::SpiDevice<u8>,
    ) -> Option<TouchEvent> {
        let is_pressed_now = self.is_pressed();

        if is_pressed_now {
            if let Some((raw_x, raw_y)) = self.read_raw_xy(touch_spi_device) {
                let (screen_x, screen_y) = Self::raw_to_screen(raw_x, raw_y);

                let event = if self.was_pressed {
                    TouchEvent::Move {
                        x: screen_x,
                        y: screen_y,
                    }
                } else {
                    TouchEvent::Down {
                        x: screen_x,
                        y: screen_y,
                    }
                };

                self.was_pressed = true;
                return Some(event);
            }
        } else if self.was_pressed {
            self.was_pressed = false;
            return Some(TouchEvent::Up);
        }

        None
    }
}
