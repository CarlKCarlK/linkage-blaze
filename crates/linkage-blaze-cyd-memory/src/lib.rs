use core::{
    cell::{Cell, RefCell},
    convert::Infallible,
    future::ready,
    ops::Range,
};
use std::{fs, path::Path, rc::Rc, vec::Vec};

use device_envoy_core::{
    button::{__ButtonMonitor, Button},
    flash_block::{FlashBlock, FlashBlockError, FlashDevice, clear_block, load_block, save_block},
};
use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{IntoStorage, Rgb565, raw::RawU16},
    prelude::{Dimensions, DrawTarget, Point, Size},
    primitives::Rectangle,
    text::{Baseline, Text},
};
use linkage_blaze_core::{PixelTarget, Rgb888, RgbColor, WebColors, rgb888_from_rgb565};
use linkage_blaze_cyd_core::{
    CopySizeError, Cyd, CydDisplay, CydFlushError, CydFrame, CydRawTouch, CydTouch, RawTouchEvent,
    RegionPixels, TouchEvent,
};
use serde::{Deserialize, Serialize};

const DEFAULT_FRAME_BUDGET: usize = 1000;
const FLASH_BLOCK_SIZE: usize = 4096;
const FLASH_BLOCK_OFFSET: u32 = 0;
const FLASH_ERASED_BYTE: u8 = 0xFF;
const TGA_HEADER_SIZE: usize = 18;
const TGA_PIXEL_BYTES: usize = 3;

#[derive(Clone)]
pub struct MemoryFrameClock {
    frame_index: Rc<Cell<usize>>,
}

impl MemoryFrameClock {
    #[must_use]
    pub fn frame_index(&self) -> usize {
        self.frame_index.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCydError {
    OutOfFrames,
}

impl CydFlushError for MemoryCydError {}

pub struct MemoryCyd {
    size: Size,
    background: Rgb888,
    foreground: Rgb888,
    background565: Rgb565,
    foreground565: Rgb565,
    framebuffer: Vec<u16>,
    flush_count: usize,
    last_flush_region: Option<Rectangle>,
    frame_budget: usize,
    raw_touch_script: RefCell<FrameScript<RawTouchEvent>>,
    touch_script: RefCell<FrameScript<TouchEvent>>,
    frame_clock: MemoryFrameClock,
}

pub struct MemoryDisplayPart<'a> {
    size: Size,
    background: Rgb888,
    foreground: Rgb888,
    background565: Rgb565,
    foreground565: Rgb565,
    framebuffer: &'a mut Vec<u16>,
    flush_count: &'a mut usize,
    last_flush_region: &'a mut Option<Rectangle>,
    frame_budget: usize,
    raw_touch_script: &'a RefCell<FrameScript<RawTouchEvent>>,
    touch_script: &'a RefCell<FrameScript<TouchEvent>>,
    frame_clock: MemoryFrameClock,
}

pub struct MemoryTouchPart<'a> {
    touch_script: &'a RefCell<FrameScript<TouchEvent>>,
}

pub struct MemoryFrame<'a> {
    framebuffer: &'a mut Vec<u16>,
    flush_count: &'a mut usize,
    last_flush_region: &'a mut Option<Rectangle>,
    frame_budget: usize,
    raw_touch_script: &'a RefCell<FrameScript<RawTouchEvent>>,
    touch_script: &'a RefCell<FrameScript<TouchEvent>>,
    frame_clock: MemoryFrameClock,
    screen_size: Size,
    region: Rectangle,
    tile_top_left: Point,
    background565: Rgb565,
    foreground565: Rgb565,
    pixels: Vec<u16>,
}

struct FrameScript<Event> {
    current_frame: Vec<Event>,
    future_frames: Vec<Vec<Event>>,
    current_read_index: usize,
}

pub struct MemoryFlashBlock {
    memory_flash_device: MemoryFlashDevice,
    save_count: usize,
}

struct MemoryFlashDevice {
    bytes: [u8; FLASH_BLOCK_SIZE],
}

pub struct MemoryButton {
    pressed: bool,
    pressed_frames: Vec<(usize, bool)>,
    frame_clock: Option<MemoryFrameClock>,
}

impl MemoryCyd {
    #[must_use]
    pub fn new(size: Size, background: Rgb888, foreground: Rgb888) -> Self {
        let background565 = Rgb565::from(background);
        let pixel_count = size.width as usize * size.height as usize;
        Self {
            size,
            background,
            foreground,
            background565,
            foreground565: Rgb565::from(foreground),
            framebuffer: vec![background565.into_storage(); pixel_count],
            flush_count: 0,
            last_flush_region: None,
            frame_budget: DEFAULT_FRAME_BUDGET,
            raw_touch_script: RefCell::new(FrameScript::default()),
            touch_script: RefCell::new(FrameScript::default()),
            frame_clock: MemoryFrameClock {
                frame_index: Rc::new(Cell::new(0)),
            },
        }
    }

    #[must_use]
    pub fn classic() -> Self {
        Self::new(Size::new(320, 240), Rgb888::CSS_BLACK, Rgb888::CSS_WHITE)
    }

