use iced_core::{
    Background, Border, Clipboard, Color, Element, Event, Layout, Length, Pixels, Point, Rectangle,
    Shell, Size, Text, Theme, Widget,
    alignment::Vertical,
    border::{self},
    keyboard::{self, key::Key},
    layout, mouse, renderer,
    text::{Alignment, LineHeight, Shaping, Wrapping},
    widget::tree::{self, Tree},
};
use std::ops::RangeInclusive;

pub struct SpinSlider<'a, Message> {
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
    precision: usize,
    scale: SliderScale,
    default: Option<f32>,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
    on_release: Option<Box<dyn Fn(f32) -> Message + 'a>>,
    width: Length,
    height: f32,
    rounded: f32,
    prefix: String,
    suffix: String,
    disabled: bool,
    style: Option<Box<dyn Fn(&Theme, Status) -> Style + 'a>>,
}

impl<'a, Message> SpinSlider<'a, Message> {
    pub const DEFAULT_HEIGHT: f32 = 24.0;

    pub fn new(
        range: RangeInclusive<f32>,
        value: f32,
        on_change: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        assert!(range.start().is_finite());
        assert!(range.end().is_finite());
        assert!(range.start() < range.end());
        assert!(value.is_finite());

        Self {
            value: value.clamp(*range.start(), *range.end()),
            range,
            step: 0.01,
            precision: 2,
            scale: SliderScale::Linear,
            default: None,
            on_change: Box::new(on_change),
            on_release: None,
            width: Length::Fill,
            height: Self::DEFAULT_HEIGHT,
            rounded: 4.0,
            prefix: String::new(),
            suffix: String::new(),
            disabled: false,
            style: None,
        }
    }

    pub fn new_01(value: f32, on_change: impl Fn(f32) -> Message + 'a) -> Self {
        Self::new(0.0..=1.0, value, on_change)
    }

    pub fn new_percent(value: f32, on_change: impl Fn(f32) -> Message + 'a) -> Self {
        Self::new(0.0..=100.0, value, on_change).precision(0)
    }

    pub fn default(mut self, value: f32) -> Self {
        assert!(value.is_finite());
        self.default = Some(value.clamp(*self.range.start(), *self.range.end()));
        self
    }

    pub fn on_release(mut self, callback: impl Fn(f32) -> Message + 'a) -> Self {
        self.on_release = Some(Box::new(callback));
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Pixels>) -> Self {
        self.height = height.into().0;
        assert!(self.height > 0.0);
        self
    }

    pub fn rounded(mut self, rounded: f32) -> Self {
        assert!(rounded >= 0.0);
        self.rounded = rounded;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        assert!(step.is_finite() && step > 0.0);
        self.step = step;
        self
    }

    pub fn precision(mut self, precision: usize) -> Self {
        assert!(precision <= 38);
        self.precision = precision;
        self.step = 10.0_f32.powi(-(precision as i32));
        assert!(self.step > 0.0);
        self
    }

    pub fn scale(mut self, scale: SliderScale) -> Self {
        if scale == SliderScale::Logarithmic {
            assert!(*self.range.start() > 0.0);
        }
        self.scale = scale;
        self
    }

    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self {
        self.style = Some(Box::new(style));
        self
    }

    fn step_value(&self, value: f32, direction: f32) -> f32 {
        self.snap(value + self.step * direction)
    }

    fn snap(&self, value: f32) -> f32 {
        let start = *self.range.start();
        let steps = ((value - start) / self.step).round();
        (start + steps * self.step).clamp(start, *self.range.end())
    }

    fn value_to_percentage(&self, value: f32) -> f32 {
        let start = *self.range.start();
        let end = *self.range.end();
        match self.scale {
            SliderScale::Linear => (value - start) / (end - start),
            SliderScale::Logarithmic => (value / start).ln() / (end / start).ln(),
        }
        .clamp(0.0, 1.0)
    }

