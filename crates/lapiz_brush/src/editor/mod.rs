use std::sync::Arc;

use anyhow::Result;
use iced_core::{Element, Size, Theme, window};
use iced_futures::Subscription;
use iced_runtime::Task;
use lapiz_runtime::{
    Services,
    windows::{WindowView, WindowViewId},
};

/// Stub editor: the real editor UI is being rebuilt on top of the effect
/// system. The window stays registered so panel entry points keep working; it
/// renders nothing and handles no messages.
pub struct BrushEditor {
    windows: Arc<[window::Id]>,
    main_window: window::Id,
}

#[derive(Clone)]
pub enum BrushEditorMessage {}

// The message enum is uninhabited, so this only gives the never value a
// concrete return type for the trait signature.
fn never(message: BrushEditorMessage) -> Task<BrushEditorMessage> {
    match message {}
}

impl WindowView for BrushEditor {
    type Message = BrushEditorMessage;

    type BootParams = ();

    fn id() -> WindowViewId {
        WindowViewId::new("brush_editor")
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
        iced_widget::column![]
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
