use std::{any::Any, sync::Arc};

use iced::keyboard::key;
use iced::{
    Element, Length, Subscription, Task, Theme,
    keyboard::{self},
    mouse, window,
};
use iced_wgpu::Renderer;
use iced_widget::pane_grid;
use lapiz_actions::{
    ActionFunctionRegistry, ActionId,
    manifest::{ActionCollection, KeyBindingDefManifest},
};
use lapiz_assets::AssetAppExt;
use lapiz_canvas::CanvasToolProxyAppExt;
use lapiz_canvas::{
    event::{CanvasCreated, CanvasRemoved},
    tools::PanTool,
};
use lapiz_dock::{
    DockManager, DockMessage,
    dock::{Dock, DockId, ResizeHandleOverlay},
};
use lapiz_input::key::KeyboardState;
use lapiz_runtime::{
    ApplicationTheme, Services,
    event::Event,
    windows::{WindowView, WindowViewId},
};
use lapiz_tools::{ErasedToolFunctionMessage, GlobalToolBindings, ToolFunction};
use lapiz_widgets::{
    bar::StatusBar,
    flex::Flex,
    kbd::Kbd,
    label::Label,
    menu::{Menu, MenuBar},
    title_bar::TitleBar,
};

use crate::dock::{
    BRUSH_PRESETS_DOCK_ID, BrushPresetDock, COLOR_SELECTOR_DOCK_ID, CanvasDock, ColorSelectorDock,
    LAYER_DOCK_ID, LayersDock, TOOL_BOX_DOCK_ID, TOOL_OPTIONS_DOCK_ID, ToolBoxDock,
    ToolOptionsDock, construct_canvas_dock_id,
};

pub struct MainView {
    dock_manager: DockManager<Theme, Renderer>,
    action_collection: ActionCollection,
    canvas_group_anchor: Option<DockId>,
}

pub enum MainViewMessage {
    Dock(DockMessage),
    WindowEvent(window::Id, window::Event),
    KeyboardEvent(window::Id, keyboard::Event),
    MouseEvent(window::Id, mouse::Event),
    CanvasCreated(CanvasCreated),
    CanvasRemoved(CanvasRemoved),
    TriggerAction(ActionId),
    SetTheme(Theme),
    ActionMessage(ActionId, Box<dyn Any + Send + Sync>),
    ToolFunctionMessage(ErasedToolFunctionMessage),
    DragWindow(window::Id),
    MinimizeWindow(window::Id),
    MaximizeWindow(window::Id),
    CloseWindow(window::Id),
    ResizeWindow(window::Id, window::Direction),
}

impl MainView {
    fn action(name: &'static str) -> MainViewMessage {
        MainViewMessage::TriggerAction(ActionId::new(name.into()))
    }

