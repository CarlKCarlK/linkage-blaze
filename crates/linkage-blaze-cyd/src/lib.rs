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
    prelude::{Dimensions, DrawTarget, OriginDimensions, Point, Size},
    primitives::Rectangle,
};
use linkage_blaze_core::PixelTarget;
use static_cell::StaticCell;

use buffer::DynPixelBuffer;
pub use buffer::{PixelBuffer, RegionBuffer, RegionView};
pub use calibration::{CalibrationConfig, RawPoint, map_raw_to_screen};
pub use display::{CydDisplayEspFlushError, CydDisplayEspInitError, DISPLAY_SPI_HZ};
use linkage_blaze_cyd_core::{CopySizeError, Cyd, CydFlushError, CydFrame};
// The device abstraction and its neutral support types live in
// `linkage-blaze-cyd-core`; re-export the public surface from this device crate.
pub use linkage_blaze_cyd_core::{
    Cyd as CydDevice, CydFrame as CydFrameTrait, DrawPrimitive, Ellipse, LineSegment, Orientation,
    RegionPixels, SCREEN_HEIGHT, SCREEN_PIXELS, SCREEN_WIDTH, TouchInputEvent, tiling,
};
pub use text::DEFAULT_FONT;
pub use touch::{CydTouchEspInitError, RawTouchEvent, TOUCH_SPI_HZ};

use display::CydDisplayEsp;
use touch::CydTouchEsp;

pub struct CydEsp {
    display: CydDisplayEsp,
    touch: Option<CydTouchEsp>,
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
    display: &'a mut CydDisplayEsp,
    view: RegionView<'a>,
    // Where this frame presents and how large it is: set from the `Rectangle`
    // passed to `frame_mut`, so `flush` needs no separate position argument.
    region: Rectangle,
    // Tile top-left in screen coordinates. Drawing coordinates are translated
    // by this point before reaching the local frame buffer.
    tile_top_left: Point,
    // Default background and foreground colors and font, copied from the owning
    // `CydEsp`, so `clear` and `write_text` can render with the device default style.
    pub(crate) background565: Rgb565,
    pub(crate) foreground565: Rgb565,
    pub(crate) font: &'static MonoFont<'static>,
}

impl<'a> CydFrameEsp<'a> {
    pub fn view_mut(&mut self) -> &mut RegionView<'a> {
        &mut self.view
    }

    /// Fill the frame with the device default background color.
    pub fn clear(&mut self) -> &mut Self {
        self.view.fill(self.background565);
        self
    }

    /// Fill the frame with an explicit color.
    pub fn fill(&mut self, color: Rgb565) -> &mut Self {
        self.view.fill(color);
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

    /// Present this frame's pixels at its region's top-left (set by [`CydDevice::frame_mut`]).
    pub fn flush(&mut self) -> Result<(), CydError> {
        Ok(self
            .display
            .flush_buffer(&self.view, self.region.top_left)?)
    }

    fn local_x(&self, x: i32) -> Option<usize> {
        usize::try_from(x.checked_sub(self.tile_top_left.x)?).ok()
    }

    fn local_y(&self, y: i32) -> Option<usize> {
        usize::try_from(y.checked_sub(self.tile_top_left.y)?).ok()
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
        for Pixel(point, color) in pixels {
            let Some(local_x) = self.local_x(point.x) else {
                continue;
            };
            let Some(local_y) = self.local_y(point.y) else {
                continue;
            };
            if local_x < self.view.width() && local_y < self.view.height() {
                let index = local_y * self.view.width() + local_x;
                self.raw_pixels_mut()[index] = color.into_storage();
            }
        }
        Ok(())
    }
}

impl Dimensions for CydFrameEsp<'_> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.tile_top_left, self.view.size())
    }
}