    pub fn set_frame_budget(&mut self, frame_budget: usize) {
        self.frame_budget = frame_budget;
    }

    #[must_use]
    pub fn frame_clock(&self) -> MemoryFrameClock {
        self.frame_clock.clone()
    }

    #[must_use]
    pub fn memory_button(&self) -> MemoryButton {
        MemoryButton::with_frame_clock(self.frame_clock())
    }

    pub fn script_raw_frames(&mut self, raw_touch_frames: &[&[RawTouchEvent]]) {
        self.raw_touch_script
            .borrow_mut()
            .replace_frames(raw_touch_frames);
    }

    pub fn script_raw_frames_owned(&mut self, raw_touch_frames: Vec<Vec<RawTouchEvent>>) {
        self.raw_touch_script
            .borrow_mut()
            .replace_owned_frames(raw_touch_frames);
    }

    pub fn script_touch_frames(&mut self, touch_frames: &[&[TouchEvent]]) {
        self.touch_script.borrow_mut().replace_frames(touch_frames);
    }

    pub fn script_touch_frames_owned(&mut self, touch_frames: Vec<Vec<TouchEvent>>) {
        self.touch_script
            .borrow_mut()
            .replace_owned_frames(touch_frames);
    }

    pub fn script_idle_frames(&mut self, idle_frame_count: usize) {
        self.raw_touch_script
            .borrow_mut()
            .push_idle_frames(idle_frame_count);
        self.touch_script
            .borrow_mut()
            .push_idle_frames(idle_frame_count);
    }

    pub fn push_raw_touch_event(&mut self, raw_touch_event: RawTouchEvent) {
        self.raw_touch_script
            .borrow_mut()
            .push_current_frame_event(raw_touch_event);
    }

    pub fn push_touch_event(&mut self, touch_event: TouchEvent) {
        self.touch_script
            .borrow_mut()
            .push_current_frame_event(touch_event);
    }

    pub fn script_tap(&mut self, raw_point: linkage_blaze_cyd_core::RawPoint) {
        self.push_raw_touch_event(RawTouchEvent::Down {
            raw_x: raw_point.x,
            raw_y: raw_point.y,
        });
        for _discarded_sample_index in 0..4 {
            self.push_raw_touch_event(RawTouchEvent::Move {
                raw_x: raw_point.x,
                raw_y: raw_point.y,
            });
        }
        for _usable_sample_index in 0..3 {
            self.push_raw_touch_event(RawTouchEvent::Move {
                raw_x: raw_point.x,
                raw_y: raw_point.y,
            });
        }
        self.push_raw_touch_event(RawTouchEvent::Up);
    }

    #[must_use]
    pub fn flush_count(&self) -> usize {
        self.flush_count
    }

    #[must_use]
    pub fn last_flush_region(&self) -> Option<Rectangle> {
        self.last_flush_region
    }

    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Rgb565 {
        assert!(
            x < self.size.width as usize,
            "x must stay within the screen"
        );
        assert!(
            y < self.size.height as usize,
            "y must stay within the screen"
        );
        let stride = self.size.width as usize;
        Rgb565::from(RawU16::new(self.framebuffer[y * stride + x]))
    }

    #[must_use]
    pub fn framebuffer(&self) -> &[u16] {
        &self.framebuffer
    }

    pub fn write_framebuffer_tga(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let width = self.size.width as usize;
        let height = self.size.height as usize;
        let pixel_bytes = width
            .checked_mul(height)
            .and_then(|pixel_count| pixel_count.checked_mul(TGA_PIXEL_BYTES))
            .expect("TGA buffer length must fit in usize");
        let mut bytes = Vec::with_capacity(TGA_HEADER_SIZE + pixel_bytes);
        bytes.resize(TGA_HEADER_SIZE, 0);
        bytes[2] = 2;
        bytes[12..14].copy_from_slice(&(self.size.width as u16).to_le_bytes());
        bytes[14..16].copy_from_slice(&(self.size.height as u16).to_le_bytes());
        bytes[16] = 24;
        bytes[17] = 0x20;
        for pixel in &self.framebuffer {
            let color = rgb888_from_rgb565(*pixel);
            bytes.push(color.b());
            bytes.push(color.g());
            bytes.push(color.r());
        }
        fs::write(path, bytes)
    }
}

impl Default for MemoryCyd {
    fn default() -> Self {
        Self::classic()
    }
}

impl Cyd for MemoryCyd {
    type Error = MemoryCydError;
    type Display<'a> = MemoryDisplayPart<'a>;
    type Touch<'a> = MemoryTouchPart<'a>;

    fn parts(&mut self) -> (Self::Display<'_>, Self::Touch<'_>) {
        let frame_clock = self.frame_clock();
        (
            MemoryDisplayPart {
                size: self.size,
                background: self.background,
                foreground: self.foreground,
                background565: self.background565,
                foreground565: self.foreground565,
                framebuffer: &mut self.framebuffer,
                flush_count: &mut self.flush_count,
                last_flush_region: &mut self.last_flush_region,
                frame_budget: self.frame_budget,
                raw_touch_script: &self.raw_touch_script,
                touch_script: &self.touch_script,
                frame_clock,
            },
            MemoryTouchPart {
                touch_script: &self.touch_script,
            },
        )
    }
}