    fn menu_bar<'a>(current_theme: &Theme) -> MenuBar<'a, MainViewMessage> {
        let row = |label: &'static str, shortcut: &'static str| {
            Flex::row([Label::new(label).into(), Kbd::new(shortcut).into()])
                .width(Length::Fill)
                .space_between()
        };
        let file = Menu::new()
            .disabled_item(row("New Canvas…", "Ctrl+N"))
            .item(row("Open…", "Ctrl+O"), Self::action("OpenFileAction"))
            .separator()
            .item(row("Save", "Ctrl+S"), Self::action("SaveFileAction"))
            .width(236);
        let edit = Menu::new()
            .item(row("Undo", "Ctrl+Z"), Self::action("UndoAction"))
            .item(row("Redo", "Ctrl+Shift+Z"), Self::action("RedoAction"))
            .separator()
            .item(
                row("Paste as New Layer", "Ctrl+V"),
                Self::action("PasteIntoNewLayerAction"),
            )
            .width(236);
        let view = Menu::new()
            .disabled_item(row("Zoom In", "Ctrl++"))
            .disabled_item(row("Zoom Out", "Ctrl+-"))
            .disabled_item(row("Fit on Screen", "Ctrl+0"))
            .width(236);
        let layer = Menu::new()
            .item(
                row("New Paint Layer", "Ctrl+Shift+N"),
                Self::action("CreateNewLayerAction"),
            )
            .item(
                row("Group Selected Layers", "Ctrl+G"),
                Self::action("GroupSelectedLayersAction"),
            )
            .separator()
            .item(
                row("Move Up", "Ctrl+Shift+Up"),
                Self::action("MoveLayerUpAction"),
            )
            .item(
                row("Move Down", "Ctrl+Shift+Down"),
                Self::action("MoveLayerDownAction"),
            )
            .item(
                row("Delete Selected Layers", "Ctrl+Delete"),
                Self::action("DeleteSelectedLayersAction"),
            )
            .width(236);
        let select = Menu::new()
            .item(
                row("Select Previous Layer", "Ctrl+Up"),
                Self::action("SelectPreviousLayerAction"),
            )
            .item(
                row("Select Next Layer", "Ctrl+Down"),
                Self::action("SelectNextLayerAction"),
            )
            .separator()
            .item(
                row("Delete Selection", "Ctrl+D"),
                Self::action("DeleteSelectionAction"),
            )
            .width(236);
        let filter = Menu::new()
            .item(
                row("Filter Panel…", "Ctrl+Shift+F"),
                Self::action("ToggleFilterPanelAction"),
            )
            .width(236);
        let themes = Theme::ALL
            .iter()
            .fold(Menu::new().width(220), |menu, theme| {
                let label = Label::new(theme.to_string());
                if theme == current_theme {
                    menu.selected_item(label, MainViewMessage::SetTheme(theme.clone()))
                } else {
                    menu.item(label, MainViewMessage::SetTheme(theme.clone()))
                }
            });
        let window = Menu::new()
            .item(
                row("Brush Editor", "F5"),
                Self::action("OpenBrushEditorAction"),
            )
            .item(
                row("Filter Panel", "Ctrl+Shift+F"),
                Self::action("ToggleFilterPanelAction"),
            )
            .separator()
            .submenu(Label::new("Theme"), themes)
            .width(236);
        let help = Menu::new()
            .disabled_item(Label::new("Documentation"))
            .separator()
            .disabled_item(Label::new("About Lapiz"));

        MenuBar::new()
            .menu(Label::new("File"), file)
            .menu(Label::new("Edit"), edit)
            .menu(Label::new("View"), view)
            .menu(Label::new("Layer"), layer)
            .menu(Label::new("Select"), select)
            .menu(Label::new("Filter"), filter)
            .menu(Label::new("Window"), window)
            .menu(Label::new("Help"), help)
    }

    fn switch_tool_keys(
        &mut self,
        services: &mut Services,
        is_keydown: bool,
    ) -> Task<MainViewMessage> {
        services
            .update_current_tool_proxy(|tool_proxy, services| {
                let keyboard_state = services.service::<KeyboardState>();
                let seq = keyboard_state.get_sequence();

                let config = services
                    .service::<GlobalToolBindings>()
                    .binding_for(seq)
                    .cloned();
                let Some(config) = config else {
                    return tool_proxy.switch_override_tool(None, services);
                };

                if config.is_temporary {
                    tool_proxy.switch_override_tool(Some(config.tool.clone()), services)
                } else if is_keydown {
                    tool_proxy.switch_tool(config.tool.clone(), services)
                } else {
                    Task::none()
                }
            })
            .unwrap_or_else(Task::none)
            .map(MainViewMessage::ToolFunctionMessage)
    }
}

impl WindowView for MainView {
    type Message = MainViewMessage;

    fn id() -> WindowViewId {
        WindowViewId::new("main_view")
    }

