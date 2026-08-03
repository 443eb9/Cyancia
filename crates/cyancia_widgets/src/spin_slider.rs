use iced_core::{
    Background, Border, Clipboard, Color, Element, Event, Layout, Length, Pixels, Point, Rectangle,
    Shell, Size, Text, Widget,
    alignment::{Horizontal, Vertical},
    border::{self},
    keyboard::{self, key::Key},
    layout, mouse, renderer,
    text::{Alignment, LineHeight, Shaping, Wrapping},
    widget::tree::{self, Tree},
};
use iced_widget::{TextInput, text_input};
use std::ops::RangeInclusive;

pub struct SpinSlider<'a, Message, Theme = iced_core::Theme>
where
    Theme: Catalog,
{
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
    precision: usize,
    scale: SliderScale,
    default: Option<f32>,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
    on_release: Option<Box<dyn Fn(f32) -> Message + 'a>>,
    width: Length,
    height: Length,
    size: f32,
    rounded: f32,
    prefix: String,
    suffix: String,
    disabled: bool,
    class: <Theme as Catalog>::Class<'a>,
}

impl<'a, Message, Theme> SpinSlider<'a, Message, Theme>
where
    Theme: Catalog,
{
    pub const DEFAULT_HEIGHT: f32 = 24.0;

    pub fn new(
        range: RangeInclusive<f32>,
        value: f32,
        on_change: impl Fn(f32) -> Message + 'a,
    ) -> Self {
        Self {
            value: value.clamp(*range.start(), *range.end()),
            range,
            step: 0.1,
            precision: 2,
            scale: SliderScale::Linear,
            default: None,
            on_change: Box::new(on_change),
            on_release: None,
            width: Length::Fill,
            height: Length::Fixed(Self::DEFAULT_HEIGHT),
            size: Self::DEFAULT_HEIGHT,
            rounded: 4.0,
            prefix: String::new(),
            suffix: String::new(),
            disabled: false,
            class: <Theme as Catalog>::default(),
        }
    }

    pub fn new_01(value: f32, on_change: impl Fn(f32) -> Message + 'a) -> Self {
        Self::new(0.0..=1.0, value, on_change)
    }

    pub fn new_percent(value: f32, on_change: impl Fn(f32) -> Message + 'a) -> Self {
        Self::new(0.0..=100.0, value, on_change).precision(0)
    }

    pub fn default(mut self, value: f32) -> Self {
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

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn rounded(mut self, rounded: f32) -> Self {
        self.rounded = rounded;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    pub fn precision(mut self, precision: usize) -> Self {
        self.precision = precision;
        self.step = 10.0_f32.powi(-(precision as i32));
        self
    }

    pub fn scale(mut self, scale: SliderScale) -> Self {
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

    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    #[must_use]
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn text_input<'b, Renderer>(
        &'b self,
        value: &'b str,
    ) -> Element<'b, SpinSliderInputMessage, Theme, Renderer>
    where
        Renderer: iced_core::text::Renderer + 'b,
        for<'c> <Theme as text_input::Catalog>::Class<'c>: From<text_input::StyleFn<'c, Theme>>,
    {
        TextInput::new("", value)
            .on_input(SpinSliderInputMessage::Changed)
            .on_submit(SpinSliderInputMessage::Submitted)
            .width(Length::Fill)
            .padding([0, 4])
            .size(self.size * 0.62)
            .line_height(LineHeight::Relative(1.0))
            .align_x(Horizontal::Center)
            .style(|theme, status| {
                let spin_style = <Theme as Catalog>::style(theme, &self.class, Status::Editing);
                let input_class = <Theme as text_input::Catalog>::default();
                let input_style =
                    <Theme as text_input::Catalog>::style(theme, &input_class, status);
                let text_color = spin_style.text_color.unwrap_or_else(|| {
                    <Theme as text_input::Catalog>::style(
                        theme,
                        &input_class,
                        text_input::Status::Active,
                    )
                    .value
                });
                text_input::Style {
                    background: Color::TRANSPARENT.into(),
                    border: Border::default(),
                    placeholder: text_color,
                    value: text_color,
                    ..input_style
                }
            })
            .into()
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

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for SpinSlider<'_, Message, Theme>
where
    Theme: Catalog,
    Renderer: iced_core::Renderer + iced_core::text::Renderer,
    for<'a> <Theme as text_input::Catalog>::Class<'a>: From<text_input::StyleFn<'a, Theme>>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SpinSliderTreeState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SpinSliderTreeState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.text_input::<Renderer>(""))]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_ref::<SpinSliderTreeState>();
        let value = match &state.interaction {
            SpinSliderState::Editing { value } => value.clone(),
            _ => format!("{:.*}", self.precision, self.value),
        };
        tree.diff_children(&[&self.text_input::<Renderer>(&value)]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(self.width, self.height, Size::new(0.0, self.size));
        let button_width = size.height.min(size.width / 3.0);
        let input_size = Size::new((size.width - button_width * 2.0).max(0.0), size.height);
        let state = tree.state.downcast_ref::<SpinSliderTreeState>();
        let value = match &state.interaction {
            SpinSliderState::Editing { value } => value.clone(),
            _ => format!("{:.*}", self.precision, self.value),
        };
        let mut input = self.text_input::<Renderer>(&value);
        let input = input
            .as_widget_mut()
            .layout(
                &mut tree.children[0],
                renderer,
                &layout::Limits::new(input_size, input_size),
            )
            .move_to(Point::new(button_width, 0.0));

        layout::Node::with_children(size, vec![input])
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<SpinSliderTreeState>();
        let bounds = layout.bounds();
        let (minus, field, plus) = split_bounds(bounds);

        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = *modifiers;
        }

        if self.disabled {
            if matches!(state.interaction, SpinSliderState::Editing { .. }) {
                tree.children[0]
                    .state
                    .downcast_mut::<text_input::State<Renderer::Paragraph>>()
                    .unfocus();
            }
            state.interaction = SpinSliderState::Idle;
            return;
        }

        if let SpinSliderState::Editing { value } = &state.interaction {
            let input_value = value.clone();
            let mut input = self.text_input::<Renderer>(&input_value);
            let mut input_messages = Vec::new();
            let mut input_shell = Shell::new(&mut input_messages);
            input.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout.child(0),
                cursor,
                renderer,
                clipboard,
                &mut input_shell,
                viewport,
            );

            let event_captured = input_shell.is_event_captured();
            if input_shell.is_event_captured() {
                shell.capture_event();
            }
            shell.request_redraw_at(input_shell.redraw_request());
            if input_shell.is_layout_invalid() {
                shell.invalidate_layout();
            }
            if input_shell.are_widgets_invalid() {
                shell.invalidate_widgets();
            }
            shell.input_method_mut().merge(input_shell.input_method());
            drop(input_shell);

            for message in input_messages {
                match message {
                    SpinSliderInputMessage::Changed(value) => {
                        state.interaction = SpinSliderState::Editing { value };
                    }
                    SpinSliderInputMessage::Submitted => {
                        let SpinSliderState::Editing { value } = &state.interaction else {
                            continue;
                        };
                        if self.commit_input(value, shell) {
                            state.interaction = SpinSliderState::Idle;
                        }
                    }
                }
            }

            if matches!(
                event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            ) {
                state.interaction = SpinSliderState::Idle;
            }

            if !matches!(state.interaction, SpinSliderState::Editing { .. }) {
                tree.children[0]
                    .state
                    .downcast_mut::<text_input::State<Renderer::Paragraph>>()
                    .unfocus();
            }

            shell.request_redraw();
            if event_captured {
                return;
            }
        }

        if let (
            SpinSliderState::Editing { value },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        ) = (&state.interaction, event)
        {
            let position = cursor.position();
            if position.is_some_and(|position| field.contains(position)) {
                shell.capture_event();
                return;
            }

            let edited_value = value.parse::<f32>().ok().map(|value| self.snap(value));
            state.interaction = SpinSliderState::Idle;
            tree.children[0]
                .state
                .downcast_mut::<text_input::State<Renderer::Paragraph>>()
                .unfocus();

            if position.is_some_and(|position| minus.contains(position)) {
                self.publish_change(
                    self.step_value(edited_value.unwrap_or(self.value), -1.0),
                    shell,
                );
                state.interaction = SpinSliderState::Pressing {
                    target: PressTarget::Minus,
                };
                shell.capture_event();
            } else if position.is_some_and(|position| plus.contains(position)) {
                self.publish_change(
                    self.step_value(edited_value.unwrap_or(self.value), 1.0),
                    shell,
                );
                state.interaction = SpinSliderState::Pressing {
                    target: PressTarget::Plus,
                };
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

                state.interaction = if minus.contains(position) {
                    self.publish_change(self.step_value(self.value, -1.0), shell);
                    SpinSliderState::Pressing {
                        target: PressTarget::Minus,
                    }
                } else if plus.contains(position) {
                    self.publish_change(self.step_value(self.value, 1.0), shell);
                    SpinSliderState::Pressing {
                        target: PressTarget::Plus,
                    }
                } else if field.contains(position) {
                    if state.modifiers.command() {
                        if let Some(default) = self.default {
                            self.publish_change(default, shell);
                        }
                        SpinSliderState::Idle
                    } else {
                        SpinSliderState::Pressing {
                            target: PressTarget::Field,
                        }
                    }
                } else {
                    return;
                };

                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
                if matches!(
                    state.interaction,
                    SpinSliderState::Pressing {
                        target: PressTarget::Field
                    } | SpinSliderState::Dragging { .. }
                ) =>
            {
                let Some(position) = cursor.position() else {
                    return;
                };
                let value = self.percentage_to_value((position.x - field.x) / field.width);
                state.interaction = SpinSliderState::Dragging { value };
                self.publish_change(value, shell);
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                match std::mem::take(&mut state.interaction) {
                    SpinSliderState::Dragging { value } => {
                        if let Some(on_release) = &self.on_release {
                            shell.publish(on_release(value));
                        }
                    }
                    SpinSliderState::Pressing {
                        target: PressTarget::Field,
                    } => {
                        state.interaction = SpinSliderState::Editing {
                            value: format!("{:.*}", self.precision, self.value),
                        };
                        let input_state = tree.children[0]
                            .state
                            .downcast_mut::<text_input::State<Renderer::Paragraph>>();
                        input_state.focus();
                        input_state.select_all();
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                    SpinSliderState::Pressing { .. } => {}
                    interaction => {
                        state.interaction = interaction;
                        return;
                    }
                }
                shell.request_redraw();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if matches!(state.interaction, SpinSliderState::Idle)
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
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if matches!(state.interaction, SpinSliderState::Idle) && cursor.is_over(bounds) =>
            {
                match key {
                    Key::Named(keyboard::key::Named::ArrowUp) => {
                        self.publish_change(self.step_value(self.value, 1.0), shell);
                        shell.capture_event();
                    }
                    Key::Named(keyboard::key::Named::ArrowDown) => {
                        self.publish_change(self.step_value(self.value, -1.0), shell);
                        shell.capture_event();
                    }
                    _ => {}
                }
            }
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
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<SpinSliderTreeState>();
        let bounds = layout.bounds();
        let (minus, field, plus) = split_bounds(bounds);
        let status = if self.disabled {
            Status::Disabled
        } else if matches!(state.interaction, SpinSliderState::Editing { .. }) {
            Status::Editing
        } else if matches!(state.interaction, SpinSliderState::Dragging { .. }) {
            Status::Dragged
        } else if cursor.is_over(bounds) {
            Status::Hovered
        } else {
            Status::Active
        };
        let style = <Theme as Catalog>::style(theme, &self.class, status);

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
        let value_bar = if matches!(state.interaction, SpinSliderState::Editing { .. }) {
            style.value_bar.scale_alpha(0.2)
        } else {
            style.value_bar
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    width: field.width * percentage,
                    ..field
                },
                ..Default::default()
            },
            value_bar,
        );

        let text_color = style.text_color.unwrap_or(renderer_style.text_color);
        fill_text(renderer, minus, "−", self.size, text_color);
        if let SpinSliderState::Editing { value } = &state.interaction {
            self.text_input::<Renderer>(value).as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                renderer_style,
                layout.child(0),
                cursor,
                viewport,
            );
        } else {
            fill_text(
                renderer,
                field,
                &format!(
                    "{}{:.*}{}",
                    self.prefix, self.precision, self.value, self.suffix
                ),
                self.size,
                text_color,
            );
        }
        fill_text(renderer, plus, "+", self.size, text_color);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.disabled || !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::default();
        }
        let state = tree.state.downcast_ref::<SpinSliderTreeState>();
        if let SpinSliderState::Editing { value } = &state.interaction
            && cursor.is_over(layout.child(0).bounds())
        {
            self.text_input::<Renderer>(value)
                .as_widget()
                .mouse_interaction(
                    &tree.children[0],
                    layout.child(0),
                    cursor,
                    viewport,
                    renderer,
                )
        } else if matches!(state.interaction, SpinSliderState::Dragging { .. }) {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Pointer
        }
    }
}

impl<Message, Theme> SpinSlider<'_, Message, Theme>
where
    Theme: Catalog,
{
    fn publish_change(&self, value: f32, shell: &mut Shell<'_, Message>) {
        if (value - self.value).abs() > f32::EPSILON {
            shell.publish((self.on_change)(value));
        }
    }

    fn commit_input(&self, input: &str, shell: &mut Shell<'_, Message>) -> bool {
        let Ok(value) = input.parse::<f32>() else {
            return false;
        };
        let value = self.snap(value);
        self.publish_change(value, shell);
        if let Some(on_release) = &self.on_release {
            shell.publish(on_release(value));
        }
        true
    }
}

impl<'a, Message, Theme, Renderer> From<SpinSlider<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: iced_core::Renderer + iced_core::text::Renderer + 'a,
    for<'b> <Theme as text_input::Catalog>::Class<'b>: From<text_input::StyleFn<'b, Theme>>,
{
    fn from(widget: SpinSlider<'a, Message, Theme>) -> Self {
        Element::new(widget)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderScale {
    Linear,
    Logarithmic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressTarget {
    Minus,
    Field,
    Plus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub background: Background,
    pub value_bar: Background,
    pub button_background: Background,
    pub border_color: Color,
    pub text_color: Option<Color>,
}

#[derive(Clone)]
enum SpinSliderInputMessage {
    Changed(String),
    Submitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Hovered,
    Dragged,
    Editing,
    Disabled,
}

#[derive(Debug, Default)]
struct SpinSliderTreeState {
    interaction: SpinSliderState,
    modifiers: keyboard::Modifiers,
}

#[derive(Debug, Default)]
enum SpinSliderState {
    #[default]
    Idle,
    Pressing {
        target: PressTarget,
    },
    Dragging {
        value: f32,
    },
    Editing {
        value: String,
    },
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
    size: f32,
    color: Color,
) where
    Renderer: iced_core::Renderer + iced_core::text::Renderer,
{
    renderer.fill_text(
        Text {
            content: content.to_owned(),
            bounds: bounds.size(),
            size: Pixels(size * 0.62),
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

pub trait Catalog: text_input::Catalog + Sized {
    type Class<'a>;

    fn default<'a>() -> <Self as Catalog>::Class<'a>;

    fn style(&self, class: &<Self as Catalog>::Class<'_>, status: Status) -> Style;
}

pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for iced_core::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> <Self as Catalog>::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &<Self as Catalog>::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

pub fn default(theme: &iced_core::Theme, status: Status) -> Style {
    let palette = theme.extended_palette();
    let value_bar = match status {
        Status::Hovered | Status::Editing => palette.primary.strong.color,
        Status::Dragged => palette.primary.weak.color,
        Status::Disabled | Status::Active => palette.primary.base.color,
    };
    let background = match status {
        Status::Editing => palette.background.weak.color,
        _ => palette.background.base.color,
    };
    Style {
        background: background.into(),
        value_bar: value_bar.into(),
        button_background: palette.background.weak.color.into(),
        border_color: palette.background.strong.color,
        text_color: None,
    }
}
