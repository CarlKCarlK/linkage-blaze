#![no_std]

mod buffer;
mod calibration;
mod display;
pub mod tiling;
mod touch;
mod translated;

use core::{convert::Infallible, fmt};

use device_envoy_esp::{
    button::Button,
    flash_block::{FlashBlock, FlashBlockEsp},
};
use embedded_graphics::{
    Pixel,
    pixelcolor::{IntoStorage, Rgb565, Rgb888},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    primitives::Rectangle,
};
use linkage_blaze_core::PixelTarget;
use static_cell::StaticCell;

pub use buffer::{DynPixelBuffer, PixelBuffer, RectBuffer, RectPixels, RectView};
pub use calibration::{CalibrationConfig, RawPoint, TouchInputEvent, map_raw_to_screen};
pub use display::{
    CydDisplay, CydDisplayConfig, CydDisplayFlushError, CydDisplayInitError, CydDisplayOrientation,
    DISPLAY_SPI_HZ, DrawPrimitive, Ellipse, LineSegment,
};
pub use linkage_blaze_armatron_core::{SCREEN_HEIGHT, SCREEN_PIXELS, SCREEN_WIDTH};
pub use touch::{CydTouch, CydTouchInitError, RawTouchEvent, TOUCH_SPI_HZ};
pub use translated::TranslatedDrawTarget;

/// A [`PixelBuffer`] sized to the whole CYD panel.
pub type PixelBufferFull = PixelBuffer<{ Cyd::SCREEN_PIXELS }>;

pub struct Cyd {
    display: CydDisplay,
    touch: Option<CydTouch>,
    calibration_config: Option<CalibrationConfig>,
    calibration_flash_block: Option<FlashBlockEsp>,
    calibration_button: Option<device_envoy_esp::button::ButtonEsp<'static>>,
    // Every Cyd owns exactly one draw buffer. Apps that don't draw through it
    // pass a zero-sized buffer (e.g. `CydStatic<PixelBuffer<0>>`).
    pixel_buffer: &'static mut dyn DynPixelBuffer,
}

/// Static storage for a [`Cyd`]-owned pixel buffer.
///
/// The app declares one at file scope and names the buffer type it wants:
///
/// ```ignore
/// static CYD_STATIC: CydStatic<PixelBufferFull> = CydStatic::new();
/// ```
///
/// The app chooses the buffer type (policy); [`Cyd::new_display_only`] owns the
/// initialization protocol.
pub struct CydStatic<B: DynPixelBuffer> {
    pixel_buffer: StaticCell<B>,
}

impl<B: DynPixelBuffer> CydStatic<B> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pixel_buffer: StaticCell::new(),
        }
    }
}

impl<B: DynPixelBuffer> Default for CydStatic<B> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CalibratedCyd<'a> {
    cyd: &'a mut Cyd,
    calibration_config: CalibrationConfig,
}

pub struct CydFrame<'a> {
    display: &'a mut CydDisplay,
    view: RectView<'a>,
}

impl<'a> CydFrame<'a> {
    pub fn view_mut(&mut self) -> &mut RectView<'a> {
        &mut self.view
    }

    pub fn clear(&mut self, color: Rgb565) {
        self.view.clear(color);
    }

    #[must_use]
    pub fn width(&self) -> usize {
        self.view.width()
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.view.height()
    }

    pub fn raw_pixels_mut(&mut self) -> &mut [u16] {
        self.view.raw_pixels_mut()
    }

    pub fn flush(self) -> Result<(), CydError> {
        self.flush_at(Point::new(0, 0))
    }

    pub fn flush_at(self, top_left: Point) -> Result<(), CydError> {
        Ok(self.display.flush_buffer(&self.view, top_left)?)
    }
}

impl DrawTarget for CydFrame<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.clear(color);
        Ok(())
    }

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.view.draw_iter(pixels)
    }
}

impl OriginDimensions for CydFrame<'_> {
    fn size(&self) -> Size {
        self.view.size()
    }
}

impl PixelTarget for CydFrame<'_> {
    fn width(&self) -> usize {
        self.width()
    }

    fn height(&self) -> usize {
        self.height()
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        let stride = self.width();
        self.raw_pixels_mut()[y * stride + x] = Cyd::rgb565(color).into_storage();
    }
}

#[derive(Debug, derive_more::From)]
pub enum CydError {
    Flash(device_envoy_esp::Error),
    DisplayInit(CydDisplayInitError),
    TouchInit(CydTouchInitError),
    DisplayFlush(CydDisplayFlushError),
    TouchUnavailable,
    CalibrationUnavailable,
}

impl Cyd {
    /// Total pixel count of the CYD panel — fixed hardware, independent of orientation.
    pub const SCREEN_PIXELS: usize = SCREEN_PIXELS;

