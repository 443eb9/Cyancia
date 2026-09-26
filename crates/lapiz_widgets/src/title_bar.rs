use iced_core::{
    Border, Element, Length, Point, Rectangle, Size, Theme, Widget, layout,
    pointer::{self, mouse},
    renderer, widget,
};
use iced_widget::{button, container};
use lapiz_runtime::Renderer;

use crate::{
    button::{Button, transparent},
    callback::Callback,
    flex::{self, Flex},
    icon,
};

pub type Style = container::Style;

pub struct TitleBar<'a, Message> {
    content: Element<'a, Message, Theme, Renderer>,
    minimize: Callback<'a, Message>,
    maximize: Callback<'a, Message>,
    close: Callback<'a, Message>,
    class: <Theme as flex::Catalog>::Class<'a>,
}

impl<'a, Message> TitleBar<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            minimize: Callback::Empty,
            maximize: Callback::Empty,
            close: Callback::Empty,
            class: Box::new(default),
        }
    }

    crate::callback_methods!(minimize);
    crate::callback_methods!(maximize);
    crate::callback_methods!(close);

    pub fn style(mut self, style: impl Fn(&Theme, flex::Status) -> Style + 'a) -> Self {
        self.class = Box::new(style);
        self
    }

    pub fn class(mut self, class: impl Into<<Theme as flex::Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

impl<'a, Message: 'a> From<TitleBar<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: TitleBar<'a, Message>) -> Self {
        let mut controls = Flex::row(Vec::new()).height(Length::Fill);
        if value.minimize.is_set() {
            controls = controls.push(
                Button::new(icon::win_minimize().size(12))
                    .width(38)
                    .height(Length::Fill)
                    .padding([10, 13])
                    .transparent()
                    .on_press_with_callback(value.minimize),
            );
        }
        if value.maximize.is_set() {
            controls = controls.push(
                Button::new(icon::win_maximize().size(12))
                    .width(38)
                    .height(Length::Fill)
                    .padding([10, 13])
                    .transparent()
                    .on_press_with_callback(value.maximize),
            );
        }
        if value.close.is_set() {
            controls = controls.push(
                Button::new(icon::win_close().size(12))
                    .width(40)
                    .height(Length::Fill)
                    .padding([10, 14])
                    .style(close_button)
                    .on_press_with_callback(value.close),
            );
        }
        Flex::row([value.content, controls.into()])
            .width(Length::Fill)
            .height(32)
            .class(value.class)
            .into()
    }
}

fn close_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = theme.palette();
    match status {
        button::Status::Hovered | button::Status::Pressed => button::Style {
            background: Some(p.danger.base.color.into()),
            text_color: p.danger.base.text,
            ..Default::default()
        },
        button::Status::Active | button::Status::Disabled => transparent(theme, status),
    }
}

pub fn default(theme: &Theme, _status: flex::Status) -> Style {
    let p = theme.palette();
    Style::default()
        .background(p.background.base.color)
        .color(p.background.base.text)
        .border(Border {
            radius: 0.0.into(),
            width: 1.0,
            color: p.background.strong.color,
        })
}

pub struct WindowCaptionRegion<Message> {
    on_drag: Box<dyn Fn(Point) -> Message>,
}

impl<Message> WindowCaptionRegion<Message> {
    pub fn new(on_drag: impl Fn(Point) -> Message + 'static) -> Self {
        Self {
            on_drag: Box::new(on_drag),
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for WindowCaptionRegion<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn update(
        &mut self,
        _tree: &mut widget::Tree,
        event: &iced_core::Event,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut iced_core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        match event {
            iced_core::Event::Pointer(event @ pointer::Event::PointerPressed { position, .. })
                if event.is_primary_press() =>
            {
                if cursor.is_over(layout.bounds()) {
                    shell.publish((self.on_drag)(*position));
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: layout::Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

impl<'a, Message> From<WindowCaptionRegion<Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
{
    fn from(value: WindowCaptionRegion<Message>) -> Element<'a, Message, Theme, Renderer> {
        Element::new(value)
    }
}