impl PixelTarget for CydFrameEsp<'_> {
    fn width(&self) -> usize {
        usize::try_from(self.tile_top_left.x)
            .expect("tile top-left x must be non-negative")
            .checked_add(self.width())
            .expect("frame width must fit in usize")
    }

    fn height(&self) -> usize {
        usize::try_from(self.tile_top_left.y)
            .expect("tile top-left y must be non-negative")
            .checked_add(self.height())
            .expect("frame height must fit in usize")
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: Rgb888) {
        let Some(local_x) = self.local_x(x as i32) else {
            return;
        };
        let Some(local_y) = self.local_y(y as i32) else {
            return;
        };
        if local_x >= self.view.width() || local_y >= self.view.height() {
            return;
        }
        let stride = self.view.width();
        self.raw_pixels_mut()[local_y * stride + local_x] = CydEsp::rgb565(color).into_storage();
    }

    /// The frame buffer already stores RGB565, so a decoded image pixel can be
    /// written verbatim with no RGB888 round-trip.
    fn put_pixel_565(&mut self, x: usize, y: usize, rgb565: u16) {
        let Some(local_x) = self.local_x(x as i32) else {
            return;
        };
        let Some(local_y) = self.local_y(y as i32) else {
            return;
        };
        if local_x >= self.view.width() || local_y >= self.view.height() {
            return;
        }
        let stride = self.view.width();
        self.raw_pixels_mut()[local_y * stride + local_x] = rgb565;
    }
}

#[derive(Debug, derive_more::From)]
pub enum CydError {
    Flash(device_envoy_esp::Error),
    DisplayInit(CydDisplayEspInitError),
    TouchInit(CydTouchEspInitError),
    DisplayFlush(CydDisplayEspFlushError),
    TouchUnavailable,
    CalibrationUnavailable,
}

impl CydFlushError for CydError {}

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

    // TODO00 Review whether this helper should remain on `CydEsp`, and whether it
    // can be `const` or otherwise moved to a more appropriate abstraction.
    #[inline]
    pub fn rgb565(color: Rgb888) -> Rgb565 {
        Rgb565::from(color)
    }

    /// Construct a display-only `CydEsp` (no touch) that owns its draw buffer,
    /// initializing the buffer from app-provided [`CydStaticEsp`] storage.
    ///
    /// The app picks the size via `PIXEL_COUNT`; `CydEsp` owns the init protocol. Use
    /// [`CydDevice::frame_mut`] or [`CydDevice::full_frame_mut`] to render into and flush the owned buffer.
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
        let touch = CydTouchEsp::new(
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
        touch: Option<CydTouchEsp>,
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

        let mut display = CydDisplayEsp::new(
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
        display.fill(background565)?;

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

impl Cyd for CydEsp {
    type Error = CydError;
    type Frame<'a> = CydFrameEsp<'a>;

    #[inline]
    fn screen_size(&self) -> Size {
        self.display.size()
    }

    fn background(&self) -> Rgb888 {
        self.background
    }

    fn foreground(&self) -> Rgb888 {
        self.foreground
    }

    fn background_565(&self) -> Rgb565 {
        self.background565
    }

    fn foreground_565(&self) -> Rgb565 {
        self.foreground565
    }

    fn frame_mut_with_tile_top_left(
        &mut self,
        region: Rectangle,
        tile_top_left: Point,
    ) -> CydFrameEsp<'_> {
        self.display.make_frame_with_tile_top_left(
            self.pixel_buffer,
            region,
            tile_top_left,
            self.background565,
            self.foreground565,
            self.font,
        )
    }

    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, CydError> {
        let Some(calibration_config) = self.calibration_config else {
            return Ok(None);
        };
        let Some(touch) = self.touch.as_mut() else {
            return Ok(None);
        };
        Ok(touch
            .read_raw_touch_event()
            .map(|raw_touch_event| match raw_touch_event {
                RawTouchEvent::Down { raw_x, raw_y } => {
                    let (x, y) = map_raw_to_screen(raw_x, raw_y, calibration_config);
                    TouchInputEvent::Down { x, y }
                }
                RawTouchEvent::Move { raw_x, raw_y } => {
                    let (x, y) = map_raw_to_screen(raw_x, raw_y, calibration_config);
                    TouchInputEvent::Move { x, y }
                }
                RawTouchEvent::Up => TouchInputEvent::Up,
            }))
    }

    #[inline]
    fn fill_rectangle(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), CydError> {
        Ok(self.display.fill_rectangle(rectangle, color)?)
    }

    #[inline]
    fn fill_contiguous<I>(&mut self, rectangle: Rectangle, pixels: I) -> Result<(), CydError>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        Ok(self.display.fill_contiguous(rectangle, pixels)?)
    }

    #[inline]
    fn flush_at(&mut self, buffer: &impl RegionPixels, top_left: Point) -> Result<(), CydError> {
        Ok(self.display.flush_buffer(buffer, top_left)?)
    }
}