    /// Create [`CydStatic`] storage for a pixel buffer of type `B`.
    ///
    /// Equivalent to `CydStatic::<B>::new()` but namespaced under `Cyd` so all
    /// construction calls share a common prefix.
    ///
    /// ```ignore
    /// static CYD_STATIC: CydStatic<PixelBufferFull> = Cyd::new_static();
    /// ```
    #[must_use]
    pub const fn new_static<B: DynPixelBuffer>() -> CydStatic<B> {
        CydStatic::new()
    }

    // todo000 couldn't this be const and/or inlined and defined elsewhere?
    // todo000 review rgb565 conversion later.
    #[inline]
    pub fn rgb565(color: Rgb888) -> Rgb565 {
        Rgb565::from(color)
    }

    /// Oriented screen size (width, height) for the configured orientation.
    #[must_use]
    pub const fn screen_size(&self) -> Size {
        self.display.size()
    }

    /// Construct a display-only `Cyd` (no touch) that owns its draw buffer,
    /// initializing the buffer from app-provided [`CydStatic`] storage.
    ///
    /// The app picks the buffer type via `B`; `Cyd` owns the init protocol. Use
    /// [`Cyd::frame_mut`] or [`Cyd::full_frame_mut`] to render into and flush the owned buffer.
    pub fn new_display_only<B: DynPixelBuffer>(
        statics: &'static CydStatic<B>,
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_config: CydDisplayConfig,
    ) -> Result<Self, CydError> {
        let pixel_buffer = B::init_static(&statics.pixel_buffer);
        Self::new_with_buffer(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            display_config,
            pixel_buffer,
        )
    }

    /// Lower-level escape hatch: construct a display-only `Cyd` from an already
    /// initialized pixel buffer. Normal apps should prefer [`Cyd::new_display_only`];
    /// this exists for tests, experiments, or unusual storage strategies.
    pub fn new_with_buffer(
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_config: CydDisplayConfig,
        pixel_buffer: &'static mut dyn DynPixelBuffer,
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
            display_config,
            None,
            None,
            None,
            pixel_buffer,
        )
    }

    /// Construct a full `Cyd` with touch + calibration that owns its draw buffer.
    pub fn new<B: DynPixelBuffer>(
        statics: &'static CydStatic<B>,
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_config: CydDisplayConfig,
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
        let pixel_buffer = B::init_static(&statics.pixel_buffer);

        Self::new_inner(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            display_config,
            Some(touch),
            Some(calibration_flash_block),
            Some(calibration_button),
            pixel_buffer,
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
        display_config: CydDisplayConfig,
        touch: Option<CydTouch>,
        calibration_flash_block: Option<FlashBlockEsp>,
        calibration_button: Option<device_envoy_esp::button::ButtonEsp<'static>>,
        pixel_buffer: &'static mut dyn DynPixelBuffer,
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
                display_config,
            )?,
            touch,
            calibration_config,
            calibration_flash_block,
            calibration_button,
            pixel_buffer,
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

    pub fn full_frame_mut(&mut self) -> CydFrame<'_> {
        self.frame_mut(self.screen_size())
    }

    pub fn frame_mut(&mut self, size: Size) -> CydFrame<'_> {
        let view = self
            .pixel_buffer
            .view_mut(size.width as usize, size.height as usize);
        CydFrame {
            display: &mut self.display,
            view,
        }
    }

    pub fn clear(&mut self, color: Rgb565) -> Result<(), CydError> {
        Ok(self.display.clear(color)?)
    }

    pub fn fill_rect(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), CydError> {
        Ok(self.display.fill_rect(rectangle, color)?)
    }

    pub fn fill_contiguous<I>(&mut self, rectangle: Rectangle, pixels: I) -> Result<(), CydError>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        Ok(self.display.fill_contiguous(rectangle, pixels)?)
    }

    pub fn draw_line_segments(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        segments: &[LineSegment],
    ) -> Result<(), CydError> {
        Ok(self
            .display
            .draw_line_segments(bounds, background, segments)?)
    }

    pub fn draw_primitives(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        primitives: &[DrawPrimitive],
    ) -> Result<(), CydError> {
        Ok(self
            .display
            .draw_primitives(bounds, background, primitives)?)
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

    pub fn clear(&mut self, color: Rgb565) -> Result<(), CydError> {
        self.cyd.clear(color)
    }

    pub fn fill_rect(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), CydError> {
        self.cyd.fill_rect(rectangle, color)
    }

    pub fn draw_line_segments(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        segments: &[LineSegment],
    ) -> Result<(), CydError> {
        self.cyd.draw_line_segments(bounds, background, segments)
    }

    pub fn draw_primitives(
        &mut self,
        bounds: Rectangle,
        background: Rgb565,
        primitives: &[DrawPrimitive],
    ) -> Result<(), CydError> {
        self.cyd.draw_primitives(bounds, background, primitives)
    }
}

impl fmt::Debug for Cyd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Cyd").finish_non_exhaustive()
    }
}
