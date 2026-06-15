#![no_std]

mod buffer;
mod calibration;
mod display;
mod touch;

use core::fmt;

use device_envoy_esp::{
    button::Button,
    flash_block::{FlashBlock, FlashBlockEsp},
};
use embedded_graphics::{pixelcolor::Rgb565, prelude::Point, primitives::Rectangle};

pub use buffer::{RectBuffer, RectPixels, RectView, RectWorkspace};
pub use calibration::{CalibrationConfig, RawPoint, TouchInputEvent, map_raw_to_screen};
pub use display::{
    CydDisplay, CydDisplayFlushError, CydDisplayInitError, DISPLAY_SPI_HZ, DrawPrimitive, Ellipse,
    LineSegment,
};
pub use robot_arm_core::cyd::{SCREEN_HEIGHT, SCREEN_WIDTH};
pub use touch::{CydTouch, CydTouchInitError, RawTouchEvent, TOUCH_SPI_HZ};

pub struct Cyd {
    display: CydDisplay,
    touch: Option<CydTouch>,
    calibration_config: Option<CalibrationConfig>,
    calibration_flash_block: Option<FlashBlockEsp>,
    calibration_button: Option<device_envoy_esp::button::ButtonEsp<'static>>,
}

pub struct CalibratedCyd<'a> {
    cyd: &'a mut Cyd,
    calibration_config: CalibrationConfig,
}

#[derive(Debug)]
pub enum CydError {
    Flash(device_envoy_esp::Error),
    DisplayInit(CydDisplayInitError),
    TouchInit(CydTouchInitError),
    DisplayFlush(CydDisplayFlushError),
    TouchUnavailable,
    CalibrationUnavailable,
}

impl From<device_envoy_esp::Error> for CydError {
    fn from(error: device_envoy_esp::Error) -> Self {
        Self::Flash(error)
    }
}

impl From<CydDisplayInitError> for CydError {
    fn from(error: CydDisplayInitError) -> Self {
        Self::DisplayInit(error)
    }
}

impl From<CydTouchInitError> for CydError {
    fn from(error: CydTouchInitError) -> Self {
        Self::TouchInit(error)
    }
}

impl From<CydDisplayFlushError> for CydError {
    fn from(error: CydDisplayFlushError) -> Self {
        Self::DisplayFlush(error)
    }
}

impl Cyd {
    pub fn new_display(
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
    ) -> Result<Self, CydError> {
        Self::new_inner(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            None,
            None,
            None,
        )
    }

    pub fn new(
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        calibration_flash_block: FlashBlockEsp,
        calibration_button: device_envoy_esp::button::ButtonEsp<'static>,
    ) -> Result<Self, CydError> {
        Self::new_inner(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            None,
            Some(calibration_flash_block),
            Some(calibration_button),
        )
    }

    pub fn new_with_touch(
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
        calibration_button: device_envoy_esp::button::ButtonEsp<'static>,
    ) -> Result<Self, CydError> {
        let touch = CydTouch::new(
            touch_spi,
            touch_sck_pin,
            touch_mosi_pin,
            touch_miso_pin,
            touch_cs_pin,
            touch_irq_pin,
        )?;

        Self::new_inner(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            Some(touch),
            Some(calibration_flash_block),
            Some(calibration_button),
        )
    }

    fn new_inner(
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        touch: Option<CydTouch>,
        calibration_flash_block: Option<FlashBlockEsp>,
        calibration_button: Option<device_envoy_esp::button::ButtonEsp<'static>>,
    ) -> Result<Self, CydError> {
        let mut calibration_flash_block = calibration_flash_block;
        let calibration_config = match (&mut calibration_flash_block, &calibration_button) {
            (Some(calibration_flash_block), Some(calibration_button))
                if !calibration_button.is_pressed() =>
            {
                calibration_flash_block.load::<CalibrationConfig>()?
            }
            _ => None,
        };

        Ok(Self {
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
            touch,
            calibration_config,
            calibration_flash_block,
            calibration_button,
        })
    }

    #[must_use]
    pub fn calibration_config(&self) -> Option<CalibrationConfig> {
        self.calibration_config
    }