impl CydRawTouch for MemoryCyd {
    type Error = MemoryCydError;

    fn read_raw_touch_event(&mut self) -> Result<Option<RawTouchEvent>, Self::Error> {
        Ok(self.raw_touch_script.borrow_mut().pop_current_frame_event())
    }
}

impl CydDisplay for MemoryDisplayPart<'_> {
    type Error = MemoryCydError;
    type Frame<'a>
        = MemoryFrame<'a>
    where
        Self: 'a;

    fn screen_size(&self) -> Size {
        self.size
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
    ) -> Self::Frame<'_> {
        let pixel_count = region.size.width as usize * region.size.height as usize;
        MemoryFrame {
            framebuffer: self.framebuffer,
            flush_count: self.flush_count,
            last_flush_region: self.last_flush_region,
            frame_budget: self.frame_budget,
            raw_touch_script: self.raw_touch_script,
            touch_script: self.touch_script,
            frame_clock: self.frame_clock.clone(),
            screen_size: self.size,
            region,
            tile_top_left,
            background565: self.background565,
            foreground565: self.foreground565,
            pixels: vec![self.background565.into_storage(); pixel_count],
        }
    }

    fn fill_rectangle(&mut self, rectangle: Rectangle, color: Rgb565) -> Result<(), Self::Error> {
        fill_rectangle_in_framebuffer(self.framebuffer, self.size, rectangle, color.into_storage());
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, rectangle: Rectangle, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        fill_contiguous_in_framebuffer(
            self.framebuffer,
            self.size,
            rectangle,
            pixels.into_iter().map(IntoStorage::into_storage),
        );
        Ok(())
    }
}

impl CydTouch for MemoryTouchPart<'_> {
    type Error = MemoryCydError;

    fn read(&mut self) -> Result<Option<TouchEvent>, Self::Error> {
        Ok(self.touch_script.borrow_mut().pop_current_frame_event())
    }
}

impl MemoryFrame<'_> {
    fn width(&self) -> usize {
        self.region.size.width as usize
    }

    fn height(&self) -> usize {
        self.region.size.height as usize
    }

    fn local_x(&self, x: i32) -> Option<usize> {
        usize::try_from(x.checked_sub(self.tile_top_left.x)?).ok()
    }

    fn local_y(&self, y: i32) -> Option<usize> {
        usize::try_from(y.checked_sub(self.tile_top_left.y)?).ok()
    }

    fn flush_now(&mut self) -> Result<(), MemoryCydError> {
        if *self.flush_count >= self.frame_budget {
            return Err(MemoryCydError::OutOfFrames);
        }

        blit_frame_to_screen(
            self.framebuffer,
            self.screen_size,
            self.region,
            &self.pixels,
        );
        *self.last_flush_region = Some(self.region);
        *self.flush_count += 1;
        self.raw_touch_script.borrow_mut().advance_frame();
        self.touch_script.borrow_mut().advance_frame();
        self.frame_clock
            .frame_index
            .set(self.frame_clock.frame_index.get() + 1);
        Ok(())
    }
}

impl DrawTarget for MemoryFrame<'_> {
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
            if local_x >= self.width() || local_y >= self.height() {
                continue;
            }
            let stride = self.width();
            self.pixels[local_y * stride + local_x] = color.into_storage();
        }
        Ok(())
    }
}

impl Dimensions for MemoryFrame<'_> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.tile_top_left, self.region.size)
    }
}

impl PixelTarget for MemoryFrame<'_> {
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
        self.put_pixel_565(x, y, Rgb565::from(color).into_storage());
    }

    fn put_pixel_565(&mut self, x: usize, y: usize, rgb565: u16) {
        let Some(local_x) = self.local_x(x as i32) else {
            return;
        };
        let Some(local_y) = self.local_y(y as i32) else {
            return;
        };
        if local_x >= self.width() || local_y >= self.height() {
            return;
        }
        let stride = self.width();
        self.pixels[local_y * stride + local_x] = rgb565;
    }
}

impl RegionPixels for MemoryFrame<'_> {
    fn width(&self) -> usize {
        self.width()
    }

    fn height(&self) -> usize {
        self.height()
    }

    fn raw_pixels(&self) -> &[u16] {
        &self.pixels
    }
}

impl CydFrame for MemoryFrame<'_> {
    type Error = MemoryCydError;

    fn tile_top_left(&self) -> Point {
        self.tile_top_left
    }

    fn region(&self) -> Rectangle {
        self.region
    }

    fn clear(&mut self) -> &mut Self {
        self.fill(self.background565)
    }

    fn fill(&mut self, color: Rgb565) -> &mut Self {
        self.pixels.fill(color.into_storage());
        self
    }

    fn write_text(&mut self, text: &str) -> &mut Self {
        Text::with_baseline(
            text,
            Point::zero(),
            MonoTextStyle::new(&FONT_6X10, self.foreground565),
            Baseline::Top,
        )
        .draw(self)
        .expect("drawing into an Infallible memory frame cannot fail");
        self
    }

    fn copy_from_565(&mut self, src: &[u16]) -> Result<(), CopySizeError> {
        if self.pixels.len() != src.len() {
            return Err(CopySizeError {
                src_len: src.len(),
                frame_len: self.pixels.len(),
            });
        }
        self.pixels.copy_from_slice(src);
        Ok(())
    }

    fn flush(
        &mut self,
    ) -> impl core::future::Future<Output = Result<(), <Self as CydFrame>::Error>> {
        ready(self.flush_now())
    }
}

