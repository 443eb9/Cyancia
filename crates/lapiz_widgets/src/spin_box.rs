use std::{fmt::Display, ops::RangeBounds, str::FromStr};

use iced_aw::{NumberInput, style};
use iced_core::{Background, Element, Length, Padding, Pixels, Theme};
use iced_wgpu::Renderer;
use num_traits::{Num, NumAssignOps, bounds::Bounded};

pub use iced_aw::style::number_input::{Catalog, ExtendedCatalog, Style};
pub use iced_aw::style::{Status, StyleFn};

pub struct SpinBox<'a, T, Message>
where
    T: Num + NumAssignOps + PartialOrd + Display + FromStr + Clone + Bounded,
    Message: Clone,
{
    inner: NumberInput<'a, T, Message, Theme, Renderer>,
}

impl<'a, T, Message> SpinBox<'a, T, Message>
where
    T: Num + NumAssignOps + PartialOrd + Display + FromStr + Clone + Bounded + 'a + 'static,
    Message: Clone + 'a,
{
    pub fn new(
        value: &T,
        bounds: impl RangeBounds<T>,
        on_change: impl Fn(T) -> Message + Clone + 'static,
    ) -> Self {
        Self {
            inner: NumberInput::new(value, bounds, on_change)
                .set_size(12)
                .style(default)
                .input_style(crate::text_input::default),
        }
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.inner = self.inner.set_size(size);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.inner = self.inner.padding(padding);
        self
    }

    pub fn step(mut self, step: T) -> Self {
        self.inner = self.inner.step(step);
        self
    }

    pub fn on_submit(mut self, message: Message) -> Self {
        self.inner = self.inner.on_submit(message);
        self
    }

    pub fn on_submit_maybe(mut self, message: Option<Message>) -> Self {
        self.inner = self.inner.on_submit_maybe(message);
        self
    }

    pub fn ignore_buttons(mut self, ignore: bool) -> Self {
        self.inner = self.inner.ignore_buttons(ignore);
        self
    }

    pub fn ignore_scroll(mut self, ignore: bool) -> Self {
        self.inner = self.inner.ignore_scroll(ignore);
        self
    }

    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self {
        self.inner = self.inner.style(style);
        self
    }

    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.inner = self.inner.class(class);
        self
    }

    pub fn compact(self) -> Self {
        self.padding([3, 6])
    }
}

impl<'a, T, Message> From<SpinBox<'a, T, Message>> for Element<'a, Message, Theme, Renderer>
where
    T: Num + NumAssignOps + PartialOrd + Display + FromStr + Clone + Bounded + 'a + 'static,
    Message: Clone + 'a,
{
    fn from(value: SpinBox<'a, T, Message>) -> Self {
        value.inner.into()
    }
}

pub fn default(theme: &Theme, status: Status) -> Style {
    let p = theme.extended_palette();
    let disabled = status == Status::Disabled;
    let mut icon = if matches!(status, Status::Hovered | Status::Focused) {
        p.primary.strong.color
    } else {
        p.background.weak.text
    };
    if disabled {
        icon.a *= 0.4;
    }
    style::number_input::Style {
        button_background: Some(Background::Color(
            if matches!(status, Status::Hovered | Status::Focused) {
                p.primary.weak.color
            } else {
                iced_core::Color::TRANSPARENT
            },
        )),
        icon_color: icon,
    }
}
