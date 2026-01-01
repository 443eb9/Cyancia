use std::{fmt::Debug, sync::Arc};

use cyancia_actions::{
    ActionFunctionCollection,
    canvas_control::{BrushToolAction, CanvasToolSwitch, PanToolAction},
    file::OpenFileAction,
    shell::{CShell, DestructedShell},
    task::ActionTask,
};
use cyancia_assets::store::{AssetLoaderRegistry, AssetRegistry};
use cyancia_canvas::{CCanvas, widget::CanvasWidget};
use cyancia_id::Id;
use cyancia_image::{
    CImage,
    tile::{GPU_TILE_STORAGE, GpuTileStorage},
};
use cyancia_input::{
    action::{Action, ActionCollection, ActionManifest},
    key::{KeySequence, KeyboardState},
};
use cyancia_render::{
    RENDER_CONTEXT, RenderContext,
    renderer_acquire::RendererAcquire,
    resources::{GLOBAL_SAMPLERS, GlobalSamplers},
};
use cyancia_tools::{CanvasToolFunctionCollection, ToolProxy, brush::BrushTool, pan::PanTool};
use glam::UVec2;
use iced::{
    Element, Point, Renderer, Subscription, Task, Theme, event,
    keyboard::{self, key},
    mouse, window,
};

use crate::input_manager::InputManager;

pub struct MainView {
    pub assets: AssetRegistry,
    pub input_manager: InputManager,
    pub canvas: Arc<CCanvas>,

    pub renderer_acquired: bool,
}

pub enum MainViewMessage {
    RendererAcquired(Arc<wgpu::Device>, Arc<wgpu::Queue>),
    WindowOpened(window::Id),
    KeyboardEvent(keyboard::Event),
    MouseEvent(mouse::Event),
    ActionTaskCompleted(Option<Box<dyn ActionTask>>),
}

impl Debug for MainViewMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RendererAcquired(arg0, arg1) => f
                .debug_tuple("RendererAcquired")
                .field(arg0)
                .field(arg1)
                .finish(),
            Self::WindowOpened(arg0) => f.debug_tuple("WindowOpened").field(arg0).finish(),
            Self::KeyboardEvent(arg0) => f.debug_tuple("KeyboardEvent").field(arg0).finish(),
            Self::MouseEvent(arg0) => f.debug_tuple("MouseEvent").field(arg0).finish(),
            Self::ActionTaskCompleted(arg0) => f.debug_tuple("ActionTaskCompleted").finish(),
        }
    }
}

impl MainView {
    pub fn new() -> Self {
        let mut loaders = AssetLoaderRegistry::new();
        cyancia_input::register_loaders(&mut loaders);
        let assets = AssetRegistry::new("assets", &loaders);

        let actions = {
            let mut collection = ActionFunctionCollection::new(ActionCollection::new(
                assets.store::<ActionManifest>().clone(),
            ));
            collection.register::<OpenFileAction>();
            collection.register::<CanvasToolSwitch<PanToolAction>>();
            collection.register::<CanvasToolSwitch<BrushToolAction>>();
            collection
        };
        let tool_functions = {
            let mut c = CanvasToolFunctionCollection::new();
            c.register::<BrushTool>();
            c.register::<PanTool>();
            c
        };
        let tools = { ToolProxy::new(Id::from_str("brush_tool"), tool_functions) };

        Self {
            assets,
            canvas: Arc::new(CCanvas {
                image: Arc::new(CImage::new(UVec2 { x: 1024, y: 768 })),
                transform: Default::default(),
            }),
            input_manager: InputManager::new(actions, tools),

            renderer_acquired: false,
        }
    }

    pub fn view(&self) -> Element<'_, MainViewMessage, Theme, iced_wgpu::Renderer> {
        if self.renderer_acquired {
            self.view_internal()
        } else {
            Element::new(RendererAcquire {
                on_acquire: Box::new(|device, queue| {
                    log::info!("Renderer acquired!");
                    MainViewMessage::RendererAcquired(Arc::new(device), Arc::new(queue))
                }),
            })
        }
    }

    fn view_internal(&self) -> Element<'_, MainViewMessage, Theme, iced_wgpu::Renderer> {
        CanvasWidget {
            canvas: self.canvas.clone(),
            gpu_tile_storage: GPU_TILE_STORAGE.clone_arc(),
        }
        .into()
    }

    pub fn update(&mut self, message: MainViewMessage) -> Task<MainViewMessage> {
        match message {
            MainViewMessage::WindowOpened(id) => {}
            MainViewMessage::RendererAcquired(device, queue) => {
                if !self.renderer_acquired {
                    self.renderer_acquired = true;

                    GLOBAL_SAMPLERS.init(GlobalSamplers::new(&device));
                    GPU_TILE_STORAGE.init(GpuTileStorage::new(device.clone(), queue.clone()));
                    RENDER_CONTEXT.init(RenderContext { device, queue });
                }
            }
            MainViewMessage::KeyboardEvent(event) => {
                let shell = self
                    .input_manager
                    .on_keyboard_event(event, self.canvas.clone());

                return self
                    .apply_shell(shell)
                    .map(|x| MainViewMessage::ActionTaskCompleted(x));
            }
            MainViewMessage::MouseEvent(event) => {
                self.input_manager.on_mouse_event(event, &self.canvas);
            }
            MainViewMessage::ActionTaskCompleted(action_task) => {
                if let Some(action_task) = action_task {
                    let mut shell = CShell::new(self.canvas.clone(), &mut self.input_manager.tools);
                    action_task.apply(&mut shell);
                    let shell = shell.destruct();
                    return self
                        .apply_shell(shell)
                        .map(|x| MainViewMessage::ActionTaskCompleted(x));
                }
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<MainViewMessage> {
        event::listen().filter_map(|event| match event {
            iced::Event::Keyboard(event) => Some(MainViewMessage::KeyboardEvent(event)),
            iced::Event::Mouse(event) => Some(MainViewMessage::MouseEvent(event)),
            _ => None,
        })
    }

    // fn create_shell(&self) -> CShell {
    //     CShell::new(self.canvas.clone())
    // }

    // TODO
    fn apply_shell(&mut self, shell: DestructedShell) -> Task<Option<Box<dyn ActionTask>>> {
        self.canvas = shell.current_canvas;
        for canvas in shell.canvases {
            log::info!("Canvas created: {:?}", canvas);
            self.canvas = canvas;
        }
        Task::batch(shell.tasks)
    }
}
