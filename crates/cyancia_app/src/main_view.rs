use std::{any::Any, sync::Arc};

use cyancia_actions::{
    ActionFunctionRegistry, ActionId,
    actions_matcher::ActionsMatcher,
    manifest::KeyBindingDefManifestCollection,
};
use cyancia_canvas::{
    CanvasManager,
    event::{CanvasCreated, CanvasRemoved},
    tools::PanTool,
};
use cyancia_dock::{
    DockManager, DockMessage,
    dock::{Dock, DockId},
};
use cyancia_runtime::{
    Services,
    event::Event,
    windows::{WindowView, WindowViewId},
};
use cyancia_tools::{ToolFunction, ToolProxies, ToolProxy};
use iced::{
    Element, Subscription, Task, Theme, event,
    keyboard::{self},
    mouse, window,
};
use iced_wgpu::Renderer;
use parking_lot::Mutex;

use crate::dock::{
    BrushPresetDock, CanvasDock, FiltersDock, LayersDock, ToolOptionsDock,
    construct_canvas_dock_id, BRUSH_PRESETS_DOCK_ID, FILTERS_DOCK_ID, LAYER_DOCK_ID,
    TOOL_OPTIONS_DOCK_ID,
};

pub struct MainView {
    dock_manager: DockManager<Theme, Renderer>,
    actions_matcher: Arc<Mutex<ActionsMatcher>>,
}

pub enum MainViewMessage {
    Dock(DockMessage),
    WindowEvent(window::Id, window::Event),
    KeyboardEvent(window::Id, keyboard::Event),
    MouseEvent(window::Id, mouse::Event),
    CanvasCreated(CanvasCreated),
    CanvasRemoved(CanvasRemoved),
    ActionMessage(ActionId, Box<dyn Any + Send + Sync>),
}

impl WindowView for MainView {
    type Message = MainViewMessage;

    fn id() -> WindowViewId {
        WindowViewId::new("main_view")
    }

    fn boot(services: &mut Services) -> (Self, Task<Self::Message>) {
        let actions = services
            .service::<KeyBindingDefManifestCollection>()
            .action_collection()
            .clone();
        let actions_matcher = Arc::new(Mutex::new(ActionsMatcher::new(actions)));

        let (main_window, task) = window::open(Default::default());
        let (mut dock_manager, dock_manager_task) = DockManager::new(main_window);
        dock_manager.register_dock(LayersDock);
        dock_manager.register_dock(FiltersDock);
        dock_manager.register_dock(ToolOptionsDock::new(services));
        dock_manager.register_dock(BrushPresetDock::new(services));

        let dock_tasks = Task::batch([
            dock_manager.open_dock(DockId::new(LAYER_DOCK_ID.into())),
            dock_manager.open_dock(DockId::new(FILTERS_DOCK_ID.into())),
            dock_manager.open_dock(DockId::new(TOOL_OPTIONS_DOCK_ID.into())),
            dock_manager.open_dock(DockId::new(BRUSH_PRESETS_DOCK_ID.into())),
        ])
        .map(MainViewMessage::Dock);

        (
            Self {
                dock_manager,
                actions_matcher,
            },
            Task::batch([
                task.discard(),
                dock_manager_task.map(MainViewMessage::Dock),
                dock_tasks,
            ]),
        )
    }

