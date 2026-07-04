//! Immediate-mode widgets for simple touch UIs.
//!
//! Widgets in this module do not own application state. Callers keep the
//! authoritative values and pass them into [`Ui::slider`] or react to
//! [`Ui::button`] and [`Ui::icon_button`] return values each frame.
//!
//! Layout specs such as [`Slider`] and [`Button`] are plain data. A slider drag
//! is identified by pointer equality on the layout spec, so sliders used with
//! [`Ui::slider`] must be defined as `static` items instead of `const` items:
//! `static` values have a stable unique address, while `const` values may be
//! duplicated at each use site.
//!
//! Widgets update state and draw in the same call, so call order is draw order.
//! Apps typically render their main scene first, then call widgets on top;
//! scene changes caused by this frame's input become visible on the next frame.
//!
//! Buttons fire on touch-down inside their rectangle. This matches resistive
//! touchscreen interaction and intentionally does not implement a press-cancel
//! gesture. A touch-down is consumed by the first widget, in call order, whose
//! touch rectangle contains it.
//!
//! ```rust,no_run
//! # use embedded_graphics::{
//! #     mock_display::MockDisplay,
//! #     pixelcolor::Rgb565,
//! # };
//! # use linkage_blaze_cyd_core::TouchEvent;
//! # use linkage_blaze_example_core::ui::{Slider, Ui};
//! static TILT_SLIDER: Slider = Slider::vertical(
//!     "z",
//!     16,
//!     24,
//!     201,
//!     1.0,
//!     0.0,
//! );
//!
//! let mut display = MockDisplay::<Rgb565>::new();
//! let mut ui = Ui::new();
//! let mut tilt = 0.5;
//!
//! ui.begin(Some(TouchEvent::Down { x: 16.0, y: 124.0 }));
//! ui.slider(&mut display, &TILT_SLIDER, &mut tilt)?;
//! ui.end(&mut display)?;
//! # Ok::<(), linkage_blaze_example_core::ui::UiError<core::convert::Infallible>>(())
//! ```

use core::{fmt, fmt::Write};

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::{Rgb565, Rgb888, WebColors},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle},
    text::{Baseline, Text},
};
use heapless::String;
use linkage_blaze_cyd_core::TouchEvent;

const LABEL_CAPACITY: usize = 24;
const SLIDER_TOUCH_PAD: i32 = 14;

/// Immediate-mode UI state for one touch pointer.
#[derive(Default)]
pub struct Ui {
    touch_cursor: Option<(f32, f32)>,
    touch_event: Option<TouchEvent>,
    active_slider: Option<&'static Slider>,
    down_consumed: bool,
}

impl Ui {
    /// Creates an empty UI state with no captured slider.
    pub const fn new() -> Self {
        Self {
            touch_cursor: None,
            touch_event: None,
            active_slider: None,
            down_consumed: false,
        }
    }

    /// Starts a frame by storing the touch event and refreshing pointer state.
    ///
    /// A touch-up clears the active drag capture before any widgets run.
    pub fn begin(&mut self, touch_event: Option<TouchEvent>) {
        self.touch_event = touch_event;
        self.down_consumed = false;

        match touch_event {
            Some(TouchEvent::Down { x, y }) | Some(TouchEvent::Move { x, y }) => {
                self.touch_cursor = Some((x, y));
            }
            Some(TouchEvent::Up) => {
                self.touch_cursor = None;
                self.active_slider = None;
            }
            None => {}
        }
    }

    /// Updates `value` from any captured drag, then draws the slider.
    ///
    /// Slider identity uses pointer equality on `slider`, so callers must pass
    /// a `static` layout spec instead of a `const` one.
    pub fn slider<D>(
        &mut self,
        target: &mut D,
        slider: &'static Slider,
        value: &mut f32,
    ) -> Result<(), UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        if let Some(TouchEvent::Down { x, y }) = self.touch_event {
            let touch_point = Point::new(x as i32, y as i32);
            if !self.down_consumed && slider.touch_rectangle.contains(touch_point) {
                self.active_slider = Some(slider);
                self.down_consumed = true;
            }
        }

        if self
            .active_slider
            .is_some_and(|active_slider| core::ptr::eq(active_slider, slider))
        {
            if let Some((position_x, position_y)) = self.touch_cursor {
                *value = slider.value_from_touch(position_x, position_y);
            }
        }

