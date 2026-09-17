use std::sync::{Arc, LazyLock};

use iced_core::{Element, Length, window};
use iced_futures::Subscription;
use iced_runtime::Task;
use iced_widget::scrollable;
use lapiz_canvas::{CCanvas, CanvasAppExt, event::CanvasCreated, recent::RecentFiles};
use lapiz_config::Config;
use lapiz_dock::dock::{Dock, DockId};
use lapiz_image::{
    CImage,
    texel::TexelType,
    tile::{GpuLayerInfo, TileStorageAppExt},
};
use lapiz_image_importer::start_import;
use lapiz_runtime::{Renderer, Services, Theme, event::Event};
use lapiz_tools::{ToolFunctionRegistry, ToolProxies, ToolProxy};
use lapiz_undo::{UndoStack, UndoStacks};
use lapiz_utils::log_err::LogErr;
use lapiz_widgets::{button::Button, flex::Flex, label::Label};

pub struct LandingDock {
    config: Config<RecentFiles>,
    files: Arc<RecentFiles>,
}

impl LandingDock {
    #[allow(
        clippy::new_without_default,
        reason = "Default cannot express the semantic of reading config from disk."
    )]
    pub fn new() -> Self {
        let config = Config::<RecentFiles>::read_or_init_or_fallback();
        Self {
            files: config.get(),
            config,
        }
    }
}

pub enum LandingDockMessage {
    OpenFile(usize),
    RecentFilesChanged,
}

pub static RECENT_FILES_DOCK_ID: LazyLock<DockId> =
    LazyLock::new(|| DockId::new("recent_files_dock".into()));

impl Dock for LandingDock {
    type Message = LandingDockMessage;

    fn id(&self) -> DockId {
        RECENT_FILES_DOCK_ID.clone()
    }

    fn view<'a>(
        &'a self,
        _window_id: window::Id,
        _services: &'a Services,
    ) -> Element<'a, Self::Message, Theme, Renderer> {
        scrollable(
            Flex::row(self.files.files.iter().enumerate().map(|(i, f)| {
                let file_name = f.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                Button::new(Label::new(file_name))
                    .width(100.0)
                    .height(140.0)
                    .on_press(LandingDockMessage::OpenFile(i))
                    .into()
            }))
            .wrap()
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn update(&mut self, message: Self::Message, services: &mut Services) -> Task<Self::Message> {
        match message {
            LandingDockMessage::OpenFile(i) => {
                let Some(file) = self.files.files.get(i) else {
                    return Task::none();
                };

                start_import(services, file.path.clone());

                Task::none()
            }
            LandingDockMessage::RecentFilesChanged => {
                self.files = self.config.get();
                Task::none()
            }
        }
    }

    fn subscription(&self, _services: &Services) -> Subscription<Self::Message> {
        self.config
            .listen_to()
            .map(|_| LandingDockMessage::RecentFilesChanged)
    }
}
