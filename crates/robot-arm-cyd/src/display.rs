#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayRect {
    pub left: u16,
    pub top: u16,
    pub width: u16,
    pub height: u16,
}

impl DisplayRect {
    #[must_use]
    pub const fn new(left: u16, top: u16, width: u16, height: u16) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

pub trait RectDisplay {
    fn write_rect_rgb565(&mut self, display_rect: DisplayRect, pixels: &[u16]);
}

pub struct NullDisplayRectWriter {
    width: u16,
    height: u16,
    last_write_pixels: usize,
}

impl NullDisplayRectWriter {
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            last_write_pixels: 0,
        }
    }

    #[must_use]
    pub const fn last_write_pixels(&self) -> usize {
        self.last_write_pixels
    }
}

impl RectDisplay for NullDisplayRectWriter {
    fn write_rect_rgb565(&mut self, display_rect: DisplayRect, pixels: &[u16]) {
        assert!(
            display_rect.left as u32 + display_rect.width as u32 <= self.width as u32,
            "display rect exceeds width"
        );
        assert!(
            display_rect.top as u32 + display_rect.height as u32 <= self.height as u32,
            "display rect exceeds height"
        );
        assert!(
            pixels.len() == display_rect.pixel_count(),
            "pixel slice length must match display rect area"
        );
        self.last_write_pixels = pixels.len();
    }
}