    fn boot(services: &mut Services) -> (Self, Task<Self::Message>) {
        let assets = services.assets();
        let manifests = assets.all_handles_of::<KeyBindingDefManifest>().unwrap();
        let manifest = manifests.first().unwrap().get().unwrap();

        log::info!(
            "Loading {} key bindings from manifest {}",
            manifest.actions.len(),
            manifest.name
        );
        let action_collection = ActionCollection::new(&manifest);

        let (main_window, task) = window::open(window::Settings {
            decorations: false,
            size: iced::Size::new(1280.0, 800.0),
            #[cfg(target_os = "windows")]
            platform_specific: window::settings::PlatformSpecific {
                corner_preference: window::settings::platform::CornerPreference::DoNotRound,
                ..Default::default()
            },
            ..Default::default()
        });
        let (mut dock_manager, dock_manager_task) = DockManager::new(main_window);
        dock_manager.register_dock(LayersDock::new());
        dock_manager.register_dock(ToolBoxDock::new());
        dock_manager.register_dock(ToolOptionsDock::new(services));
        dock_manager.register_dock(BrushPresetDock::new(services));
        dock_manager.register_dock(ColorSelectorDock::new(services));

        let tool_options = DockId::new(TOOL_OPTIONS_DOCK_ID.into());
        let tools = DockId::new(TOOL_BOX_DOCK_ID.into());
        let color = DockId::new(COLOR_SELECTOR_DOCK_ID.into());
        let brushes = DockId::new(BRUSH_PRESETS_DOCK_ID.into());
        let layers = DockId::new(LAYER_DOCK_ID.into());
        let dock_tasks = Task::batch([
            dock_manager.open_dock(tool_options.clone()),
            dock_manager.open_dock_split(tools, &tool_options, pane_grid::Edge::Left, 0.06),
            dock_manager.open_dock_split(
                color.clone(),
                &tool_options,
                pane_grid::Edge::Right,
                0.76,
            ),
            dock_manager.open_dock_split(brushes.clone(), &color, pane_grid::Edge::Bottom, 0.34),
            dock_manager.open_dock_split(layers, &brushes, pane_grid::Edge::Bottom, 0.5),
        ])
        .map(MainViewMessage::Dock);

        (
            Self {
                dock_manager,
                action_collection,
                canvas_group_anchor: None,
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
        let dock = self
            .dock_manager
            .view(window, services)?
            .map(MainViewMessage::Dock);

        if window != self.dock_manager.main_window().id {
            return Some(dock);
        }

        let title_content = Flex::row([
            Label::new("LAPIZ").size(13).strong().into(),
            Self::menu_bar(&services.service::<ApplicationTheme>().0)
                .height(Length::Fill)
                .into(),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .gap(12)
        .padding([0, 10]);
        let title = TitleBar::new(title_content)
            .on_drag(MainViewMessage::DragWindow(window))
            .on_minimize(MainViewMessage::MinimizeWindow(window))
            .on_maximize(MainViewMessage::MaximizeWindow(window))
            .on_close(MainViewMessage::CloseWindow(window));

        let status = StatusBar::new([
            Label::new("READY").size(10).accent().into(),
            Label::new("Lapiz painting workspace")
                .size(10)
                .muted()
                .into(),
        ]);

        let content: Element<'a, MainViewMessage, Theme, Renderer> =
            Flex::column([title.into(), dock, status.into()])
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        Some(
            iced_widget::stack![
                content,
                Element::new(ResizeHandleOverlay::new(move |direction| {
                    MainViewMessage::ResizeWindow(window, direction)
                })),
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
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
                self.dock_manager.on_window_event(id, event).discard()
            }

            MainViewMessage::KeyboardEvent(_window, event) => {
                let keyboard_state = services.service_mut::<KeyboardState>();
                let old_modifier_count = keyboard_state.modifiers().bits().count_ones();

                match &event {
                    keyboard::Event::KeyPressed {
                        physical_key: key::Physical::Code(code),
                        repeat: false,
                        ..
                    } => {
                        if *code == key::Code::ControlLeft
                            || *code == key::Code::ControlRight
                            || *code == key::Code::ShiftLeft
                            || *code == key::Code::ShiftRight
                            || *code == key::Code::AltLeft
                            || *code == key::Code::AltRight
                            || *code == key::Code::SuperLeft
                            || *code == key::Code::SuperRight
                            || *code == key::Code::Meta
                        {
                            return Task::none();
                        }
                        keyboard_state.press(*code);

                        // TODO prevent any action from triggering when a tool is updating.
                        if let Some(action) = self
                            .action_collection
                            .get_action_id(keyboard_state.get_sequence())
                            && let Some(action_func) = services
                                .service_mut::<ActionFunctionRegistry>()
                                .get(action.clone())
                        {
                            log::info!("Triggering action: {}", action.0);
                            return action_func.trigger(services).map(move |message| {
                                MainViewMessage::ActionMessage(action.clone(), message)
                            });
                        }

                        self.switch_tool_keys(services, true)
                    }
                    keyboard::Event::KeyReleased {
                        physical_key: key::Physical::Code(code),
                        ..
                    } => {
                        keyboard_state.release(*code);
                        self.switch_tool_keys(services, false)
                    }
                    keyboard::Event::ModifiersChanged(modifiers) => {
                        keyboard_state.set_modifiers(*modifiers);

                        let new_modifier_count = keyboard_state.modifiers().bits().count_ones();
                        let is_keydown = new_modifier_count > old_modifier_count;
                        self.switch_tool_keys(services, is_keydown)
                    }
                    _ => Task::none(),
                }
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
                let tool_task = services
                    .update_tool_proxy(&e.id, |tool_proxy, services| {
                        tool_proxy.switch_tool(PanTool::id(), services)
                    })
                    .unwrap_or_else(Task::none);
                let dock = CanvasDock::new(e.id, self.dock_manager.main_window().id);
                let id = <CanvasDock as Dock<Theme, Renderer>>::id(&dock);
                self.dock_manager.register_dock(dock);

                let target = self
                    .canvas_group_anchor
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| DockId::new(TOOL_OPTIONS_DOCK_ID.into()));
                let dock_task = self
                    .dock_manager
                    .open_dock_in_group(id.clone(), &target)
                    .map(MainViewMessage::Dock);
                self.canvas_group_anchor = Some(id);
                Task::batch([
                    tool_task.map(MainViewMessage::ToolFunctionMessage),
                    dock_task,
                ])
            }
            MainViewMessage::CanvasRemoved(e) => {
                log::info!("Canvas removed: {}", e.id);
                let id = DockId::new(construct_canvas_dock_id(e.id).into());
                self.dock_manager.unregister_dock(&id);
                if self.canvas_group_anchor.as_ref() == Some(&id) {
                    self.canvas_group_anchor = None;
                }

                Task::none()
            }
            MainViewMessage::SetTheme(theme) => {
                services.service_mut::<ApplicationTheme>().0 = theme;
                Task::none()
            }
            MainViewMessage::TriggerAction(action_id) => {
                if let Some(action_func) = services
                    .service_mut::<ActionFunctionRegistry>()
                    .get(action_id.clone())
                {
                    action_func.trigger(services).map(move |message| {
                        MainViewMessage::ActionMessage(action_id.clone(), message)
                    })
                } else {
                    Task::none()
                }
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
            MainViewMessage::ToolFunctionMessage(message) => services
                .update_current_tool_proxy(|tool_proxy, services| {
                    tool_proxy.handle_message(message, services)
                })
                .unwrap_or_else(Task::none)
                .map(MainViewMessage::ToolFunctionMessage),
            MainViewMessage::DragWindow(id) => window::drag(id),
            MainViewMessage::MinimizeWindow(id) => window::minimize(id, true),
            MainViewMessage::MaximizeWindow(id) => window::toggle_maximize(id),
            MainViewMessage::CloseWindow(id) => window::close(id),
            MainViewMessage::ResizeWindow(id, direction) => window::drag_resize(id, direction),
        }
    }

    fn close(self, _services: &mut Services) -> Task<()> {
        iced::exit()
    }

    fn subscription(&self, services: &Services) -> Subscription<Self::Message> {
        let external = iced::event::listen_with(|event, _status, window| match event {
            iced::Event::Window(e) => Some(MainViewMessage::WindowEvent(window, e)),
            iced::Event::Keyboard(e) => Some(MainViewMessage::KeyboardEvent(window, e)),
            iced::Event::Mouse(e) => Some(MainViewMessage::MouseEvent(window, e)),
            _ => None,
        });

        let dock = self
            .dock_manager
            .subscription(services)
            .map(MainViewMessage::Dock);
        let canvas_create = CanvasCreated::listen_to().map(MainViewMessage::CanvasCreated);
        let canvas_remove = CanvasRemoved::listen_to().map(MainViewMessage::CanvasRemoved);

        Subscription::batch([external, dock, canvas_create, canvas_remove])
    }

    fn windows(&self) -> Arc<[iced_core::window::Id]> {
        self.dock_manager
            .window_infos()
            .map(|i| i.id)
            .chain(self.dock_manager.sub_windows())
            .collect::<Vec<_>>()
            .into()
    }

    fn root_window(&self) -> Option<iced_core::window::Id> {
        Some(self.dock_manager.main_window().id)
    }
}