    fn view<'a>(
        &'a self,
        window: window::Id,
        services: &'a Services,
    ) -> impl Into<Element<'a, Self::Message, Theme, iced_wgpu::Renderer>> {
        Some(
            self.dock_manager
                .view(window, services)?
                .map(MainViewMessage::Dock),
        )
    }

    fn update(
        &mut self,
        message: Self::Message,
        services: &mut Services,
    ) -> impl Into<Task<Self::Message>> {
        match message {
            MainViewMessage::Dock(m) => self
                .dock_manager
                .update(m, services)
                .map(MainViewMessage::Dock),
            MainViewMessage::WindowEvent(id, event) => {
                match event {
                    window::Event::Focused => {
                        self.actions_matcher.lock().reset_keyboard_state();
                    }
                    _ => {}
                }

                self.dock_manager.on_window_event(id, event).discard()
            }

            MainViewMessage::KeyboardEvent(window, event) => {
                if let Some(action) = self.actions_matcher.lock().on_keyboard_event(event) {
                    if let Some(action_func) = services
                        .service_mut::<ActionFunctionRegistry>()
                        .get(action.clone())
                    {
                        log::info!("Triggering action: {}", action.0);
                        return action_func.trigger(services).map(move |message| {
                            MainViewMessage::ActionMessage(action.clone(), message)
                        });
                    } else {
                        log::warn!("No action function found for action: {}", action.0);
                    }
                }
                Task::none()
            }
            MainViewMessage::MouseEvent(window, event) => {
                match event {
                    mouse::Event::CursorMoved { position } => {
                        return self
                            .dock_manager
                            .on_cursor_moved(window, position)
                            .map(MainViewMessage::Dock);
                    }
                    mouse::Event::ButtonReleased(mouse::Button::Left) => {
                        return self
                            .dock_manager
                            .on_float_window_drag_end()
                            .map(MainViewMessage::Dock);
                    }
                    _ => {}
                }

                Task::none()
            }
            MainViewMessage::CanvasCreated(e) => {
                log::info!("Canvas created: {}", e.id);
                let tool_proxy_id = services
                    .service::<CanvasManager>()
                    .get(&e.id)
                    .map(|canvas| canvas.tool_proxy_id());
                if let Some(tool_proxy_id) = tool_proxy_id {
                    let _ = services.service_scope::<ToolProxies, _>(|tool_proxies, services| {
                        tool_proxies
                            .get_mut(&tool_proxy_id)
                            .switch_tool(PanTool::id(), services)
                    });
                }
                let dock = CanvasDock::new(e.id, self.actions_matcher.clone());
                let id = <CanvasDock as Dock<Theme, Renderer>>::id(&dock);
                self.dock_manager.register_dock(dock);
                self.dock_manager.open_dock(id).map(MainViewMessage::Dock)
            }
            MainViewMessage::CanvasRemoved(e) => {
                log::info!("Canvas removed: {}", e.id);
                let id = DockId::new(construct_canvas_dock_id(e.id).into());
                self.dock_manager.unregister_dock(&id);

                Task::none()
            }
            MainViewMessage::ActionMessage(action_id, message) => {
                if let Some(action_func) = services
                    .service_mut::<ActionFunctionRegistry>()
                    .get(action_id.clone())
                {
                    action_func
                        .handle_message(message, services)
                        .map(move |message| {
                            MainViewMessage::ActionMessage(action_id.clone(), message)
                        })
                } else {
                    Task::none()
                }
            }
        }
    }

    fn close(self, services: &mut Services) -> Task<()> {
        iced::exit()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let external = iced::event::listen_with(|event, status, window| match event {
            iced::Event::Window(e) => Some(MainViewMessage::WindowEvent(window, e)),
            iced::Event::Keyboard(e) => Some(MainViewMessage::KeyboardEvent(window, e)),
            iced::Event::Mouse(e) => Some(MainViewMessage::MouseEvent(window, e)),
            _ => None,
        });

        let dock = self.dock_manager.subscription().map(MainViewMessage::Dock);
        let canvas_create = CanvasCreated::listen_to().map(MainViewMessage::CanvasCreated);
        let canvas_remove = CanvasRemoved::listen_to().map(MainViewMessage::CanvasRemoved);

        Subscription::batch([external, dock, canvas_create, canvas_remove])
    }

    fn windows(&self) -> Arc<[iced_core::window::Id]> {
        self.dock_manager
            .window_infos()
            .map(|i| i.id)
            .collect::<Vec<_>>()
            .into()
    }

    fn root_window(&self) -> Option<iced_core::window::Id> {
        Some(self.dock_manager.main_window().id)
    }
}