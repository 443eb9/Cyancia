use iced_core::{Element, Length, Padding, Pixels, Widget};
use iced_widget::{Column, column};

pub struct Form<'a, Message, Theme, Renderer> {
    items: Vec<(
        Element<'a, Message, Theme, Renderer>,
        Element<'a, Message, Theme, Renderer>,
    )>,
    padding: Padding,
    spacing: Pixels,
}

impl<'a, Message, Theme, Renderer> Form<'a, Message, Theme, Renderer> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            padding: Padding::default(),
            spacing: Pixels(2.0),
        }
    }

    pub fn push<L, V>(mut self, label: L, value: V) -> Self
    where
        L: Into<Element<'a, Message, Theme, Renderer>>,
        V: Into<Element<'a, Message, Theme, Renderer>>,
    {
        self.items.push((label.into(), value.into()));
        self
    }

    pub fn extend<I>(mut self, items: I) -> Self
    where
        I: IntoIterator<
            Item = (
                Element<'a, Message, Theme, Renderer>,
                Element<'a, Message, Theme, Renderer>,
            ),
        >,
    {
        self.items.extend(items.into_iter());
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }
}

impl<'a, Message, Theme, Renderer> Into<Element<'a, Message, Theme, Renderer>>
    for Form<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced_core::Renderer + 'a,
{
    fn into(self) -> Element<'a, Message, Theme, Renderer> {
        Column::new()
            .padding(self.padding)
            .spacing(self.spacing)
            .extend(
                self.items
                    .into_iter()
                    .map(|(label, value)| column![label, value].spacing(self.spacing * 0.5).into()),
            )
            .into()
    }
}