        slider.draw(target, *value).map_err(UiError::Draw)
    }

    /// Draws the button and returns `true` only on the frame a touch-down lands
    /// inside its rectangle.
    pub fn button<D>(
        &mut self,
        target: &mut D,
        button: &'static Button,
    ) -> Result<bool, UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let was_clicked = self.consume_down(button.touch_rectangle);
        button.draw(target).map_err(UiError::Draw)?;
        Ok(was_clicked)
    }

    /// Like [`Ui::button`], but draws an icon instead of text.
    pub fn icon_button<D>(
        &mut self,
        target: &mut D,
        icon_button: &'static IconButton,
    ) -> Result<bool, UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let was_clicked = self.consume_down(icon_button.touch_rectangle);
        icon_button.draw(target).map_err(UiError::Draw)?;
        Ok(was_clicked)
    }

    /// Formats `args` into a stack buffer and draws the label text.
    pub fn label<D>(
        &self,
        target: &mut D,
        label: &'static Label,
        args: fmt::Arguments<'_>,
    ) -> Result<(), UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let mut text = String::<LABEL_CAPACITY>::new();
        text.write_fmt(args)?;
        label.draw(target, text.as_str()).map_err(UiError::Draw)
    }

    /// Draws the cyan touch cursor on top of everything when a touch is active.
    pub fn end<D>(&self, target: &mut D) -> Result<(), UiError<D::Error>>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let Some((position_x, position_y)) = self.touch_cursor else {
            return Ok(());
        };
        let center_x = position_x as i32;
        let center_y = position_y as i32;
        let radius = 5;
        Circle::new(
            Point::new(center_x - radius, center_y - radius),
            (radius * 2 + 1) as u32,
        )
        .into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_CYAN))
        .draw(target)
        .map_err(UiError::Draw)?;
        Ok(())
    }

    fn consume_down(&mut self, touch_rectangle: Rectangle) -> bool {
        let Some(TouchEvent::Down { x, y }) = self.touch_event else {
            return false;
        };
        if self.down_consumed {
            return false;
        }

        let touch_point = Point::new(x as i32, y as i32);
        if !touch_rectangle.contains(touch_point) {
            return false;
        }

        self.down_consumed = true;
        true
    }
}

/// Slider layout and value range.
#[derive(Clone, Copy)]
pub struct Slider {
    label: &'static str,
    label_position: Point,
    touch_rectangle: Rectangle,
    track_start: Point,
    track_end: Point,
    orientation: SliderOrientation,
    /// The value at `track_start`.
    start: f32,
    /// The value at `track_end`.
    last: f32,
}

impl Slider {
    /// Creates a horizontal slider from its track geometry.
    pub const fn horizontal(
        label: &'static str,
        track_x: i32,
        track_y: i32,
        track_length: u32,
        start: f32,
        last: f32,
    ) -> Self {
        let track_rectangle =
            Rectangle::new(Point::new(track_x, track_y), Size::new(track_length, 1));
        let track_start = track_rectangle.top_left;
        Self {
            label,
            label_position: Point::new(track_start.x, track_start.y - 15),
            touch_rectangle: horizontal_touch_rectangle(track_rectangle),
            track_start,
            track_end: Point::new(
                track_rectangle.top_left.x + track_rectangle.size.width as i32 - 1,
                track_rectangle.top_left.y,
            ),
            orientation: SliderOrientation::Horizontal,
            start,
            last,
        }
    }

    /// Creates a vertical slider from its track geometry.
    pub const fn vertical(
        label: &'static str,
        track_x: i32,
        track_y: i32,
        track_length: u32,
        start: f32,
        last: f32,
    ) -> Self {
        let track_rectangle =
            Rectangle::new(Point::new(track_x, track_y), Size::new(1, track_length));
        let track_start = track_rectangle.top_left;
        Self {
            label,
            label_position: Point::new(track_start.x - 5, 5),
            touch_rectangle: vertical_touch_rectangle(track_rectangle),
            track_start,
            track_end: Point::new(
                track_rectangle.top_left.x,
                track_rectangle.top_left.y + track_rectangle.size.height as i32 - 1,
            ),
            orientation: SliderOrientation::Vertical,
            start,
            last,
        }
    }