    fn percentage_to_value(&self, percentage: f32) -> f32 {
        let percentage = percentage.clamp(0.0, 1.0);
        let start = *self.range.start();
        let end = *self.range.end();
        let value = match self.scale {
            SliderScale::Linear => start + (end - start) * percentage,
            SliderScale::Logarithmic => (end / start).powf(percentage) * start,
        };
        self.snap(value)
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for SpinSlider<'_, Message>
where
    Renderer: iced_core::Renderer + iced_core::text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SpinSliderState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SpinSliderState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Shrink)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<SpinSliderState>();
        let bounds = layout.bounds();
        let (minus, field, plus) = split_bounds(bounds);

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = *modifiers;
        }

        if self.disabled {
            return;
        }

        if state.mode == Mode::Type
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            )
        {
            let position = cursor.position();
            if position.is_some_and(|position| field.contains(position)) {
                shell.capture_event();
                return;
            }

            let edited_value = state
                .input_buffer
                .parse::<f32>()
                .ok()
                .map(|value| self.snap(value));
            state.input_buffer.clear();
            state.mode = Mode::Drag;

            if position.is_some_and(|position| minus.contains(position)) {
                self.publish_change(
                    self.step_value(edited_value.unwrap_or(self.value), -1.0),
                    shell,
                );
                state.press_target = Some(PressTarget::Minus);
                shell.capture_event();
            } else if position.is_some_and(|position| plus.contains(position)) {
                self.publish_change(
                    self.step_value(edited_value.unwrap_or(self.value), 1.0),
                    shell,
                );
                state.press_target = Some(PressTarget::Plus);
                shell.capture_event();
            } else if let Some(value) = edited_value {
                self.publish_change(value, shell);
                if let Some(on_release) = &self.on_release {
                    shell.publish(on_release(value));
                }
            }
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position() else {
                    return;
                };

                if minus.contains(position) {
                    self.publish_change(self.step_value(self.value, -1.0), shell);
                    state.press_target = Some(PressTarget::Minus);
                } else if plus.contains(position) {
                    self.publish_change(self.step_value(self.value, 1.0), shell);
                    state.press_target = Some(PressTarget::Plus);
                } else if field.contains(position) {
                    if state.modifiers.command() {
                        if let Some(default) = self.default {
                            self.publish_change(default, shell);
                        }
                    } else {
                        state.press_target = Some(PressTarget::Field);
                        state.clicked_but_not_moved = true;
                        state.drag_value = None;
                    }
                } else {
                    return;
                }

                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
                if state.press_target == Some(PressTarget::Field) =>
            {
                let Some(position) = cursor.position() else {
                    return;
                };
                let percentage = (position.x - field.x) / field.width;
                let value = self.percentage_to_value(percentage);
                state.drag_value = Some(value);
                self.publish_change(value, shell);
                state.dragging = true;
                state.clicked_but_not_moved = false;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                match state.press_target.take() {
                    Some(PressTarget::Field) if state.dragging => {
                        state.dragging = false;
                        let value = state.drag_value.take().unwrap_or(self.value);
                        if let Some(on_release) = &self.on_release {
                            shell.publish(on_release(value));
                        }
                    }
                    Some(PressTarget::Field) if state.clicked_but_not_moved => {
                        state.clicked_but_not_moved = false;
                        state.mode = Mode::Type;
                        state.input_buffer = format!("{:.*}", self.precision, self.value);
                    }
                    Some(PressTarget::Field | PressTarget::Minus | PressTarget::Plus) => {}
                    None => return,
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if state.mode == Mode::Drag
                    && state.modifiers.control()
                    && cursor.is_over(bounds) =>
            {
                let delta = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                self.publish_change(
                    self.step_value(self.value, if delta < 0.0 { -1.0 } else { 1.0 }),
                    shell,
                );
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key, physical_key, ..
            }) => match state.mode {
                Mode::Drag if cursor.is_over(bounds) => match key {
                    Key::Named(keyboard::key::Named::ArrowUp) => {
                        self.publish_change(self.step_value(self.value, 1.0), shell);
                        shell.capture_event();
                    }
                    Key::Named(keyboard::key::Named::ArrowDown) => {
                        self.publish_change(self.step_value(self.value, -1.0), shell);
                        shell.capture_event();
                    }
                    _ => {}
                },
                Mode::Type => {
                    if key == &Key::Named(keyboard::key::Named::Escape) {
                        state.input_buffer.clear();
                        state.mode = Mode::Drag;
                        shell.capture_event();
                    } else if key == &Key::Named(keyboard::key::Named::Backspace) {
                        state.input_buffer.pop();
                        shell.capture_event();
                    } else if key == &Key::Named(keyboard::key::Named::Enter) {
                        self.commit_input(state, shell);
                        shell.capture_event();
                    } else if let Some(character) = key.to_latin(*physical_key)
                        && (character.is_ascii_digit()
                            || matches!(character, '.' | '-' | '+' | 'e' | 'E'))
                    {
                        state.input_buffer.push(character);
                        shell.capture_event();
                    }
                }
                Mode::Drag => {}
            },
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        renderer_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<SpinSliderState>();
        let bounds = layout.bounds();
        let (minus, field, plus) = split_bounds(bounds);
        let status = if self.disabled {
            Status::Disabled
        } else if state.mode == Mode::Type {
            Status::Editing
        } else if state.dragging {
            Status::Dragged
        } else if cursor.is_over(bounds) {
            Status::Hovered
        } else {
            Status::Active
        };
        let style = self.style.as_ref().map_or_else(
            || default_style(theme, status),
            |style| style(theme, status),
        );

        renderer.fill_quad(
            renderer::Quad {
                bounds: minus,
                border: Border {
                    color: style.border_color,
                    width: 1.0,
                    radius: border::Radius {
                        top_left: self.rounded,
                        bottom_left: self.rounded,
                        ..Default::default()
                    },
                },
                ..Default::default()
            },
            style.button_background,
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: field,
                border: Border {
                    color: style.border_color,
                    width: 1.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            style.background,
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: plus,
                border: Border {
                    color: style.border_color,
                    width: 1.0,
                    radius: border::Radius {
                        top_right: self.rounded,
                        bottom_right: self.rounded,
                        ..Default::default()
                    },
                },
                ..Default::default()
            },
            style.button_background,
        );

        let percentage = self.value_to_percentage(self.value);
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    width: field.width * percentage,
                    ..field
                },
                ..Default::default()
            },
            style.value_bar,
        );

        let value_text = if state.mode == Mode::Type {
            state.input_buffer.clone()
        } else {
            format!(
                "{}{:.*}{}",
                self.prefix, self.precision, self.value, self.suffix
            )
        };
        let text_color = style.text_color.unwrap_or(renderer_style.text_color);
        fill_text(renderer, minus, "−", self.height, text_color);
        fill_text(renderer, field, &value_text, self.height, text_color);
        fill_text(renderer, plus, "+", self.height, text_color);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.disabled || !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::default();
        }
        if tree.state.downcast_ref::<SpinSliderState>().dragging {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Pointer
        }
    }
}

