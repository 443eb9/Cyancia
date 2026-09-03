use iced_aw::menu::{Item as AwItem, Menu as AwMenu, MenuBar as AwMenuBar};
use iced_core::{Element, Length, Pixels, Theme};
use iced_wgpu::Renderer;

use crate::{button::Button, divider::Divider, flex::Flex, icon};

pub use iced_aw::menu::{Catalog, DrawPath, ScrollSpeed, Style};

pub struct Menu<'a, Message> {
    items: Vec<AwItem<'a, Message, Theme, Renderer>>,
    width: f32,
}

impl<'a, Message: 'a> Menu<'a, Message> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            width: 180.0,
        }
    }

    pub fn item(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        message: Message,
    ) -> Self {
        self.items
            .push(AwItem::new(menu_button(content).on_press(message)));
        self
    }

    pub fn selected_item(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        message: Message,
    ) -> Self {
        self.items.push(AwItem::new(
            menu_button(content).activated(true).on_press(message),
        ));
        self
    }

    pub fn disabled_item(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.items.push(AwItem::new(
            menu_button(content).style(crate::button::transparent),
        ));
        self
    }

    pub fn submenu(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        submenu: Menu<'a, Message>,
    ) -> Self {
        let content = Flex::row([content.into(), icon::chevron_right().size(12).into()])
            .width(Length::Fill)
            .justify_content(taffy::JustifyContent::SpaceBetween);
        self.items.push(
            AwItem::with_menu(
                menu_button(content).style(menu_item_style).interactive(),
                submenu.into_inner(),
            )
            .close_on_click(false),
        );
        self
    }

    pub fn separator(mut self) -> Self {
        self.items.push(AwItem::new(
            Flex::row([Divider::horizontal(1).into()])
                .width(Length::Fill)
                .height(7)
                .padding([0, 8]),
        ));
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into().0;
        self
    }

    fn into_inner(self) -> AwMenu<'a, Message, Theme, Renderer> {
        AwMenu::new(self.items)
            .width(Length::Fixed(self.width))
            .max_width(self.width)
            .padding(3)
            .spacing(0)
            .close_on_item_click(true)
            .close_on_background_click(true)
    }
}

impl<'a, Message: 'a> Default for Menu<'a, Message> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MenuBar<'a, Message> {
    roots: Vec<AwItem<'a, Message, Theme, Renderer>>,
    width: Length,
    height: Length,
}

impl<'a, Message: 'a> MenuBar<'a, Message> {
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            width: Length::Shrink,
            height: Length::Fixed(28.0),
        }
    }

    pub fn menu(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        menu: Menu<'a, Message>,
    ) -> Self {
        self.roots.push(AwItem::with_menu(
            Button::new(content)
                .height(Length::Fill)
                .padding([0, 8])
                .style(menu_item_style)
                .interactive(),
            menu.into_inner(),
        ));
        self
    }

    pub fn action(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        message: Message,
    ) -> Self {
        self.roots.push(AwItem::new(
            Button::new(content)
                .height(Length::Fill)
                .padding([0, 8])
                .style(menu_item_style)
                .on_press(message),
        ));
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
}

impl<'a, Message: 'a> Default for MenuBar<'a, Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message: 'a> From<MenuBar<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: MenuBar<'a, Message>) -> Self {
        AwMenuBar::new(value.roots)
            .width(value.width)
            .height(value.height)
            .spacing(0)
            .padding(0)
            .close_on_item_click_global(true)
            .close_on_background_click_global(true)
            .style(menu_bar_style)
            .into()
    }
}

pub struct Dropdown<'a, Message> {
    trigger: Element<'a, Message, Theme, Renderer>,
    menu: Option<Menu<'a, Message>>,
}

impl<'a, Message> Dropdown<'a, Message> {
    pub fn new(
        trigger: impl Into<Element<'a, Message, Theme, Renderer>>,
        menu: Option<Menu<'a, Message>>,
    ) -> Self {
        Self {
            trigger: trigger.into(),
            menu,
        }
    }
}

impl<'a, Message: 'a> From<Dropdown<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: Dropdown<'a, Message>) -> Self {
        let root = match value.menu {
            Some(menu) => AwItem::with_menu(value.trigger, menu.into_inner()),
            None => AwItem::new(value.trigger),
        };
        AwMenuBar::new(vec![root])
            .close_on_item_click_global(true)
            .close_on_background_click_global(true)
            .style(menu_bar_style)
            .into()
    }
}

pub type ContextMenu<'a, Message> = Dropdown<'a, Message>;

fn menu_button<'a, Message: 'a>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Button<'a, Message> {
    Button::new(Flex::row([content.into()]).width(Length::Fill))
        .width(Length::Fill)
        .height(24)
        .padding([0, 8])
        .style(menu_item_style)
        .clip(true)
}

fn menu_item_style(theme: &Theme, status: crate::button::Status) -> crate::button::Style {
    let p = theme.extended_palette();
    match status {
        crate::button::Status::Hovered | crate::button::Status::Pressed => crate::button::Style {
            background: Some(p.primary.weak.color.into()),
            text_color: p.primary.weak.text,
            ..crate::button::Style::default()
        },
        crate::button::Status::Active | crate::button::Status::Disabled => crate::button::Style {
            text_color: p.background.base.text,
            ..crate::button::Style::default()
        },
    }
}

pub fn menu_bar_style(theme: &Theme, _status: iced_aw::style::Status) -> Style {
    let p = theme.extended_palette();
    Style {
        bar_background: iced_core::Color::TRANSPARENT.into(),
        bar_border: iced_core::Border::default(),
        bar_shadow: iced_core::Shadow::default(),
        menu_background: p.background.weak.color.into(),
        menu_border: iced_core::Border {
            radius: 0.0.into(),
            width: 1.0,
            color: p.background.strong.color,
        },
        menu_shadow: iced_core::Shadow {
            color: iced_core::Color::BLACK.scale_alpha(0.25),
            offset: iced_core::Vector::new(3.0, 3.0),
            blur_radius: 0.0,
        },
        path: p.primary.weak.color.into(),
        path_border: iced_core::Border::default(),
    }
}