impl Cyd for CalibratedCydEsp<'_> {
    type Error = CydError;
    type Frame<'a>
        = CydFrameEsp<'a>
    where
        Self: 'a;

    #[inline]
    fn screen_size(&self) -> Size {
        self.cyd.display.size()
    }

    fn background(&self) -> Rgb888 {
        self.cyd.background
    }

    fn foreground(&self) -> Rgb888 {
        self.cyd.foreground
    }

    fn background_565(&self) -> Rgb565 {
        self.cyd.background565
    }

    fn foreground_565(&self) -> Rgb565 {
        self.cyd.foreground565
    }

    fn frame_mut_with_tile_top_left(
        &mut self,
        region: Rectangle,
        tile_top_left: Point,
    ) -> CydFrameEsp<'_> {
        self.cyd.display.make_frame_with_tile_top_left(
            self.cyd.pixel_buffer,
            region,
            tile_top_left,
            self.cyd.background565,
            self.cyd.foreground565,
            self.cyd.font,
        )
    }

    fn read_touch_input(&mut self) -> Result<Option<TouchInputEvent>, CydError> {
        CalibratedCydEsp::read_touch_input(self)
    }

    #[inline]
    fn fill_rectangle(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), CydError> {
        Ok(self.cyd.display.fill_rectangle(rectangle, color)?)
    }

    #[inline]
    fn fill_contiguous<I>(&mut self, rectangle: Rectangle, pixels: I) -> Result<(), CydError>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        Ok(self.cyd.display.fill_contiguous(rectangle, pixels)?)
    }

    #[inline]
    fn flush_at(&mut self, buffer: &impl RegionPixels, top_left: Point) -> Result<(), CydError> {
        Ok(self.cyd.display.flush_buffer(buffer, top_left)?)
    }
}

impl CydFrame for CydFrameEsp<'_> {
    type Error = CydError;

    fn tile_top_left(&self) -> Point {
        self.tile_top_left
    }

    fn region(&self) -> Rectangle {
        self.region
    }

    fn clear(&mut self) -> &mut Self {
        CydFrameEsp::clear(self)
    }

    fn fill(&mut self, color: Rgb565) -> &mut Self {
        CydFrameEsp::fill(self, color)
    }

    fn write_text(&mut self, text: &str) -> &mut Self {
        CydFrameEsp::write_text(self, text)
    }

    fn copy_from_565(&mut self, src: &[u16]) -> Result<(), CopySizeError> {
        let dst = self.raw_pixels_mut();
        if dst.len() != src.len() {
            return Err(CopySizeError {
                src_len: src.len(),
                frame_len: dst.len(),
            });
        }
        dst.copy_from_slice(src);
        Ok(())
    }

    // Flushing the panel over SPI is synchronous, so this future resolves on its
    // first poll. The `async fn` is the device-agnostic frame boundary the
    // render loop awaits; on the MCU it adds no suspension.
    async fn flush(&mut self) -> Result<(), CydError> {
        CydFrameEsp::flush(self)
    }
}