impl<Message> SpinSlider<'_, Message> {
    fn publish_change(&self, value: f32, shell: &mut Shell<'_, Message>) {
        if (value - self.value).abs() > f32::EPSILON {
            shell.publish((self.on_change)(value));
        }
    }

    fn commit_input(&self, state: &mut SpinSliderState, shell: &mut Shell<'_, Message>) {
        let Ok(value) = state.input_buffer.parse::<f32>() else {
            return;
        };
        let value = self.snap(value);
        self.publish_change(value, shell);
        if let Some(on_release) = &self.on_release {
            shell.publish(on_release(value));
        }
        state.input_buffer.clear();
        state.mode = Mode::Drag;
    }
}

impl<'a, Message, Renderer> From<SpinSlider<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: iced_core::Renderer + iced_core::text::Renderer + 'a,
{
    fn from(widget: SpinSlider<'a, Message>) -> Self {
        Element::new(widget)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderScale {
    Linear,
    Logarithmic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Hovered,
    Dragged,
    Editing,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub background: Background,
    pub value_bar: Background,
    pub button_background: Background,
    pub border_color: Color,
    pub text_color: Option<Color>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Drag,
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressTarget {
    Minus,
    Field,
    Plus,
}

#[derive(Debug, Default)]
struct SpinSliderState {
    dragging: bool,
    clicked_but_not_moved: bool,
    mode: Mode,
    modifiers: keyboard::Modifiers,
    input_buffer: String,
    press_target: Option<PressTarget>,
    drag_value: Option<f32>,
}

fn split_bounds(bounds: Rectangle) -> (Rectangle, Rectangle, Rectangle) {
    let button_width = bounds.height.min(bounds.width / 3.0);
    let minus = Rectangle {
        width: button_width,
        ..bounds
    };
    let field = Rectangle {
        x: bounds.x + button_width,
        width: bounds.width - button_width * 2.0,
        ..bounds
    };
    let plus = Rectangle {
        x: bounds.x + bounds.width - button_width,
        width: button_width,
        ..bounds
    };
    (minus, field, plus)
}

fn fill_text<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    content: &str,
    height: f32,
    color: Color,
) where
    Renderer: iced_core::Renderer + iced_core::text::Renderer,
{
    renderer.fill_text(
        Text {
            content: content.to_owned(),
            bounds: bounds.size(),
            size: Pixels(height * 0.62),
            line_height: LineHeight::Relative(1.0),
            font: renderer.default_font(),
            align_x: Alignment::Center,
            align_y: Vertical::Center,
            shaping: Shaping::Auto,
            wrapping: Wrapping::None,
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        color,
        bounds,
    );
}

fn default_style(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();
    let value_bar = match status {
        Status::Hovered | Status::Editing => palette.primary.strong.color,
        Status::Dragged => palette.primary.weak.color,
        Status::Disabled | Status::Active => palette.primary.base.color,
    };
    Style {
        background: palette.background.base.color.into(),
        value_bar: value_bar.into(),
        button_background: palette.background.weak.color.into(),
        border_color: palette.background.strong.color,
        text_color: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_linear_values_to_step() {
        let slider = SpinSlider::<()>::new(-1.0..=1.0, 0.0, |_| ()).step(0.25);
        assert_eq!(slider.percentage_to_value(0.0), -1.0);
        assert_eq!(slider.percentage_to_value(0.625), 0.25);
        assert_eq!(slider.percentage_to_value(1.0), 1.0);
    }

    #[test]
    fn maps_logarithmic_values() {
        let slider = SpinSlider::<()>::new(1.0..=100.0, 10.0, |_| ())
            .step(0.01)
            .scale(SliderScale::Logarithmic);
        assert!((slider.value_to_percentage(10.0) - 0.5).abs() < f32::EPSILON * 4.0);
        assert!((slider.percentage_to_value(0.5) - 10.0).abs() < 0.01);
    }

    #[test]
    fn splits_button_and_value_regions() {
        let bounds = Rectangle::new(Point::new(10.0, 20.0), Size::new(120.0, 24.0));
        let (minus, field, plus) = split_bounds(bounds);
        assert_eq!(minus.width, 24.0);
        assert_eq!(
            field,
            Rectangle::new(Point::new(34.0, 20.0), Size::new(72.0, 24.0))
        );
        assert_eq!(plus.x, 106.0);
    }
}
