#![no_std]

mod buffer;
mod calibration;
mod display;
mod text;
mod touch;

use core::{convert::Infallible, fmt};

use device_envoy_esp::{
    button::Button,
    flash_block::{FlashBlock, FlashBlockEsp},
};
use embedded_graphics::{
    Pixel,
    mono_font::MonoFont,
    pixelcolor::{IntoStorage, Rgb565, Rgb888},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    primitives::Rectangle,
};
use linkage_blaze_core::PixelTarget;
use static_cell::StaticCell;

use buffer::DynPixelBuffer;
pub use buffer::{PixelBuffer, RectBuffer, RectPixels, RectView};
pub use calibration::{CalibrationConfig, RawPoint, map_raw_to_screen};
pub use display::{
    CydPanel, CydPanelFlushError, CydPanelInitError, DISPLAY_SPI_HZ, DrawPrimitive, Ellipse,
    LineSegment,
};
// The device abstraction and its neutral support types live in
// `linkage-blaze-cyd-core`; re-export them so existing call sites
// (`linkage_blaze_cyd::{Orientation, tiling, TranslatedDrawTarget, ...}`) keep working.
pub use linkage_blaze_cyd_core::{
    Cyd as CydDevice, CydFrame as CydFrameTrait, Orientation, SCREEN_HEIGHT, SCREEN_PIXELS,
    SCREEN_WIDTH, TouchInputEvent, TranslatedDrawTarget, tiling,
};
pub use text::DEFAULT_FONT;
pub use touch::{CydTouch, CydTouchInitError, RawTouchEvent, TOUCH_SPI_HZ};

pub struct CydEsp {
    display: CydPanel,
    touch: Option<CydTouch>,
    calibration_config: Option<CalibrationConfig>,
    calibration_flash_block: Option<FlashBlockEsp>,
    calibration_button: Option<device_envoy_esp::button::ButtonEsp<'static>>,
    // Every CydEsp owns exactly one draw buffer. Apps that don't draw through it
    // pass a zero-sized buffer (e.g. `CydStaticEsp<0>`).
    pixel_buffer: &'static mut dyn DynPixelBuffer,
    // Default drawing style. Background clears the device at construction and
    // fills every new frame; foreground and font drive `CydFrameEsp::write_text`.
    // The `Rgb565` versions are precomputed so the hot drawing paths skip the
    // per-call conversion.
    background: Rgb888,
    foreground: Rgb888,
    background565: Rgb565,
    foreground565: Rgb565,
    font: &'static MonoFont<'static>,
}

/// Static storage for a [`CydEsp`]-owned pixel buffer.
///
/// The app declares one at file scope and names the workspace pixel count it
/// wants:
///
/// ```ignore
/// static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
/// ```
///
/// The app chooses the pixel count (policy); [`CydEsp::new_display_only`] owns the
/// initialization protocol and the storage details.
pub struct CydStaticEsp<const PIXEL_COUNT: usize> {
    pixel_buffer: StaticCell<PixelBuffer<PIXEL_COUNT>>,
}

impl<const PIXEL_COUNT: usize> CydStaticEsp<PIXEL_COUNT> {
    /// Internal constructor. Apps create storage via [`CydEsp::new_static`] so all
    /// construction goes through the `CydEsp` device abstraction.
    pub(crate) const fn new() -> Self {
        Self {
            pixel_buffer: StaticCell::new(),
        }
    }
}

pub struct CalibratedCydEsp<'a> {
    cyd: &'a mut CydEsp,
    calibration_config: CalibrationConfig,
}

pub struct CydFrameEsp<'a> {
    display: &'a mut CydPanel,
    view: RectView<'a>,
    // Default background and foreground colors and font, copied from the owning
    // `CydEsp`, so `clear` and `write_text` can render with the device default style.
    pub(crate) background565: Rgb565,
    pub(crate) foreground565: Rgb565,
    pub(crate) font: &'static MonoFont<'static>,
}

impl<'a> CydFrameEsp<'a> {
    pub fn view_mut(&mut self) -> &mut RectView<'a> {
        &mut self.view
    }

    /// Fill the frame with the device default background color.
    pub fn clear(&mut self) -> &mut Self {
        self.view.clear(self.background565);
        self
    }

    /// Fill the frame with an explicit color.
    pub fn fill(&mut self, color: Rgb565) -> &mut Self {
        self.view.clear(color);
        self
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

    pub fn flush_at(&mut self, top_left: Point) -> Result<(), CydError> {
        Ok(self.display.flush_buffer(&self.view, top_left)?)
    }
}

