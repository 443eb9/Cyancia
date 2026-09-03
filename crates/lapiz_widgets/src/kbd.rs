use iced_core::{Element, Length, Padding, Pixels, Theme, alignment, text};
use iced_wgpu::Renderer;
use iced_widget::{Container, Text, container};

pub use iced_widget::container::{Catalog, Style, StyleFn};

pub struct Kbd<'a, Message> {
    inner: Container<'a, Message, Theme, Renderer>,
}

impl<'a, Message> Kbd<'a, Message> {
    pub fn new(content: impl text::IntoFragment<'a>) -> Self {
        Self {
            inner: Container::new(Text::new(content).size(10).wrapping(text::Wrapping::None))
                .width(Length::Shrink)
                .height(18)
                .padding(Padding::from([1, 4]))
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .style(default),
        }
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.inner = self.inner.height(height);
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.inner = self.inner.padding(padding);
        self
    }

    pub fn size(self, _size: impl Into<Pixels>) -> Self {
        self
    }

    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self {
        self.inner = self.inner.style(style);
        self
    }

    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.inner = self.inner.class(class);
        self
    }

    pub fn accent(self) -> Self {
        self.style(accent)
    }
}

impl<'a, Message: 'a> From<Kbd<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: Kbd<'a, Message>) -> Self {
        value.inner.into()
    }
}

pub fn default(theme: &Theme) -> Style {
    let p = theme.extended_palette();
    container::Style::default()
        .background(p.background.weakest.color)
        .color(p.background.weak.text)
        .border(iced_core::Border {
            radius: 0.0.into(),
            width: 1.0,
            color: p.background.strong.color,
        })
}

pub fn accent(theme: &Theme) -> Style {
    let p = theme.extended_palette();
    container::Style::default()
        .background(p.primary.weak.color)
        .color(p.primary.strong.color)
        .border(iced_core::Border {
            radius: 0.0.into(),
            width: 1.0,
            color: p.primary.base.color,
        })
}