impl<Event> Default for FrameScript<Event> {
    fn default() -> Self {
        Self {
            current_frame: Vec::new(),
            future_frames: Vec::new(),
            current_read_index: 0,
        }
    }
}

impl<Event: Clone> FrameScript<Event> {
    fn replace_frames(&mut self, frames: &[&[Event]]) {
        self.current_frame.clear();
        self.future_frames.clear();
        self.current_read_index = 0;
        if let Some((first_frame, remaining_frames)) = frames.split_first() {
            self.current_frame = first_frame.to_vec();
            self.future_frames = remaining_frames
                .iter()
                .map(|frame| frame.to_vec())
                .collect();
        }
    }

    fn push_idle_frames(&mut self, idle_frame_count: usize) {
        self.future_frames
            .extend((0..idle_frame_count).map(|_| Vec::new()));
    }

    fn replace_owned_frames(&mut self, mut frames: Vec<Vec<Event>>) {
        self.current_frame.clear();
        self.future_frames.clear();
        self.current_read_index = 0;
        if frames.is_empty() {
            return;
        }
        self.current_frame = frames.remove(0);
        self.future_frames = frames;
    }

    fn push_current_frame_event(&mut self, event: Event) {
        self.current_frame.push(event);
    }

    fn pop_current_frame_event(&mut self) -> Option<Event> {
        let event = self.current_frame.get(self.current_read_index).cloned();
        if event.is_some() {
            self.current_read_index += 1;
        }
        event
    }

    fn advance_frame(&mut self) {
        if self.current_read_index >= self.current_frame.len() {
            if let Some(next_frame) = self.future_frames.first().cloned() {
                self.current_frame = next_frame;
                self.future_frames.remove(0);
            } else {
                self.current_frame.clear();
            }
            self.current_read_index = 0;
            return;
        }

        self.current_frame.drain(0..self.current_read_index);
        self.current_read_index = 0;
    }
}

impl MemoryFlashBlock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            memory_flash_device: MemoryFlashDevice::new(),
            save_count: 0,
        }
    }

    #[must_use]
    pub fn with_value<T>(value: &T) -> Self
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let mut memory_flash_block = Self::new();
        memory_flash_block
            .save(value)
            .expect("saving a small in-memory flash value should succeed");
        memory_flash_block
    }

    #[must_use]
    pub fn with_raw_bytes(bytes: &[u8]) -> Self {
        let mut memory_flash_block = Self::new();
        memory_flash_block
            .memory_flash_device
            .write_raw_bytes(bytes);
        memory_flash_block
    }

    #[must_use]
    pub fn save_count(&self) -> usize {
        self.save_count
    }
}

impl Default for MemoryFlashBlock {
    fn default() -> Self {
        Self::new()
    }
}

impl FlashBlock for MemoryFlashBlock {
    type Error = FlashBlockError<Infallible>;

    fn load<T>(&mut self) -> Result<Option<T>, Self::Error>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        match load_block::<FLASH_BLOCK_SIZE, T, _>(
            &mut self.memory_flash_device,
            FLASH_BLOCK_OFFSET,
        ) {
            Ok(value) => Ok(value),
            Err(FlashBlockError::StorageCorrupted | FlashBlockError::FormatError) => Ok(None),
            Err(FlashBlockError::Io(infallible)) => match infallible {},
        }
    }

    fn save<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        save_block::<FLASH_BLOCK_SIZE, _, _>(
            &mut self.memory_flash_device,
            FLASH_BLOCK_OFFSET,
            value,
        )?;
        self.save_count += 1;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        clear_block::<FLASH_BLOCK_SIZE, _>(&mut self.memory_flash_device, FLASH_BLOCK_OFFSET)
    }
}

impl MemoryFlashDevice {
    fn new() -> Self {
        Self {
            bytes: [FLASH_ERASED_BYTE; FLASH_BLOCK_SIZE],
        }
    }

    fn checked_range(&self, offset: u32, len: usize) -> Range<usize> {
        let start = usize::try_from(offset).expect("flash offset must fit in usize");
        let end = start
            .checked_add(len)
            .expect("flash range must fit in usize");
        assert!(
            end <= FLASH_BLOCK_SIZE,
            "flash range must stay in the block"
        );
        start..end
    }

    fn write_raw_bytes(&mut self, bytes: &[u8]) {
        self.bytes.fill(FLASH_ERASED_BYTE);
        let len = bytes.len().min(FLASH_BLOCK_SIZE);
        self.bytes[..len].copy_from_slice(&bytes[..len]);
    }
}