impl DrawTarget for CydFrameEsp<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill(color);
        Ok(())
    }

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.view.draw_iter(pixels)
    }
}

impl OriginDimensions for CydFrameEsp<'_> {
    fn size(&self) -> Size {
        self.view.size()
    }
}

impl PixelTarget for CydFrameEsp<'_> {
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
        self.raw_pixels_mut()[y * stride + x] = CydEsp::rgb565(color).into_storage();
    }
}

#[derive(Debug, derive_more::From)]
pub enum CydError {
    Flash(device_envoy_esp::Error),
    DisplayInit(CydPanelInitError),
    TouchInit(CydTouchInitError),
    DisplayFlush(CydPanelFlushError),
    TouchUnavailable,
    CalibrationUnavailable,
}

impl CydEsp {
    /// Total pixel count of the CYD panel — fixed hardware, independent of orientation.
    pub const SCREEN_PIXELS: usize = SCREEN_PIXELS;

    /// Create [`CydStaticEsp`] storage for a `PIXEL_COUNT`-sized draw buffer.
    ///
    /// Equivalent to `CydStaticEsp::<PIXEL_COUNT>::new()` but namespaced under `CydEsp` so
    /// all construction calls share a common prefix.
    ///
    /// ```ignore
    /// static CYD_STATIC: CydStaticEsp<{ CydEsp::SCREEN_PIXELS }> = CydEsp::new_static();
    /// ```
    #[must_use]
    pub const fn new_static<const PIXEL_COUNT: usize>() -> CydStaticEsp<PIXEL_COUNT> {
        CydStaticEsp::new()
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

    /// Construct a display-only `CydEsp` (no touch) that owns its draw buffer,
    /// initializing the buffer from app-provided [`CydStaticEsp`] storage.
    ///
    /// The app picks the size via `PIXEL_COUNT`; `CydEsp` owns the init protocol. Use
    /// [`CydEsp::frame_mut`] or [`CydEsp::full_frame_mut`] to render into and flush the owned buffer.
    pub fn new_display_only<const PIXEL_COUNT: usize>(
        statics: &'static CydStaticEsp<PIXEL_COUNT>,
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        orientation: Orientation,
        background: Rgb888,
        foreground: Rgb888,
        font: &'static MonoFont<'static>,
    ) -> Result<Self, CydError> {
        let pixel_buffer = PixelBuffer::init_static(&statics.pixel_buffer);
        Self::new_inner(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            orientation,
            background,
            foreground,
            font,
            None,
            None,
            None,
            pixel_buffer,
        )
    }

    /// Construct a full `CydEsp` with touch + calibration that owns its draw buffer.
    pub fn new<const PIXEL_COUNT: usize>(
        statics: &'static CydStaticEsp<PIXEL_COUNT>,
        display_spi: impl esp_hal::spi::master::Instance + 'static,
        display_sck_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_mosi_pin: impl esp_hal::gpio::interconnect::PeripheralOutput<'static>,
        display_miso_pin: impl esp_hal::gpio::interconnect::PeripheralInput<'static>,
        display_cs_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_dc_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_rst_pin: impl esp_hal::gpio::OutputPin + 'static,
        display_backlight_pin: impl esp_hal::gpio::OutputPin + 'static,
        orientation: Orientation,
        background: Rgb888,
        foreground: Rgb888,
        font: &'static MonoFont<'static>,
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
        let pixel_buffer = PixelBuffer::init_static(&statics.pixel_buffer);

        Self::new_inner(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            orientation,
            background,
            foreground,
            font,
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
        orientation: Orientation,
        background: Rgb888,
        foreground: Rgb888,
        font: &'static MonoFont<'static>,
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

        let mut display = CydPanel::new(
            display_spi,
            display_sck_pin,
            display_mosi_pin,
            display_miso_pin,
            display_cs_pin,
            display_dc_pin,
            display_rst_pin,
            display_backlight_pin,
            orientation,
        )?;
        // Start every device on a clean background so apps never see boot-time
        // garbage before their first draw.
        let background565 = Self::rgb565(background);
        display.clear(background565)?;

        Ok(Self {
            display,
            touch,
            calibration_config,
            calibration_flash_block,
            calibration_button,
            pixel_buffer,
            background,
            foreground,
            background565,
            foreground565: Self::rgb565(foreground),
            font,
        })
    }

    /// The device default background color (cleared at construction and used to
    /// clear every new frame from [`CydEsp::frame_mut`]).
    #[must_use]
    pub fn background(&self) -> Rgb888 {
        self.background
    }

    /// The device default foreground/text color (used by [`CydFrameEsp::write_text`]).
    #[must_use]
    pub fn foreground(&self) -> Rgb888 {
        self.foreground
    }

    /// The device background color in the native `Rgb565` format (pre-computed).
    #[must_use]
    pub fn background_565(&self) -> Rgb565 {
        self.background565
    }

    /// The device foreground color in the native `Rgb565` format (pre-computed).
    #[must_use]
    pub fn foreground_565(&self) -> Rgb565 {
        self.foreground565
    }

    /// Convert an `Rgb888` color to the device's native `Rgb565` format.
    #[must_use]
    pub fn to_rgb565(&self, color: Rgb888) -> Rgb565 {
        Rgb565::from(color)
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

    pub fn ensure_calibration(&mut self) -> Result<CalibratedCydEsp<'_>, CydError> {
        if self.recalibration_requested() {
            self.calibration_config = None;
        }

        let calibration_config = self
            .calibration_config
            .ok_or(CydError::CalibrationUnavailable)?;

        Ok(CalibratedCydEsp {
            cyd: self,
            calibration_config,
        })
    }

    pub fn read_raw_touch_event(&mut self) -> Option<RawTouchEvent> {
        self.touch.as_mut()?.read_raw_touch_event()
    }

    pub fn flush_at(&mut self, buffer: &impl RectPixels, top_left: Point) -> Result<(), CydError> {
        Ok(self.display.flush_buffer(buffer, top_left)?)
    }

    pub fn full_frame_mut(&mut self) -> CydFrameEsp<'_> {
        self.frame_mut(self.screen_size())
    }

    pub fn frame_mut(&mut self, size: Size) -> CydFrameEsp<'_> {
        let mut view = self
            .pixel_buffer
            .view_mut(size.width as usize, size.height as usize);
        // Every new frame starts cleared to the device background so callers
        // never have to clear it themselves.
        view.clear(self.background565);
        CydFrameEsp {
            display: &mut self.display,
            view,
            background565: self.background565,
            foreground565: self.foreground565,
            font: self.font,
        }
    }

