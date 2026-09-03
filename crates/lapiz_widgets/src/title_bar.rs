use iced_core::{Element, Length, Theme};
use iced_wgpu::Renderer;
use iced_widget::container;

use crate::{
    button::Button,
    callback::Callback,
    flex::{Flex, Status},
    icon,
};

pub type Style = container::Style;

pub struct TitleBar<'a, Message> {
    content: Element<'a, Message, Theme, Renderer>,
    drag: Callback<'a, Message>,
    minimize: Callback<'a, Message>,
    maximize: Callback<'a, Message>,
    close: Callback<'a, Message>,
    class: <Theme as crate::flex::Catalog>::Class<'a>,
}

impl<'a, Message> TitleBar<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            drag: None,
            minimize: None,
            maximize: None,
            close: None,
            class: Box::new(default),
        }
    }

    crate::callback_methods!(drag);
    crate::callback_methods!(minimize);
    crate::callback_methods!(maximize);
    crate::callback_methods!(close);

    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self {
        self.class = Box::new(style);
        self
    }

    pub fn class(mut self, class: impl Into<<Theme as crate::flex::Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

impl<'a, Message: 'a> From<TitleBar<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: TitleBar<'a, Message>) -> Self {
        let mut controls = Flex::row(Vec::new()).height(Length::Fill);
        if value.minimize.is_some() {
            controls = controls.push(
                Button::new(icon::win_minimize().size(12))
                    .width(38)
                    .height(Length::Fill)
                    .padding([10, 13])
                    .transparent()
                    .on_press_with_maybe(value.minimize),
            );
        }
        if value.maximize.is_some() {
            controls = controls.push(
                Button::new(icon::win_maximize().size(12))
                    .width(38)
                    .height(Length::Fill)
                    .padding([10, 13])
                    .transparent()
                    .on_press_with_maybe(value.maximize),
            );
        }
        if value.close.is_some() {
            controls = controls.push(
                Button::new(icon::win_close().size(12))
                    .width(40)
                    .height(Length::Fill)
                    .padding([10, 14])
                    .style(close_button)
                    .on_press_with_maybe(value.close),
            );
        }
        Flex::row([value.content, controls.into()])
            .width(Length::Fill)
            .height(32)
            .class(value.class)
            .on_press_with_maybe(value.drag)
            .into()
    }
}

fn close_button(theme: &Theme, status: crate::button::Status) -> crate::button::Style {
    let p = theme.extended_palette();
    match status {
        crate::button::Status::Hovered | crate::button::Status::Pressed => crate::button::Style {
            background: Some(p.danger.base.color.into()),
            text_color: p.danger.base.text,
            ..Default::default()
        },
        crate::button::Status::Active | crate::button::Status::Disabled => {
            crate::button::transparent(theme, status)
        }
    }
}

pub fn default(theme: &Theme, _status: Status) -> Style {
    let p = theme.extended_palette();
    Style::default()
        .background(p.background.base.color)
        .color(p.background.base.text)
        .border(iced_core::Border {
            radius: 0.0.into(),
            width: 1.0,
            color: p.background.strong.color,
        })
}