impl FlashDevice for MemoryFlashDevice {
    type Error = Infallible;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let checked_range = self.checked_range(offset, bytes.len());
        bytes.copy_from_slice(&self.bytes[checked_range]);
        Ok(())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let checked_range = self.checked_range(offset, bytes.len());
        self.bytes[checked_range].copy_from_slice(bytes);
        Ok(())
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        let len = usize::try_from(to.saturating_sub(from)).expect("flash erase length fits usize");
        let checked_range = self.checked_range(from, len);
        self.bytes[checked_range].fill(FLASH_ERASED_BYTE);
        Ok(())
    }
}

impl MemoryButton {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pressed: false,
            pressed_frames: Vec::new(),
            frame_clock: None,
        }
    }

    #[must_use]
    pub fn with_frame_clock(frame_clock: MemoryFrameClock) -> Self {
        Self {
            pressed: false,
            pressed_frames: Vec::new(),
            frame_clock: Some(frame_clock),
        }
    }

    pub fn set_pressed(&mut self, pressed: bool) {
        self.pressed = pressed;
    }

    pub fn set_pressed_for_frame(&mut self, frame_index: usize, pressed: bool) {
        if let Some(existing_state) = self
            .pressed_frames
            .iter_mut()
            .find(|(existing_frame_index, _pressed_state)| *existing_frame_index == frame_index)
        {
            existing_state.1 = pressed;
            return;
        }
        self.pressed_frames.push((frame_index, pressed));
    }

    fn current_pressed_state(&self) -> bool {
        let Some(frame_clock) = &self.frame_clock else {
            return self.pressed;
        };
        let frame_index = frame_clock.frame_index();
        self.pressed_frames
            .iter()
            .find_map(|(pressed_frame_index, pressed)| {
                (*pressed_frame_index == frame_index).then_some(*pressed)
            })
            .unwrap_or(self.pressed)
    }
}

impl Default for MemoryButton {
    fn default() -> Self {
        Self::new()
    }
}

impl __ButtonMonitor for MemoryButton {
    fn is_pressed_raw(&self) -> bool {
        self.current_pressed_state()
    }

    async fn wait_until_pressed_state(&mut self, _pressed: bool) {}
}

impl Button for MemoryButton {}

fn fill_rectangle_in_framebuffer(
    framebuffer: &mut [u16],
    screen_size: Size,
    rectangle: Rectangle,
    color: u16,
) {
    let clipped_rectangle = rectangle.intersection(&Rectangle::new(Point::zero(), screen_size));
    if clipped_rectangle.size.width == 0 || clipped_rectangle.size.height == 0 {
        return;
    }
    let stride = screen_size.width as usize;
    for y in clipped_rectangle.top_left.y
        ..clipped_rectangle.top_left.y + clipped_rectangle.size.height as i32
    {
        for x in clipped_rectangle.top_left.x
            ..clipped_rectangle.top_left.x + clipped_rectangle.size.width as i32
        {
            let index = y as usize * stride + x as usize;
            framebuffer[index] = color;
        }
    }
}

fn fill_contiguous_in_framebuffer<I>(
    framebuffer: &mut [u16],
    screen_size: Size,
    rectangle: Rectangle,
    pixels: I,
) where
    I: IntoIterator<Item = u16>,
{
    if rectangle.size.width == 0 || rectangle.size.height == 0 {
        return;
    }
    let stride = screen_size.width as usize;
    for (pixel_index, pixel) in pixels.into_iter().enumerate() {
        let local_x = pixel_index % rectangle.size.width as usize;
        let local_y = pixel_index / rectangle.size.width as usize;
        if local_y >= rectangle.size.height as usize {
            break;
        }
        let x = rectangle.top_left.x + local_x as i32;
        let y = rectangle.top_left.y + local_y as i32;
        if x < 0 || y < 0 || x >= screen_size.width as i32 || y >= screen_size.height as i32 {
            continue;
        }
        framebuffer[y as usize * stride + x as usize] = pixel;
    }
}

fn blit_frame_to_screen(
    framebuffer: &mut [u16],
    screen_size: Size,
    region: Rectangle,
    pixels: &[u16],
) {
    fill_contiguous_in_framebuffer(framebuffer, screen_size, region, pixels.iter().copied());
}

#[cfg(test)]
mod tests {
    use super::{MemoryCyd, MemoryCydError, MemoryFlashBlock};
    use device_envoy_core::flash_block::FlashBlock;
    use embedded_graphics::{
        Pixel,
        pixelcolor::{IntoStorage, Rgb565, WebColors},
        prelude::{DrawTarget, Point, Size},
        primitives::Rectangle,
    };
    use futures_executor::block_on;
    use linkage_blaze_cyd_core::{
        CalibrationConfig, CalibrationCorner, Cyd, CydDisplay, CydFrame, CydRawTouch,
        EnsureCalibrationError, EnsureCalibrationOutcome, RawPoint, RawTouchEvent, RegionPixels,
        calibration_corner_center, calibration_verify_target_center, distort_demo_screen_to_raw,
        ensure_calibration,
    };
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
    struct DemoValue {
        count: u16,
    }

