use std::sync::LazyLock;

use iced::{Element, Length, Task, Theme, window};
use iced_widget::space;
use lapiz_canvas::CanvasToolProxyAppExt as _;
use lapiz_dock::dock::{Dock, DockId};
use lapiz_i18n::t;
use lapiz_runtime::Services;
use lapiz_tools::ErasedToolFunctionMessage;
use lapiz_widgets::{label::Label, panel::Panel, scrollable::Scrollable};

pub static TOOL_OPTIONS_DOCK_ID: LazyLock<DockId> =
    LazyLock::new(|| DockId::new("tool_options_dock".into()));

pub struct ToolOptionsDock;

pub enum ToolOptionsDockMessage {
    ToolFunction(ErasedToolFunctionMessage),
}

impl ToolOptionsDock {
    pub fn new(_: &Services) -> Self {
        Self
    }
}

impl Dock for ToolOptionsDock {
    type Message = ToolOptionsDockMessage;

    fn id(&self) -> DockId {
        TOOL_OPTIONS_DOCK_ID.clone()
    }

    fn view<'a>(
        &'a self,
        _window_id: window::Id,
        services: &'a Services,
    ) -> Element<'a, Self::Message, Theme, lapiz_runtime::Renderer> {
        let Some(tool_proxy) = services.current_tool_proxy() else {
            return space().into();
        };

        let Some(widget) = tool_proxy.tool_option_widget(services) else {
            return Panel::new(Label::new(t!("no_tool_options")).muted())
                .padding(8)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        };

        Panel::new(Scrollable::new(
            widget.map(ToolOptionsDockMessage::ToolFunction),
        ))
        .padding(4)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn update(&mut self, message: Self::Message, services: &mut Services) -> Task<Self::Message> {
        match message {
            ToolOptionsDockMessage::ToolFunction(message) => services
                .update_current_tool_proxy(|tool_proxy, services| {
                    tool_proxy.handle_message(message, services)
                })
                .unwrap_or_else(Task::none)
                .map(ToolOptionsDockMessage::ToolFunction),
        }
    }
}
