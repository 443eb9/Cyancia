use iced_core::{Element, Length, Theme};
use iced_wgpu::Renderer;
use iced_widget::scrollable;

use crate::{button::Button, callback::Callback, flex::Flex, icon, label::Label, panel::Panel};

struct Item<'a, Message> {
    content: Element<'a, Message, Theme, Renderer>,
    selected: bool,
    message: Option<Message>,
}

pub struct PickList<'a, Message> {
    available: Vec<Item<'a, Message>>,
    selected: Vec<Item<'a, Message>>,
    available_label: String,
    selected_label: String,
    move_to_selected: Callback<'a, Message>,
    move_to_available: Callback<'a, Message>,
    width: Length,
    height: Length,
}

impl<'a, Message> PickList<'a, Message> {
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
            selected: Vec::new(),
            available_label: String::from("Available"),
            selected_label: String::from("Active"),
            move_to_selected: None,
            move_to_available: None,
            width: Length::Fill,
            height: Length::Fixed(150.0),
        }
    }

    pub fn labels(mut self, available: impl Into<String>, selected: impl Into<String>) -> Self {
        self.available_label = available.into();
        self.selected_label = selected.into();
        self
    }

    pub fn available_item(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        selected: bool,
        message: Message,
    ) -> Self {
        self.available.push(Item {
            content: content.into(),
            selected,
            message: Some(message),
        });
        self
    }

    pub fn selected_item(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        selected: bool,
        message: Message,
    ) -> Self {
        self.selected.push(Item {
            content: content.into(),
            selected,
            message: Some(message),
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

    crate::callback_methods!(move_to_selected);
    crate::callback_methods!(move_to_available);
}

impl<Message> Default for PickList<'_, Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message: 'a> From<PickList<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: PickList<'a, Message>) -> Self {
        let left_count = value.available.len();
        let right_count = value.selected.len();
        let left_items = value.available.into_iter().map(item_button);
        let right_items = value.selected.into_iter().map(item_button);
        let left = Panel::new(
            Flex::column([
                Flex::row([
                    Label::new(value.available_label).muted().into(),
                    Label::new(left_count.to_string()).faint().into(),
                ])
                .width(Length::Fill)
                .justify_content(taffy::JustifyContent::SpaceBetween)
                .padding([4, 8])
                .into(),
                scrollable(Flex::column(left_items).width(Length::Fill))
                    .height(Length::Fill)
                    .into(),
            ])
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .inset()
        .width(Length::Fill)
        .height(Length::Fill);
        let right = Panel::new(
            Flex::column([
                Flex::row([
                    Label::new(value.selected_label).muted().into(),
                    Label::new(right_count.to_string()).faint().into(),
                ])
                .width(Length::Fill)
                .justify_content(taffy::JustifyContent::SpaceBetween)
                .padding([4, 8])
                .into(),
                scrollable(Flex::column(right_items).width(Length::Fill))
                    .height(Length::Fill)
                    .into(),
            ])
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .inset()
        .width(Length::Fill)
        .height(Length::Fill);
        let controls = Flex::column([
            Button::new(icon::chevron_right().size(13))
                .width(24)
                .height(24)
                .padding(0)
                .on_press_with_maybe(value.move_to_selected)
                .into(),
            Button::new(icon::chevron_left().size(13))
                .width(24)
                .height(24)
                .padding(0)
                .on_press_with_maybe(value.move_to_available)
                .into(),
        ])
        .gap(4);
        Flex::row([left.into(), controls.into(), right.into()])
            .width(value.width)
            .height(value.height)
            .gap(8)
            .into()
    }
}

fn item_button<'a, Message: 'a>(item: Item<'a, Message>) -> Element<'a, Message, Theme, Renderer> {
    Button::new(item.content)
        .width(Length::Fill)
        .height(24)
        .padding([0, 8])
        .transparent()
        .activated(item.selected)
        .on_press_maybe(item.message)
        .into()
}