    const CAPTURE_ACK_EXTRA_IDLE_FRAMES: usize = 8;
    const REJECTED_RESTART_IDLE_FRAMES: usize = 30;
    const VERIFY_TIMEOUT_EXTRA_IDLE_FRAMES: usize = 99;

    #[test]
    fn fresh_frame_starts_cleared_to_background() {
        let mut memory_cyd = MemoryCyd::classic();
        let (mut display, _touch) = memory_cyd.parts();
        let frame = display.frame_mut(Rectangle::new(Point::new(3, 4), Size::new(2, 2)));
        assert_eq!(frame.raw_pixels(), &[Rgb565::CSS_BLACK.into_storage(); 4]);
    }

    #[test]
    fn draw_target_pixel_flushes_to_screen_coordinate() {
        let mut memory_cyd = MemoryCyd::classic();
        {
            let (mut display, _touch) = memory_cyd.parts();
            let mut frame = display.frame_mut_with_tile_top_left(
                Rectangle::new(Point::new(10, 20), Size::new(4, 3)),
                Point::new(10, 20),
            );
            frame
                .draw_iter([Pixel(Point::new(11, 21), Rgb565::CSS_RED)])
                .expect("drawing into memory frame should succeed");
            block_on(frame.flush()).expect("flush should succeed");
        }
        assert_eq!(memory_cyd.pixel(11, 21), Rgb565::CSS_RED);
        assert_eq!(
            memory_cyd.last_flush_region(),
            Some(Rectangle::new(Point::new(10, 20), Size::new(4, 3)))
        );
    }

    #[test]
    fn fill_rectangle_clips_to_screen_edges() {
        let mut memory_cyd = MemoryCyd::new(
            Size::new(4, 4),
            linkage_blaze_core::Rgb888::CSS_BLACK,
            linkage_blaze_core::Rgb888::CSS_WHITE,
        );
        {
            let (mut display, _touch) = memory_cyd.parts();
            display
                .fill_rectangle(
                    Rectangle::new(Point::new(-1, -1), Size::new(3, 3)),
                    Rgb565::CSS_GREEN,
                )
                .expect("fill_rectangle should succeed");
            display
                .fill_rectangle(
                    Rectangle::new(Point::new(10, 10), Size::new(2, 2)),
                    Rgb565::CSS_RED,
                )
                .expect("off-screen fill_rectangle should stay a no-op");
        }
        assert_eq!(memory_cyd.pixel(0, 0), Rgb565::CSS_GREEN);
        assert_eq!(memory_cyd.pixel(1, 1), Rgb565::CSS_GREEN);
        assert_eq!(memory_cyd.pixel(3, 3), Rgb565::CSS_BLACK);
    }

    #[test]
    fn raw_touch_frames_drain_then_advance_after_flush() {
        let mut memory_cyd = MemoryCyd::classic();
        let first_frame = [
            RawTouchEvent::Down { raw_x: 1, raw_y: 2 },
            RawTouchEvent::Up,
        ];
        let second_frame = [RawTouchEvent::Down { raw_x: 3, raw_y: 4 }];
        memory_cyd.script_raw_frames(&[&first_frame, &second_frame]);

        assert_eq!(
            memory_cyd
                .read_raw_touch_event()
                .expect("read should succeed"),
            Some(RawTouchEvent::Down { raw_x: 1, raw_y: 2 })
        );
        assert_eq!(
            memory_cyd
                .read_raw_touch_event()
                .expect("read should succeed"),
            Some(RawTouchEvent::Up)
        );
        assert_eq!(
            memory_cyd
                .read_raw_touch_event()
                .expect("read should succeed"),
            None
        );

        {
            let (mut display, _touch) = memory_cyd.parts();
            let mut frame = display.full_frame_mut();
            block_on(frame.flush()).expect("flush should succeed");
        }

        assert_eq!(memory_cyd.flush_count(), 1);
        assert_eq!(
            memory_cyd
                .read_raw_touch_event()
                .expect("read should succeed"),
            Some(RawTouchEvent::Down { raw_x: 3, raw_y: 4 })
        );
    }

    #[test]
    fn flush_budget_returns_out_of_frames() {
        let mut memory_cyd = MemoryCyd::classic();
        memory_cyd.set_frame_budget(1);
        {
            let (mut display, _touch) = memory_cyd.parts();
            let mut frame = display.full_frame_mut();
            block_on(frame.flush()).expect("first flush should succeed");
        }
        {
            let (mut display, _touch) = memory_cyd.parts();
            let mut frame = display.full_frame_mut();
            let error = block_on(frame.flush()).expect_err("second flush should hit frame budget");
            assert_eq!(error, MemoryCydError::OutOfFrames);
        }
        assert_eq!(memory_cyd.flush_count(), 1);
    }