    /// Creates a column of `N` identical horizontal sliders, each ranged
    /// `0.0..=1.0`, with their tracks at `x`, the first track at `first_track_y`,
    /// and successive tracks spaced `step_y` apart. Touch rectangles follow the
    /// same symmetric-pad rule as any other horizontal slider.
    pub const fn column<const N: usize>(
        x: i32,
        first_track_y: i32,
        step_y: i32,
        track_length: u32,
        labels: [&'static str; N],
    ) -> [Self; N] {
        let mut slider_array = [Self::horizontal("", x, first_track_y, track_length, 0.0, 1.0); N];
        let mut slider_index = 0;
        while slider_index < N {
            let track_y = first_track_y + slider_index as i32 * step_y;
            slider_array[slider_index] =
                Self::horizontal(labels[slider_index], x, track_y, track_length, 0.0, 1.0);
            slider_index += 1;
        }
        slider_array
    }

    /// The label drawn beside the track.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.label
    }

    fn draw<D>(&self, target: &mut D, value: f32) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_WHITE);
        Text::with_baseline(self.label, self.label_position, text_style, Baseline::Top)
            .draw(target)?;
        Line::new(self.track_start, self.track_end)
            .into_styled(stroke_style(Rgb565::CSS_LIGHT_SLATE_GRAY, 2))
            .draw(target)?;
        Circle::with_center(self.knob_center(value), 9)
            .into_styled(fill_style(Rgb565::CSS_YELLOW))
            .draw(target)?;
        Ok(())
    }

    fn knob_center(&self, value: f32) -> Point {
        let display_value = (value - self.start) / (self.last - self.start);
        match self.orientation {
            SliderOrientation::Horizontal => Point::new(
                self.track_start.x
                    + round_to_i32((self.track_end.x - self.track_start.x) as f32 * display_value),
                self.track_start.y,
            ),
            SliderOrientation::Vertical => Point::new(
                self.track_start.x,
                self.track_start.y
                    + round_to_i32((self.track_end.y - self.track_start.y) as f32 * display_value),
            ),
        }
    }

    fn value_from_touch(&self, position_x: f32, position_y: f32) -> f32 {
        let slider_position = match self.orientation {
            SliderOrientation::Horizontal => {
                (position_x - self.track_start.x as f32)
                    / (self.track_end.x - self.track_start.x) as f32
            }
            SliderOrientation::Vertical => {
                (position_y - self.track_start.y as f32)
                    / (self.track_end.y - self.track_start.y) as f32
            }
        }
        .clamp(0.0, 1.0);
        self.start + (self.last - self.start) * slider_position
    }
}

#[derive(Clone, Copy)]
enum SliderOrientation {
    Horizontal,
    Vertical,
}

/// Text button layout spec.
#[derive(Clone, Copy)]
pub struct Button {
    touch_rectangle: Rectangle,
    label: &'static str,
}

impl Button {
    /// Creates a text button.
    pub const fn new(touch_rectangle: Rectangle, label: &'static str) -> Self {
        Self {
            touch_rectangle,
            label,
        }
    }

    fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_WHITE);
        self.touch_rectangle
            .into_styled(stroke_style(Rgb565::CSS_LIGHT_SLATE_GRAY, 1))
            .draw(target)?;
        Text::with_baseline(
            self.label,
            centered_text_position(self.touch_rectangle, self.label),
            text_style,
            Baseline::Top,
        )
        .draw(target)?;
        Ok(())
    }
}

/// Icon button layout spec.
#[derive(Clone, Copy)]
pub struct IconButton {
    touch_rectangle: Rectangle,
    icon: Icon,
}

impl IconButton {
    /// Creates an icon button.
    pub const fn new(touch_rectangle: Rectangle, icon: Icon) -> Self {
        Self {
            touch_rectangle,
            icon,
        }
    }

    fn draw<D>(&self, target: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        self.icon.draw(target, self.touch_rectangle)
    }
}

/// Icons supported by [`IconButton`].
#[derive(Clone, Copy)]
pub enum Icon {
    /// Filled green triangle.
    Play,
    /// Outlined box, green triangle, and white bar.
    StepForward,
}

impl Icon {
    fn draw<D>(&self, target: &mut D, touch_rectangle: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        match self {
            Self::Play => Self::draw_play(target, touch_rectangle),
            Self::StepForward => Self::draw_step_forward(target, touch_rectangle),
        }
    }

    fn draw_play<D>(target: &mut D, touch_rectangle: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        Triangle::new(
            touch_rectangle.top_left,
            Point::new(
                touch_rectangle.top_left.x,
                touch_rectangle.top_left.y + touch_rectangle.size.height as i32,
            ),
            Point::new(
                touch_rectangle.top_left.x + touch_rectangle.size.width as i32,
                touch_rectangle.top_left.y + touch_rectangle.size.height as i32 / 2,
            ),
        )
        .into_styled(fill_style(Rgb565::CSS_LIME))
        .draw(target)?;
        Ok(())
    }

