use std::{
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use iced_core::{Element, Length, window};
use iced_futures::Subscription;
use iced_runtime::Task;
use lapiz_canvas::{CCanvas, CanvasAppExt, event::CanvasCreated};
use lapiz_config::{Config, Configuration};
use lapiz_dock::dock::{Dock, DockId};
use lapiz_image::{
    CImage,
    texel::TexelType,
    tile::{GpuLayerInfo, TileStorageAppExt},
};
use lapiz_runtime::{Renderer, Services, Theme, event::Event};
use lapiz_tools::{ToolFunctionRegistry, ToolProxies, ToolProxy};
use lapiz_undo::{UndoStack, UndoStacks};
use lapiz_utils::log_err::LogErr;
use lapiz_widgets::{button::Button, flex::Flex, label::Label};
use serde::{Deserialize, Serialize};

pub struct LandingDock {
    config: Config<RecentFiles>,
    files: Arc<RecentFiles>,
}

impl LandingDock {
    pub fn new() -> Self {
        let config = Config::<RecentFiles>::read_or_init_or_fallback();
        Self {
            files: config.get(),
            config,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentFiles {
    pub files: Vec<RecentFileRecord>,
}

impl Configuration for RecentFiles {
    const NAME: &'static str = "recent_files.toml";

    const DEFAULT: &'static str = "files = []";
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentFileRecord {
    pub path: PathBuf,
}

pub enum LandingDockMessage {
    OpenFile(usize),
    RecentFilesChanged,
}

static LANDING_DOCK_ID: LazyLock<DockId> = LazyLock::new(|| DockId::new("landing_dock".into()));

impl Dock for LandingDock {
    type Message = LandingDockMessage;

    fn id(&self) -> DockId {
        LANDING_DOCK_ID.clone()
    }

    fn view<'a>(
        &'a self,
        window_id: window::Id,
        services: &'a Services,
    ) -> Element<'a, Self::Message, Theme, Renderer> {
        Flex::row(self.files.files.iter().enumerate().map(|(i, f)| {
            let file_name = f.path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            Button::new(Label::new(file_name))
                .on_press(LandingDockMessage::OpenFile(i))
                .into()
        }))
        .wrap()
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

                // TODO This is copied from OpenFileAction and should be avoided
                let Ok((image, archive)) = CImage::from_file(&file.path, services).logged_err()
                else {
                    return Task::none();
                };
                log::info!("Opened image from file {:?}.", file.path);

                let canvas = CCanvas::new(file.path.clone(), image, archive);
                let canvas_id = canvas.id();
                let tool_proxy = ToolProxy::new(services.service::<ToolFunctionRegistry>());
                services
                    .service_mut::<ToolProxies>()
                    .insert(*canvas_id, tool_proxy);
                let undo_stack = UndoStack::new(*canvas_id, 200);
                services
                    .service_mut::<UndoStacks>()
                    .insert(*canvas_id, undo_stack);

                // TODO this should not be done here
                let tiles = services.tile_storage();
                for layer in canvas.image.layer_stack().iter_layers() {
                    tiles.declare_layer(
                        *layer.id(),
                        GpuLayerInfo {
                            // TODO
                            texel_type: TexelType::RGBA8,
                        },
                    );
                }
                tiles.declare_layer(
                    canvas.image.selection_layer(),
                    GpuLayerInfo {
                        // TODO This will change when image depth is not 8 bit
                        texel_type: TexelType::A8,
                    },
                );

                services.add_canvas(canvas);
                CanvasCreated::broadcast(CanvasCreated { id: canvas_id });

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
