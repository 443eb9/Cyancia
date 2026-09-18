use std::sync::Arc;

use anyhow::Result;
use iced_core::{Element, Size, Theme, window};
use iced_futures::Subscription;
use iced_runtime::Task;
use iced_widget::column;
use lapiz_runtime::{
    Services,
    windows::{WindowView, WindowViewId},
};

/// Stub editor: the real editor UI is being rebuilt on another branch. The
/// window stays registered so panel entry points keep working; it renders
/// nothing and handles no messages.
pub struct FilterEditor {
    windows: Arc<[window::Id]>,
    main_window: window::Id,
}

#[derive(Clone)]
pub enum FilterEditorMessage {}

/// The message enum is uninhabited, so this only gives the never value a
/// concrete return type for the trait signature.
fn never(message: FilterEditorMessage) -> Task<FilterEditorMessage> {
    match message {}
}

impl WindowView for FilterEditor {
    type Message = FilterEditorMessage;

    type BootParams = ();

    fn id() -> WindowViewId {
        WindowViewId::new("filter_editor")
    }

    fn boot(
        _params: Option<Self::BootParams>,
        _services: &mut Services,
    ) -> Result<(Self, Task<Self::Message>)> {
        let (main_window, open) = iced_runtime::window::open(window::Settings {
            size: Size {
                width: 720.0,
                height: 480.0,
            },
            ..Default::default()
        });
        Ok((
            Self {
                windows: [main_window].into(),
                main_window,
            },
            open.discard(),
        ))
    }

    fn view<'a>(
        &'a self,
        _: window::Id,
        _: &'a Services,
    ) -> impl Into<Element<'a, Self::Message, Theme, lapiz_runtime::Renderer>> {
        column![]
    }

    fn update(
        &mut self,
        message: Self::Message,
        _services: &mut Services,
    ) -> impl Into<Task<Self::Message>> {
        never(message)
    }

    fn subscription(&self, _services: &Services) -> Subscription<Self::Message> {
        Subscription::none()
    }

    fn close(self, _: &mut Services) -> Task<()> {
        iced_runtime::window::close(self.main_window)
    }

    fn windows(&self) -> Arc<[window::Id]> {
        self.windows.clone()
    }

    fn root_window(&self) -> Option<window::Id> {
        Some(self.main_window)
    }
}