    #[must_use]
    pub fn recalibration_requested(&self) -> bool {
        self.calibration_button
            .as_ref()
            .is_some_and(Button::is_pressed)
    }

    pub fn remove_calibration(&mut self) {
        self.calibration_config = None;
    }

    pub fn save_calibration(
        &mut self,
        calibration_config: CalibrationConfig,
    ) -> Result<(), CydError> {
        let calibration_flash_block = self
            .calibration_flash_block
            .as_mut()
            .ok_or(CydError::CalibrationUnavailable)?;
        calibration_flash_block.save(&calibration_config)?;
        self.calibration_config = Some(calibration_config);
        Ok(())
    }

    pub fn clear_saved_calibration(&mut self) -> Result<(), CydError> {
        let calibration_flash_block = self
            .calibration_flash_block
            .as_mut()
            .ok_or(CydError::CalibrationUnavailable)?;
        calibration_flash_block.clear()?;
        self.calibration_config = None;
        Ok(())
    }

    pub fn ensure_calibration(&mut self) -> Result<CalibratedCyd<'_>, CydError> {
        if self.recalibration_requested() {
            self.calibration_config = None;
        }

        let calibration_config = self
            .calibration_config
            .ok_or(CydError::CalibrationUnavailable)?;

        Ok(CalibratedCyd {
            cyd: self,
            calibration_config,
        })
    }

    pub fn read_raw_touch_event(&mut self) -> Option<RawTouchEvent> {
        self.touch.as_mut()?.read_raw_touch_event()
    }

    pub fn flush(&mut self, buffer: &impl RectPixels, top_left: Point) -> Result<(), CydError> {
        Ok(self.display.flush_buffer(buffer, top_left)?)
    }

    pub fn clear_now(&mut self, color: Rgb565) -> Result<(), CydError> {
        Ok(self.display.clear_now(color)?)
    }

    pub fn fill_rect_now(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), CydError> {
        Ok(self.display.fill_rect_now(rectangle, color)?)
    }

    pub fn draw_line_segments_now(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        segments: &[LineSegment],
    ) -> Result<(), CydError> {
        Ok(self
            .display
            .draw_line_segments_now(bounds, background, segments)?)
    }

    pub fn draw_primitives_now(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        primitives: &[DrawPrimitive],
    ) -> Result<(), CydError> {
        Ok(self
            .display
            .draw_primitives_now(bounds, background, primitives)?)
    }
}

impl CalibratedCyd<'_> {
    pub fn remove_calibration(&mut self) {
        self.cyd.remove_calibration();
    }

    pub fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, CydError> {
        let raw_touch_event = self
            .cyd
            .touch
            .as_mut()
            .ok_or(CydError::TouchUnavailable)?
            .read_raw_touch_event();

        Ok(
            raw_touch_event.map(|raw_touch_event| match raw_touch_event {
                RawTouchEvent::Down { raw_x, raw_y } => {
                    let (x, y) = map_raw_to_screen(raw_x, raw_y, self.calibration_config);
                    TouchInputEvent::Down { x, y }
                }
                RawTouchEvent::Move { raw_x, raw_y } => {
                    let (x, y) = map_raw_to_screen(raw_x, raw_y, self.calibration_config);
                    TouchInputEvent::Move { x, y }
                }
                RawTouchEvent::Up => TouchInputEvent::Up,
            }),
        )
    }

    pub fn flush(&mut self, buffer: &impl RectPixels, top_left: Point) -> Result<(), CydError> {
        self.cyd.flush(buffer, top_left)
    }

    pub fn clear_now(&mut self, color: Rgb565) -> Result<(), CydError> {
        self.cyd.clear_now(color)
    }

    pub fn fill_rect_now(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), CydError> {
        self.cyd.fill_rect_now(rectangle, color)
    }

    pub fn draw_line_segments_now(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        segments: &[LineSegment],
    ) -> Result<(), CydError> {
        self.cyd
            .draw_line_segments_now(bounds, background, segments)
    }

    pub fn draw_primitives_now(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        primitives: &[DrawPrimitive],
    ) -> Result<(), CydError> {
        self.cyd.draw_primitives_now(bounds, background, primitives)
    }
}

impl fmt::Debug for Cyd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Cyd").finish_non_exhaustive()
    }
}
