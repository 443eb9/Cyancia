use std::sync::LazyLock;

use iced::{Element, Length, Task, Theme, window};
use lapiz_canvas::CanvasToolProxyAppExt;
use lapiz_dock::dock::{Dock, DockId};
use lapiz_i18n::t;
use lapiz_runtime::{Renderer, Services};
use lapiz_tools::{
    ErasedToolFunctionMessage, ToolFunctionRegistry, ToolId, manifest::ToolBoxManifestConfig,
};
use lapiz_widgets::{
    button::{self, Button},
    divider::Divider,
    flex::Flex,
    icon,
    label::Label,
    scrollable::Scrollable,
    tooltip::{Position, Tooltip},
};

pub static TOOL_BOX_DOCK_ID: LazyLock<DockId> =
    LazyLock::new(|| DockId::new("tool_box_dock".into()));

pub struct ToolBoxDock {
    manifest: ToolBoxManifestConfig,
}

pub enum ToolBoxDockMessage {
    Switch(ToolId),
    ToolFunction(ErasedToolFunctionMessage),
}

impl ToolBoxDock {
    pub fn new() -> Self {
        let manifest = ToolBoxManifestConfig::read_or_init_or_fallback();
        Self { manifest }
    }
}

impl Dock for ToolBoxDock {
    type Message = ToolBoxDockMessage;

    fn id(&self) -> DockId {
        TOOL_BOX_DOCK_ID.clone()
    }

    fn view<'a>(
        &'a self,
        _window_id: window::Id,
        services: &'a Services,
    ) -> Element<'a, Self::Message, Theme, Renderer> {
        let active_tool = services
            .current_tool_proxy()
            .and_then(|proxy| proxy.current_tool());
        let tool_button = |tool: &ToolId| {
            let selected = active_tool == Some(tool);
            let glyph = services
                .service::<ToolFunctionRegistry>()
                .icon(tool)
                .unwrap_or_else(icon::info)
                .size(12)
                .style(move |theme, _| {
                    let p = theme.palette();
                    icon::Style {
                        color: Some(if selected {
                            p.primary.base.text
                        } else {
                            p.background.weak.text
                        }),
                    }
                });
            let button = Button::new(glyph)
                .width(28)
                .height(28)
                .padding(8)
                .style(move |theme, status| {
                    let p = theme.palette();
                    let hovered =
                        matches!(status, button::Status::Hovered | button::Status::Pressed);
                    button::Style {
                        background: Some(
                            if selected {
                                p.primary.base.color
                            } else if hovered {
                                p.primary.weak.color
                            } else {
                                iced::Color::TRANSPARENT
                            }
                            .into(),
                        ),
                        text_color: if selected {
                            p.primary.base.text
                        } else {
                            p.background.weak.text
                        },
                        border: iced::Border {
                            radius: 0.0.into(),
                            width: if selected { 1.0 } else { 0.0 },
                            color: p.primary.base.color,
                        },
                        ..Default::default()
                    }
                })
                .on_press(ToolBoxDockMessage::Switch(tool.clone()));
            Tooltip::new(button, Label::new(t!(tool)), Position::Right).into()
        };
        let separator = || {
            Flex::row([Divider::horizontal(1).into()])
                .height(7)
                .padding([3, 0])
                .into()
        };
        let mut items = Vec::new();
        let manifest = self.manifest.get();
        for group in &manifest.groups {
            if !items.is_empty() {
                items.push(separator());
            }
            let buttons: Vec<_> = group.tools.iter().map(tool_button).collect();
            items.push(
                Flex::row(buttons)
                    .wrap()
                    .space_evenly()
                    .gap(1)
                    .width(Length::Fill)
                    .into(),
            );
        }
        let content = Flex::column(items).width(Length::Fill).gap(0).padding(4);

        Scrollable::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn update(&mut self, message: Self::Message, services: &mut Services) -> Task<Self::Message> {
        match message {
            ToolBoxDockMessage::Switch(tool) => services
                .update_current_tool_proxy(|proxy, services| proxy.switch_tool(tool, services))
                .unwrap_or_else(Task::none)
                .map(ToolBoxDockMessage::ToolFunction),
            ToolBoxDockMessage::ToolFunction(message) => services
                .update_current_tool_proxy(|proxy, services| {
                    proxy.handle_message(message, services)
                })
                .unwrap_or_else(Task::none)
                .map(ToolBoxDockMessage::ToolFunction),
        }
    }
}
