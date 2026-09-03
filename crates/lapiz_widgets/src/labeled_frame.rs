use iced_core::{Background, Element, Length, Pixels, Theme};
use iced_wgpu::Renderer;

pub use iced_aw::widget::labeled_frame::{Catalog, Style, StyleFn};

pub struct LabeledFrame<'a, Message> {
    inner: iced_aw::widget::LabeledFrame<'a, Message, Theme, Renderer>,
}

impl<'a, Message> LabeledFrame<'a, Message> {
    pub fn new(
        title: impl Into<Element<'a, Message, Theme, Renderer>>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            inner: iced_aw::widget::LabeledFrame::new(title, content)
                .inset(10)
                .outset(0)
                .stroke_width(1)
                .horizontal_title_padding(6)
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

    pub fn inset(mut self, inset: impl Into<Pixels>) -> Self {
        self.inner = self.inner.inset(inset);
        self
    }

    pub fn outset(mut self, outset: impl Into<Pixels>) -> Self {
        self.inner = self.inner.outset(outset);
        self
    }

    pub fn stroke_width(mut self, width: impl Into<Pixels>) -> Self {
        self.inner = self.inner.stroke_width(width);
        self
    }

    pub fn title_padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.inner = self.inner.horizontal_title_padding(padding);
        self
    }

    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self {
        self.inner = self.inner.style(style);
        self
    }
}

impl<'a, Message: 'a> From<LabeledFrame<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: LabeledFrame<'a, Message>) -> Self {
        value.inner.into()
    }
}

pub fn default(theme: &Theme) -> Style {
    Style {
        color: Background::Color(theme.extended_palette().background.strong.color),
        radius: 0.0.into(),
    }
}
