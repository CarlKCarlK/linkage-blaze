#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchEvent {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up,
}

pub trait TouchInput {
    fn read_touch_event(
        &mut self,
        display: &mut crate::display::Ili9341RectWriter<
            impl embedded_hal::spi::SpiBus,
            impl embedded_hal::digital::OutputPin,
            impl embedded_hal::digital::OutputPin,
            impl embedded_hal::digital::OutputPin,
        >,
    ) -> Option<TouchEvent>;
}

// TODO0 Calibrate XPT2046 raw-to-screen mapping on hardware with CYD display.
// Current constants are hard-coded placeholders.
const XPT2046_RAW_X_MIN: u16 = 100;
const XPT2046_RAW_X_MAX: u16 = 3900;
const XPT2046_RAW_Y_MIN: u16 = 100;
const XPT2046_RAW_Y_MAX: u16 = 3900;

/// Concrete XPT2046 touch controller input for CYD with shared SPI.
/// Hard-coded for CYD pins: touch CS on GPIO33, touch IRQ on GPIO36.
pub struct Xpt2046TouchInput<TouchCs, TouchIrq> {
    touch_cs: TouchCs,
    touch_irq: TouchIrq,
    was_pressed: bool,
}

impl<TouchCs, TouchIrq> Xpt2046TouchInput<TouchCs, TouchIrq>
where
    TouchCs: embedded_hal::digital::OutputPin,
    TouchIrq: embedded_hal::digital::InputPin,
{
    pub fn new(touch_cs: TouchCs, touch_irq: TouchIrq) -> Self {
        Self {
            touch_cs,
            touch_irq,
            was_pressed: false,
        }
    }

    fn is_pressed(&mut self) -> bool {
        // XPT2046 IRQ is active-low; pressed when low.
        self.touch_irq.is_low().unwrap_or(false)
    }

    fn read_raw_xy<SPI, DC, RST, CS>(
        &mut self,
        display: &mut crate::display::Ili9341RectWriter<SPI, DC, RST, CS>,
    ) -> Option<(u16, u16)>
    where
        SPI: embedded_hal::spi::SpiBus,
        DC: embedded_hal::digital::OutputPin,
        RST: embedded_hal::digital::OutputPin,
        CS: embedded_hal::digital::OutputPin,
    {
        let _ = self.touch_cs.set_low();

        // XPT2046 command: 0xD1 (read Y+), 0x91 (read X+) with 12-bit resolution.
        let tx_buf = [0xD1u8, 0x00, 0x91, 0x00];
        let mut rx_buf = [0u8; 4];

        display.touch_spi_transact(&tx_buf, &mut rx_buf);

        let _ = self.touch_cs.set_high();

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
}

impl<TouchCs, TouchIrq> TouchInput for Xpt2046TouchInput<TouchCs, TouchIrq>
where
    TouchCs: embedded_hal::digital::OutputPin,
    TouchIrq: embedded_hal::digital::InputPin,
{
    fn read_touch_event(
        &mut self,
        display: &mut crate::display::Ili9341RectWriter<
            impl embedded_hal::spi::SpiBus,
            impl embedded_hal::digital::OutputPin,
            impl embedded_hal::digital::OutputPin,
            impl embedded_hal::digital::OutputPin,
        >,
    ) -> Option<TouchEvent> {
        let is_pressed_now = self.is_pressed();

        if is_pressed_now {
            if let Some((raw_x, raw_y)) = self.read_raw_xy(display) {
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