    #[test]
    fn memory_flash_block_round_trips_and_handles_corruption() {
        let mut memory_flash_block = MemoryFlashBlock::new();
        memory_flash_block
            .save(&DemoValue { count: 7 })
            .expect("save should succeed");
        assert_eq!(
            memory_flash_block
                .load::<DemoValue>()
                .expect("load should succeed"),
            Some(DemoValue { count: 7 })
        );

        let mut corrupt_flash_block = MemoryFlashBlock::with_raw_bytes(&[1, 2, 3, 4]);
        assert_eq!(
            corrupt_flash_block
                .load::<DemoValue>()
                .expect("corrupt load should degrade to None"),
            None
        );

        memory_flash_block.clear().expect("clear should succeed");
        assert_eq!(
            memory_flash_block
                .load::<DemoValue>()
                .expect("load should succeed"),
            None
        );
    }

    #[test]
    fn ensure_calibration_happy_path_saves_predictable_config() {
        let mut memory_cyd = MemoryCyd::classic();
        let mut memory_flash_block = MemoryFlashBlock::new();
        let mut memory_button = memory_cyd.memory_button();
        let raw_points = script_happy_path(&mut memory_cyd);

        let outcome = block_on(ensure_calibration(
            &mut memory_cyd,
            &mut memory_flash_block,
            &mut memory_button,
            Some("saved"),
        ))
        .expect("happy-path calibration should succeed");

        let EnsureCalibrationOutcome::Saved(calibration_config) = outcome else {
            panic!("happy-path calibration should save a new config");
        };
        assert_eq!(memory_flash_block.save_count(), 1);

        let saved_config = memory_flash_block
            .load::<CalibrationConfig>()
            .expect("saved config should deserialize")
            .expect("saved config should exist");
        assert_eq!(saved_config, calibration_config);

        for (raw_point, calibration_corner) in raw_points.into_iter().zip([
            CalibrationCorner::UpperLeft,
            CalibrationCorner::UpperRight,
            CalibrationCorner::LowerRight,
            CalibrationCorner::LowerLeft,
        ]) {
            let expected_screen_point = calibration_corner_center(calibration_corner);
            let (mapped_x, mapped_y) = saved_config.map_raw_to_screen(raw_point.x, raw_point.y);
            assert!(
                (mapped_x - expected_screen_point.x as f32).abs() <= 1.0,
                "mapped_x={mapped_x} expected_x={}",
                expected_screen_point.x
            );
            assert!(
                (mapped_y - expected_screen_point.y as f32).abs() <= 1.0,
                "mapped_y={mapped_y} expected_y={}",
                expected_screen_point.y
            );
        }
        assert!(memory_cyd.flush_count() > 0);
        assert_eq!(
            memory_cyd.last_flush_region(),
            Some(Rectangle::new(Point::zero(), Size::new(320, 240)))
        );
    }

    #[test]
    fn ensure_calibration_uses_preloaded_flash_without_flushing() {
        let saved_config = CalibrationConfig::new(1.0, 0.0, 2.0, 0.0, 1.0, 3.0);
        let mut memory_cyd = MemoryCyd::classic();
        memory_cyd.push_raw_touch_event(RawTouchEvent::Down { raw_x: 7, raw_y: 9 });
        let mut memory_flash_block = MemoryFlashBlock::with_value(&saved_config);
        let mut memory_button = memory_cyd.memory_button();

        let outcome = block_on(ensure_calibration(
            &mut memory_cyd,
            &mut memory_flash_block,
            &mut memory_button,
            None,
        ))
        .expect("preloaded calibration should load");

        let EnsureCalibrationOutcome::Loaded(loaded_config) = outcome else {
            panic!("preloaded flash should skip the calibration flow");
        };
        assert_eq!(loaded_config, saved_config);
        assert_eq!(memory_cyd.flush_count(), 0);
        assert_eq!(
            memory_cyd
                .read_raw_touch_event()
                .expect("touch read should succeed"),
            Some(RawTouchEvent::Down { raw_x: 7, raw_y: 9 })
        );
    }

    #[test]
    fn ensure_calibration_corrupt_flash_reruns_and_overwrites() {
        let mut memory_cyd = MemoryCyd::classic();
        let mut memory_flash_block = MemoryFlashBlock::with_raw_bytes(&[1, 2, 3, 4]);
        let mut memory_button = memory_cyd.memory_button();
        script_happy_path(&mut memory_cyd);

        let outcome = block_on(ensure_calibration(
            &mut memory_cyd,
            &mut memory_flash_block,
            &mut memory_button,
            None,
        ))
        .expect("corrupt flash should fall back to calibration");

        assert!(matches!(outcome, EnsureCalibrationOutcome::Saved(_)));
        assert_eq!(memory_flash_block.save_count(), 1);
        assert!(
            memory_flash_block
                .load::<CalibrationConfig>()
                .expect("load should succeed")
                .is_some()
        );
    }

    #[test]
    fn ensure_calibration_paces_with_one_flush_per_iteration() {
        let mut memory_cyd = MemoryCyd::classic();
        memory_cyd.set_frame_budget(3);
        let mut memory_flash_block = MemoryFlashBlock::new();
        let mut memory_button = memory_cyd.memory_button();

        let error = block_on(ensure_calibration(
            &mut memory_cyd,
            &mut memory_flash_block,
            &mut memory_button,
            None,
        ))
        .expect_err("empty input should stop at the frame budget");

        assert!(matches!(
            error,
            EnsureCalibrationError::Device(MemoryCydError::OutOfFrames)
        ));
        assert_eq!(memory_cyd.flush_count(), 3);
    }

