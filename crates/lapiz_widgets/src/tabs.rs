use iced_core::{Element, Length, Theme};
use iced_wgpu::Renderer;

use crate::{button::Button, flex::Flex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    #[default]
    Line,
    Block,
}

struct Tab<'a, Message> {
    content: Element<'a, Message, Theme, Renderer>,
    selected: bool,
    message: Option<Message>,
}

pub struct Tabs<'a, Message> {
    tabs: Vec<Tab<'a, Message>>,
    variant: Variant,
    width: Length,
    height: Length,
}

impl<'a, Message> Tabs<'a, Message> {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            variant: Variant::Line,
            width: Length::Shrink,
            height: Length::Fixed(26.0),
        }
    }

    pub fn push(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        selected: bool,
        message: Message,
    ) -> Self {
        self.tabs.push(Tab {
            content: content.into(),
            selected,
            message: Some(message),
        });
        self
    }

    pub fn push_disabled(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        selected: bool,
    ) -> Self {
        self.tabs.push(Tab {
            content: content.into(),
            selected,
            message: None,
        });
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

    pub fn line(mut self) -> Self {
        self.variant = Variant::Line;
        self
    }

    pub fn block(mut self) -> Self {
        self.variant = Variant::Block;
        self
    }
}

impl<Message> Default for Tabs<'_, Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message: 'a> From<Tabs<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: Tabs<'a, Message>) -> Self {
        let variant = value.variant;
        let tabs = value.tabs.into_iter().map(move |tab| {
            let selected = tab.selected;
            Button::new(tab.content)
                .height(Length::Fill)
                .padding([0, 12])
                .style(move |theme, status| style(theme, status, variant, selected))
                .on_press_maybe(tab.message)
                .into()
        });
        Flex::row(tabs)
            .width(value.width)
            .height(value.height)
            .style(tab_bar)
            .into()
    }
}

fn style(
    theme: &Theme,
    status: crate::button::Status,
    variant: Variant,
    selected: bool,
) -> crate::button::Style {
    if variant == Variant::Block && selected {
        return crate::button::activated_style(theme, status);
    }
    let p = theme.extended_palette();
    let mut style = crate::button::transparent(theme, status);
    if selected {
        style.text_color = p.background.base.text;
        if variant == Variant::Line {
            style.border.width = 0.0;
            style.shadow = iced_core::Shadow {
                color: p.primary.base.color,
                offset: iced_core::Vector::new(0.0, 2.0),
                blur_radius: 0.0,
            };
        }
    }
    style
}

fn tab_bar(theme: &Theme, _status: crate::flex::Status) -> crate::flex::Style {
    let p = theme.extended_palette();
    crate::flex::Style::default().border(iced_core::Border {
        radius: 0.0.into(),
        width: 1.0,
        color: p.background.strong.color,
    })
}