    fn draw_step_forward<D>(target: &mut D, touch_rectangle: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let width = touch_rectangle.size.width as i32;
        let height = touch_rectangle.size.height as i32;
        touch_rectangle
            .into_styled(stroke_style(Rgb565::CSS_LIGHT_SLATE_GRAY, 1))
            .draw(target)?;
        Rectangle::new(
            Point::new(
                touch_rectangle.top_left.x + width - scale_offset(width, 5),
                touch_rectangle.top_left.y + height / 2 - scale_offset(height, 5),
            ),
            Size::new(scale_size(width, 2), scale_size(height, 10)),
        )
        .into_styled(fill_style(Rgb565::CSS_WHITE))
        .draw(target)?;
        Triangle::new(
            Point::new(
                touch_rectangle.top_left.x + scale_offset(width, 3),
                touch_rectangle.top_left.y + scale_offset(height, 4),
            ),
            Point::new(
                touch_rectangle.top_left.x + scale_offset(width, 3),
                touch_rectangle.top_left.y + height - scale_offset(height, 4),
            ),
            Point::new(
                touch_rectangle.top_left.x + width - scale_offset(width, 7),
                touch_rectangle.top_left.y + height / 2,
            ),
        )
        .into_styled(fill_style(Rgb565::CSS_LIME))
        .draw(target)?;
        Ok(())
    }
}

/// Text label position and color.
#[derive(Clone, Copy)]
pub struct Label {
    position: Point,
    color: Rgb888,
}

impl Label {
    /// Creates a text label.
    pub const fn new(position: Point, color: Rgb888) -> Self {
        Self { position, color }
    }

    fn draw<D>(&self, target: &mut D, text: &str) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::from(self.color));
        Text::with_baseline(text, self.position, text_style, Baseline::Top).draw(target)?;
        Ok(())
    }
}

/// Widget drawing or text-formatting failure.
///
/// Formatting has a concrete source type, so it gets a derived `From` and
/// propagates with `?`. The generic draw error remains an explicit
/// `UiError::Draw` conversion because a blanket `From<D>` would overlap with
/// those concrete conversions under coherence.
#[derive(Debug, derive_more::From)]
pub enum UiError<D> {
    /// Formatting label text failed, usually because the stack buffer overflowed.
    Text(fmt::Error),
    /// Drawing to the target failed.
    #[from(ignore)]
    Draw(D),
}

fn centered_text_position(touch_rectangle: Rectangle, label: &str) -> Point {
    let label_width = label.len() as i32 * FONT_6X10.character_size.width as i32;
    Point::new(
        touch_rectangle.top_left.x + (touch_rectangle.size.width as i32 - label_width) / 2,
        touch_rectangle.top_left.y + 2,
    )
}

const fn horizontal_touch_rectangle(track_rectangle: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(
            track_rectangle.top_left.x,
            track_rectangle.top_left.y - SLIDER_TOUCH_PAD,
        ),
        Size::new(
            track_rectangle.size.width,
            track_rectangle.size.height + (SLIDER_TOUCH_PAD as u32 * 2),
        ),
    )
}

const fn vertical_touch_rectangle(track_rectangle: Rectangle) -> Rectangle {
    Rectangle::new(
        Point::new(
            track_rectangle.top_left.x - SLIDER_TOUCH_PAD,
            track_rectangle.top_left.y,
        ),
        Size::new(
            track_rectangle.size.width + (SLIDER_TOUCH_PAD as u32 * 2),
            track_rectangle.size.height,
        ),
    )
}

fn fill_style(color: Rgb565) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_fill(color)
}

fn stroke_style(color: Rgb565, stroke_width: u32) -> PrimitiveStyle<Rgb565> {
    PrimitiveStyle::with_stroke(color, stroke_width)
}

fn scale_offset(extent: i32, baseline_offset: i32) -> i32 {
    round_to_i32(extent as f32 * baseline_offset as f32 / 18.0)
}

fn scale_size(extent: i32, baseline_size: i32) -> u32 {
    round_to_i32(extent as f32 * baseline_size as f32 / 18.0).max(1) as u32
}

fn round_to_i32(value: f32) -> i32 {
    libm::roundf(value) as i32
}