    #[test]
    fn ensure_calibration_drains_a_full_tap_in_one_frame() {
        let mut memory_cyd = MemoryCyd::classic();
        memory_cyd.set_frame_budget(1);
        let upper_left_raw_point = raw_point_for_corner(CalibrationCorner::UpperLeft);
        memory_cyd.script_raw_frames_owned(vec![tap_frame(upper_left_raw_point)]);
        let mut memory_flash_block = MemoryFlashBlock::new();
        let mut memory_button = memory_cyd.memory_button();

        let error = block_on(ensure_calibration(
            &mut memory_cyd,
            &mut memory_flash_block,
            &mut memory_button,
            None,
        ))
        .expect_err("single-frame budget should stop after the first drawn frame");

        assert!(matches!(
            error,
            EnsureCalibrationError::Device(MemoryCydError::OutOfFrames)
        ));
        let upper_left_center = calibration_corner_center(CalibrationCorner::UpperLeft);
        let upper_right_center = calibration_corner_center(CalibrationCorner::UpperRight);
        assert_eq!(
            memory_cyd.pixel(upper_left_center.x as usize, upper_left_center.y as usize),
            Rgb565::CSS_WHITE
        );
        assert_eq!(
            memory_cyd.pixel(upper_right_center.x as usize, upper_right_center.y as usize),
            Rgb565::CSS_WHITE
        );
        assert_eq!(memory_cyd.pixel(160, 120), Rgb565::CSS_BLACK);
    }

    #[test]
    fn ensure_calibration_verify_timeout_restarts_and_then_succeeds() {
        let mut memory_cyd = MemoryCyd::classic();
        let mut frames = happy_path_frames();
        frames.truncate(frames.len() - 1);
        frames.extend((0..VERIFY_TIMEOUT_EXTRA_IDLE_FRAMES).map(|_| Vec::new()));
        frames.extend((0..REJECTED_RESTART_IDLE_FRAMES).map(|_| Vec::new()));
        frames.extend(happy_path_frames());
        memory_cyd.script_raw_frames_owned(frames);

        let mut memory_flash_block = MemoryFlashBlock::new();
        let mut memory_button = memory_cyd.memory_button();

        let outcome = block_on(ensure_calibration(
            &mut memory_cyd,
            &mut memory_flash_block,
            &mut memory_button,
            None,
        ))
        .expect("flow should restart after verify timeout and then save");

        assert!(matches!(outcome, EnsureCalibrationOutcome::Saved(_)));
        assert_eq!(memory_flash_block.save_count(), 1);
    }

    fn script_happy_path(memory_cyd: &mut MemoryCyd) -> [RawPoint; 4] {
        memory_cyd.script_raw_frames_owned(happy_path_frames());
        [
            raw_point_for_corner(CalibrationCorner::UpperLeft),
            raw_point_for_corner(CalibrationCorner::UpperRight),
            raw_point_for_corner(CalibrationCorner::LowerRight),
            raw_point_for_corner(CalibrationCorner::LowerLeft),
        ]
    }

    fn happy_path_frames() -> Vec<Vec<RawTouchEvent>> {
        let mut frames = Vec::new();
        let calibration_corners = [
            CalibrationCorner::UpperLeft,
            CalibrationCorner::UpperRight,
            CalibrationCorner::LowerRight,
            CalibrationCorner::LowerLeft,
        ];
        for (corner_index, calibration_corner) in calibration_corners.into_iter().enumerate() {
            frames.push(tap_frame(raw_point_for_corner(calibration_corner)));
            if corner_index + 1 != calibration_corners.len() {
                frames.extend((0..CAPTURE_ACK_EXTRA_IDLE_FRAMES).map(|_| Vec::new()));
            }
        }
        let verify_center = calibration_verify_target_center();
        frames.push(tap_frame(distort_demo_screen_to_raw(
            verify_center.x as f32,
            verify_center.y as f32,
        )));
        frames
    }

    fn raw_point_for_corner(calibration_corner: CalibrationCorner) -> RawPoint {
        let screen_point = calibration_corner_center(calibration_corner);
        distort_demo_screen_to_raw(screen_point.x as f32, screen_point.y as f32)
    }

    fn tap_frame(raw_point: RawPoint) -> Vec<RawTouchEvent> {
        let mut raw_touch_events = Vec::new();
        raw_touch_events.push(RawTouchEvent::Down {
            raw_x: raw_point.x,
            raw_y: raw_point.y,
        });
        for _discarded_sample_index in 0..4 {
            raw_touch_events.push(RawTouchEvent::Move {
                raw_x: raw_point.x,
                raw_y: raw_point.y,
            });
        }
        for _usable_sample_index in 0..3 {
            raw_touch_events.push(RawTouchEvent::Move {
                raw_x: raw_point.x,
                raw_y: raw_point.y,
            });
        }
        raw_touch_events.push(RawTouchEvent::Up);
        raw_touch_events
    }
}