    /// Clear the whole screen to `color`.
    ///
    /// Mirrors embedded-graphics' [`DrawTarget::clear`]. The device is already
    /// cleared to its default background at construction and every frame from
    /// [`CydEsp::frame_mut`] starts cleared, so this is only needed for an explicit
    /// non-default full-screen fill.
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

impl CalibratedCydEsp<'_> {
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

    pub fn flush_at(&mut self, buffer: &impl RectPixels, top_left: Point) -> Result<(), CydError> {
        self.cyd.flush_at(buffer, top_left)
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

impl fmt::Debug for CydEsp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("CydEsp").finish_non_exhaustive()
    }
}

// ── Device-agnostic `Cyd` trait impls ─────────────────────────────────────────
//
// These let platform-neutral code (`linkage-blaze-cyd-core` consumers) drive the
// concrete esp `CydEsp` through the `Cyd`/`CydFrame` traits without naming any
// esp type.

impl linkage_blaze_cyd_core::Cyd for CydEsp {
    type Error = CydError;
    type Frame<'a> = CydFrameEsp<'a>;

    fn screen_size(&self) -> Size {
        CydEsp::screen_size(self)
    }

    fn frame_mut(&mut self, size: Size) -> CydFrameEsp<'_> {
        CydEsp::frame_mut(self, size)
    }

    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, CydError> {
        let Some(calibration_config) = self.calibration_config else {
            return Ok(None);
        };
        let Some(touch) = self.touch.as_mut() else {
            return Ok(None);
        };
        Ok(touch.read_raw_touch_event().map(|raw_touch_event| {
            match raw_touch_event {
                RawTouchEvent::Down { raw_x, raw_y } => {
                    let (x, y) = map_raw_to_screen(raw_x, raw_y, calibration_config);
                    TouchInputEvent::Down { x, y }
                }
                RawTouchEvent::Move { raw_x, raw_y } => {
                    let (x, y) = map_raw_to_screen(raw_x, raw_y, calibration_config);
                    TouchInputEvent::Move { x, y }
                }
                RawTouchEvent::Up => TouchInputEvent::Up,
            }
        }))
    }
}

impl linkage_blaze_cyd_core::CydFrame for CydFrameEsp<'_> {
    type Error = CydError;

    fn write_text(&mut self, text: &str) -> &mut Self {
        CydFrameEsp::write_text(self, text)
    }

    fn flush_at(&mut self, top_left: Point) -> Result<(), CydError> {
        CydFrameEsp::flush_at(self, top_left)
    }
}
