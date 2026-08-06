use std::{cell::RefCell, sync::Arc, time::Duration};

use bevy_math::{IRect, Rect};
use cyancia_assets::AssetAppExt;
use cyancia_brush::{asset::BrushPreset, tool::BrushServicesExt, widget::BrushPresetListDelegate};
use cyancia_canvas::{
    CanvasAppExt, CanvasId, CanvasManager,
    event::{CanvasRemoved, CanvasUpdated},
    render::ICC_TRANSFORM_SHADER_IDENT,
    tools::PanTool,
    widget::canvas::CanvasWidget,
};
use cyancia_color::{platform::get_window_color_profile, shader::IccTransformShader};
use cyancia_dock::dock::{Dock, DockId};
use cyancia_image::{
    composite::{BlendFunctionRegistry, ImageCompositor, LayerPreviewOverriders},
    tile::{GpuTileStorage, TileStorageAppExt},
};
use cyancia_input::{
    key::KeyboardState,
    mouse::{HoverMouseState, PressedMouseState},
};
use cyancia_render::render_context::RenderContextAppExt;
use cyancia_runtime::{Services, event::Event, service::RenderContext};
use cyancia_tools::{ErasedToolFunctionMessage, ToolProxies};
use iced::{
    Element, Length, Subscription, Task, Theme,
    event::listen_with,
    mouse,
    widget::{Space, button, column, text},
    window,
};
use iced_core::Point;
use iced_runtime::task;
use iced_wgpu::Renderer;
use moxcms::Layout;
use parking_lot::Mutex;

// pub struct CurrentCanvasLayersDock {
//     widget: Option<Entity<LayerStackWidget>>,
//     focus_handle: FocusHandle,
// }
//
// impl CurrentCanvasLayersDock {
//     pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
//         let focus_handle = cx.focus_handle();
//         cx.subscribe_in(
//             &cx.global_canvas_events_entity(),
//             window,
//             |dock, _, _: &CurrentCanvasChanged, window, cx| {
//                 if let Some(canvas) = cx.current_canvas().and_then(|e| e.upgrade()) {
//                     dock.widget = Some(cx.new(|cx| LayerStackWidget::new(canvas, window, cx)));
//                 }
//             },
//         )
//         .detach();
//
//         Self {
//             widget: None,
//             focus_handle,
//         }
//     }
// }
//
// impl EventEmitter<PanelEvent> for CurrentCanvasLayersDock {}
//
// impl Focusable for CurrentCanvasLayersDock {
//     fn focus_handle(&self, _: &App) -> FocusHandle {
//         self.focus_handle.clone()
//     }
// }
//
// impl Panel for CurrentCanvasLayersDock {
//     fn panel_name(&self) -> &'static str {
//         "current_canvas_layers"
//     }
//
//     fn tab_name(&self, _: &App) -> Option<SharedString> {
//         Some("Current Canvas Layers".into())
//     }
// }
//
// impl Render for CurrentCanvasLayersDock {
//     fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
//         self.widget
//             .as_ref()
//             .map(|w| w.clone().into_any_element())
//             .unwrap_or_else(|| div().into_any_element())
//     }
// }

// pub struct ColorSelectorDock {
//     focus_handle: FocusHandle,
//     color_selector: Entity<ColorSelectorState>,
//     config_editor: Option<Entity<ColorSelectorConfigEditorState>>,
//     editor_window: Option<AnyWindowHandle>,
//     _subscriptions: Vec<Subscription>,
// }
//
// impl ColorSelectorDock {
//     pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
//         let config = ColorSelectorConfig {
//             name: "RGB".to_string(),
//             max_plane_size: 512,
//             max_planes_per_row: 2,
//             planes: vec![
//                 GradientPlaneConfig {
//                     model: ColorModel::Rgb,
//                     shape: GradientPlaneShape::Square,
//                     variable_channels: 0b110,
//                     flip_axis: GradientPlaneFlipAxis::empty(),
//                     rotation: 0.0,
//                     show_primary_channel_ring: false,
//                     primary_channel_ring_width: 20.0,
//                     ring_bar_saturated_hue_channel: false,
//                     ring_rotation: 0.0,
//                     reversed_ring: false,
//                 },
//                 GradientPlaneConfig {
//                     model: ColorModel::Hsv,
//                     shape: GradientPlaneShape::Triangle,
//                     variable_channels: 0b110,
//                     flip_axis: GradientPlaneFlipAxis::empty(),
//                     rotation: 0.0,
//                     show_primary_channel_ring: true,
//                     primary_channel_ring_width: 20.0,
//                     ring_bar_saturated_hue_channel: true,
//                     ring_rotation: std::f32::consts::FRAC_PI_2,
//                     reversed_ring: false,
//                 },
//             ],
//             bars: vec![
//                 GradientBarConfig {
//                     model: ColorModel::Rgb,
//                     channel: 0,
//                     bar_height: 20.0,
//                     show_channel_label: true,
//                     show_precise_spin_box: true,
//                     show_primary_channel_lock: true,
//                 },
//                 GradientBarConfig {
//                     model: ColorModel::Rgb,
//                     channel: 1,
//                     bar_height: 20.0,
//                     show_channel_label: true,
//                     show_precise_spin_box: false,
//                     show_primary_channel_lock: true,
//                 },
//                 GradientBarConfig {
//                     model: ColorModel::Rgb,
//                     channel: 2,
//                     bar_height: 20.0,
//                     show_channel_label: false,
//                     show_precise_spin_box: true,
//                     show_primary_channel_lock: true,
//                 },
//                 GradientBarConfig {
//                     model: ColorModel::Hsv,
//                     channel: 0,
//                     bar_height: 20.0,
//                     show_channel_label: true,
//                     show_precise_spin_box: true,
//                     show_primary_channel_lock: false,
//                 },
//             ],
//             out_of_gamut_color: Rgb::new(0.5, 0.5, 0.5),
//             use_out_of_gamut_color: true,
//             clip_to_gamut: false,
//         };
//
//         let color_selector = cx.new(|cx| {
//             ColorSelectorState::new(
//                 Color::Rgb(Rgb::new(0.0, 0.0, 0.0)),
//                 ColorProfile::new_srgb(),
//                 vec![config.clone()],
//                 0,
//                 window,
//                 cx,
//             )
//         });
//
//         let dock = cx.entity().downgrade();
//         let subscriptions = vec![cx.on_window_closed(move |cx, window_id| {
//             let dock = dock.clone();
//             cx.defer(move |cx| {
//                 dock.update(cx, |dock, cx| {
//                     if dock
//                         .editor_window
//                         .is_some_and(|window| window.window_id() == window_id)
//                     {
//                         dock.editor_window = None;
//                         cx.notify();
//                     }
//                 })
//                 .ok();
//             });
//         })];
//
//         Self {
//             focus_handle: cx.focus_handle(),
//             color_selector,
//             config_editor: None,
//             editor_window: None,
//             _subscriptions: subscriptions,
//         }
//     }
//
//     fn on_config_editor_event(
//         &mut self,
//         editor: &Entity<ColorSelectorConfigEditorState>,
//         event: &ColorSelectorConfigEvent,
//         window: &mut Window,
//         cx: &mut Context<Self>,
//     ) {
//         match event {
//             ColorSelectorConfigEvent::Confirm => {
//                 let configs = editor.read(cx).configs().to_vec();
//                 self.color_selector.update(cx, |selector, cx| {
//                     selector.set_configs(configs, window, cx);
//                     cx.notify();
//                 });
//                 cx.refresh_windows();
//             }
//             ColorSelectorConfigEvent::Cancel => {
//                 if let Some(editor_window) = self.editor_window.take() {
//                     editor_window
//                         .update(cx, |_, window, _| window.remove_window())
//                         .ok();
//                 }
//             }
//         }
//     }
//
//     fn on_open_editor(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
//         if self.editor_window.is_some() {
//             return;
//         }
//
//         let (configs, selected_config) = self.color_selector.read_with(cx, |selector, _| {
//             (selector.configs().to_vec(), selector.selected_config())
//         });
//         let config_editor =
//             cx.new(|cx| ColorSelectorConfigEditorState::new(configs, selected_config, window, cx));
//         cx.subscribe_in(&config_editor, window, Self::on_config_editor_event)
//             .detach();
//
//         let parent_center = window.bounds().center();
//         let size = Size::new(px(500.0), px(1080.0));
//
//         let editor_window = cx.open_window(
//             WindowOptions {
//                 window_bounds: Some(WindowBounds::Windowed(Bounds::new(
//                     parent_center - Point::new(size.width, size.height) * 0.5,
//                     size,
//                 ))),
//                 ..Default::default()
//             },
//             |window, cx| cx.new(|cx| Root::new(config_editor.clone(), window, cx)),
//         );
//
//         self.config_editor = Some(config_editor);
//
//         let Ok(editor_window) = editor_window.logged_err() else {
//             return;
//         };
//
//         self.editor_window = Some(editor_window.into());
//     }
// }
//
// impl EventEmitter<PanelEvent> for ColorSelectorDock {}
//
// impl Focusable for ColorSelectorDock {
//     fn focus_handle(&self, _cx: &App) -> FocusHandle {
//         self.focus_handle.clone()
//     }
// }
//
// impl Panel for ColorSelectorDock {
//     fn panel_name(&self) -> &'static str {
//         "color_selector"
//     }
//
//     fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
//         "Color Selector"
//     }
// }
//
// impl Render for ColorSelectorDock {
//     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//         div()
//             .size_full()
//             .min_w_0()
//             .min_h_0()
//             .overflow_hidden()
//             .child(
//                 v_flex()
//                     .size_full()
//                     .overflow_y_scrollbar()
//                     .child(self.color_selector.clone())
//                     .child(
//                         div().flex_shrink_0().child(
//                             Button::new("open-editor")
//                                 .icon(IconName::Settings)
//                                 .on_click(cx.listener(Self::on_open_editor)),
//                         ),
//                     ),
//             )
//     }
// }

macro_rules! test_dummy_dock {
    ($name:ident, $id:ident, $text:expr) => {
        pub struct $name;

        impl Dock<Theme, Renderer> for $name {
            type Message = ();

            fn id(&self) -> DockId {
                DockId::new($text.into())
            }

            fn view<'a>(
                &'a self,
                window_id: window::Id,
                services: &'a Services,
            ) -> Element<'a, Self::Message, Theme, Renderer> {
                text($text).into()
            }

            fn update(&mut self, _message: (), services: &mut Services) -> Task<()> {
                Task::none()
            }
        }

        pub const $id: &'static str = $text;
    };
}

test_dummy_dock!(LayersDock, LAYER_DOCK_ID, "Layers");
test_dummy_dock!(FiltersDock, FILTERS_DOCK_ID, "Filters");

pub const TOOL_OPTIONS_DOCK_ID: &'static str = "tool_options";
pub const BRUSH_PRESETS_DOCK_ID: &'static str = "brush_presets";

pub fn construct_canvas_dock_id(canvas: CanvasId) -> String {
    format!("canvas_dock_{}", canvas)
}

pub struct CanvasDock {
    canvas: CanvasId,

    compositor: ImageCompositor,
    cursor_position: Point,

    window_id: RefCell<window::Id>,
    raw_window_id: Option<u64>,
    monitor_name: Option<String>,
}

impl CanvasDock {
    pub fn new(canvas: CanvasId, window_id: window::Id) -> Self {
        Self {
            canvas,
            compositor: ImageCompositor::default(),
            cursor_position: Point::default(),
            window_id: RefCell::new(window_id),
            raw_window_id: None,
            monitor_name: None,
        }
    }
}

pub enum CanvasDockMessage {
    WindowMoved,
    CanvasUpdated(Option<IRect>),
    CanvasFocus(Point),
    MouseEvent(mouse::Event),
    WidgetRectChange(Rect),
    ToolFunctionMessage(ErasedToolFunctionMessage),
    RawWindowIdUpdate(u64),
    MonitorNameUpdate(Option<String>),
}

impl Dock<Theme, Renderer> for CanvasDock {
    type Message = CanvasDockMessage;

    fn id(&self) -> DockId {
        DockId::new(construct_canvas_dock_id(self.canvas).into())
    }

    fn view<'a>(
        &'a self,
        window_id: window::Id,
        services: &'a Services,
    ) -> Element<'a, Self::Message, Theme, Renderer> {
        let canvas_manager = services.service::<CanvasManager>();
        self.window_id.replace(window_id);

        let (Some(canvas), Some(window_id), Some(monitor_name)) =
            (canvas_manager.get(&self.canvas), self.raw_window_id, self.monitor_name.clone())
        else {
            return Space::new().into();
        };

        CanvasWidget {
            is_focusing: canvas_manager.current_id() == Some(self.canvas),
            canvas,
            tile_storage: services.service::<GpuTileStorage>().clone(),
            on_focus: Box::new(CanvasDockMessage::CanvasFocus),
            on_mouse_event: Box::new(CanvasDockMessage::MouseEvent),
            on_widget_rect_change: Box::new(CanvasDockMessage::WidgetRectChange),
            // TODO wrap in arc?
            color_profile: canvas.image.profile().clone(),
            window_id,
            monitor_name,
        }
        .into()
    }

    fn update(&mut self, message: Self::Message, services: &mut Services) -> Task<Self::Message> {
        match message {
            CanvasDockMessage::CanvasUpdated(dirty_tiles) => {
                services.service_scope::<LayerPreviewOverriders, _>(|overriders, services| {
                    let Some(canvas) = services.canvas(&self.canvas) else {
                        return;
                    };
                    let tiles = services.tile_storage();
                    let blend_functions = services.service::<BlendFunctionRegistry>();
                    let device = services.render_device();
                    let queue = services.render_queue();
                    self.compositor.create_cache(
                        overriders,
                        &canvas.image,
                        tiles,
                        blend_functions,
                        device,
                        queue,
                    );
                    self.compositor.composite(
                        overriders,
                        dirty_tiles.unwrap_or_else(|| canvas.image.image_tile_rect()),
                        &canvas.image,
                        tiles,
                        device,
                        queue,
                    );
                });

                Task::none()
            }
            CanvasDockMessage::MouseEvent(event) => {
                let canvas_manager = services.service_mut::<CanvasManager>();
                if canvas_manager.current_id() != Some(self.canvas) {
                    return Task::none();
                }

                let Some(canvas) = canvas_manager.current() else {
                    return Task::none();
                };
                let tool_proxy_id = canvas.tool_proxy_id();

                let task = services.service_scope::<ToolProxies, _>(|tool_proxies, services| {
                    let tool_proxy = tool_proxies.get_mut(&tool_proxy_id);
                    let keyboard_state = services.service::<KeyboardState>().clone();

                    match event {
                        mouse::Event::ButtonPressed(button) => {
                            if button != mouse::Button::Left {
                                return Task::none();
                            }

                            tool_proxy.mouse_pressed(
                                &keyboard_state,
                                &PressedMouseState {
                                    position: self.cursor_position,
                                },
                                services,
                            )
                        }
                        mouse::Event::ButtonReleased(button) => {
                            if button != mouse::Button::Left {
                                return Task::none();
                            }

                            tool_proxy.mouse_released(
                                &keyboard_state,
                                &PressedMouseState {
                                    position: self.cursor_position,
                                },
                                services,
                            )
                        }
                        mouse::Event::CursorMoved { position } => {
                            self.cursor_position = position;
                            tool_proxy.mouse_moved(&keyboard_state, position, services)
                        }
                        _ => Task::none(),
                    }
                });

                task.map(CanvasDockMessage::ToolFunctionMessage)
            }
            CanvasDockMessage::CanvasFocus(cursor_pos) => {
                self.cursor_position = cursor_pos;
                services
                    .service_mut::<CanvasManager>()
                    .set_current(self.canvas);
                Task::none()
            }
            CanvasDockMessage::WidgetRectChange(rect) => {
                let canvas_manager = services.service_mut::<CanvasManager>();
                if let Some(canvas) = canvas_manager.get_mut(&self.canvas) {
                    canvas.transform.widget_bounds = rect;
                }
                Task::none()
            }
            CanvasDockMessage::ToolFunctionMessage(message) => {
                let Some(canvas) = services.service::<CanvasManager>().get(&self.canvas) else {
                    return Task::none();
                };

                let tool_proxy_id = canvas.tool_proxy_id();
                services
                    .service_scope::<ToolProxies, _>(|tool_proxies, services| {
                        tool_proxies
                            .get_mut(&tool_proxy_id)
                            .handle_message(message, services)
                    })
                    .map(CanvasDockMessage::ToolFunctionMessage)
            }
            CanvasDockMessage::WindowMoved => {
                let window_id = self.window_id.borrow().clone();

                let monitor_name = task::oneshot(move |channel| {
                    iced_runtime::Action::Window(window::Action::GetMonitorName(window_id, channel))
                })
                .map(CanvasDockMessage::MonitorNameUpdate);

                let window_raw_id =
                    window::raw_id::<()>(window_id).map(CanvasDockMessage::RawWindowIdUpdate);

                Task::batch([monitor_name, window_raw_id])
            }
            CanvasDockMessage::RawWindowIdUpdate(id) => {
                self.raw_window_id = Some(id);
                Task::none()
            }
            CanvasDockMessage::MonitorNameUpdate(name) => {
                self.monitor_name = name;
                Task::none()
            }
        }
    }

    fn on_open(&mut self) -> Task<Self::Message> {
        Task::batch([
            Task::done(CanvasDockMessage::WindowMoved),
            Task::done(CanvasDockMessage::CanvasUpdated(None)),
        ])
    }

    fn on_close(&mut self) -> Task<Self::Message> {
        CanvasRemoved::broadcast(CanvasRemoved { id: self.canvas });

        Task::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let cur_window = self.window_id.borrow().clone();

        let canvas_update = CanvasUpdated::listen_to()
            .map(|e| CanvasDockMessage::CanvasUpdated(Some(e.dirty_tiles)));
        let window_moved =
            window::events()
                .with(cur_window)
                .filter_map(|(cur_window, (window_id, event))| {
                    if matches!(event, window::Event::Moved(_)) && cur_window == window_id {
                        Some(CanvasDockMessage::WindowMoved)
                    } else {
                        None
                    }
                });

        Subscription::batch([canvas_update, window_moved])
    }
}

pub struct ToolOptionsDock;

pub enum ToolOptionsDockMessage {
    ToolFunction(ErasedToolFunctionMessage),
}

impl ToolOptionsDock {
    pub fn new(_: &Services) -> Self {
        Self
    }
}

impl Dock<Theme, iced_wgpu::Renderer> for ToolOptionsDock {
    type Message = ToolOptionsDockMessage;

    fn id(&self) -> DockId {
        DockId::new(TOOL_OPTIONS_DOCK_ID.into())
    }

    fn view<'a>(
        &'a self,
        window_id: window::Id,
        services: &'a Services,
    ) -> Element<'a, Self::Message, Theme, iced_wgpu::Renderer> {
        let Some(canvas) = services.service::<CanvasManager>().current() else {
            return Space::new().into();
        };

        let tool_proxy_id = canvas.tool_proxy_id();
        let tool_proxy = services.service::<ToolProxies>().get(&tool_proxy_id);
        let indicator = text(format!(
            "Tool: {} | override: {}",
            tool_proxy
                .current_tool()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".into()),
            tool_proxy
                .override_tool()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".into()),
        ));

        let Some(widget) = tool_proxy.tool_option_widget(services) else {
            return column![indicator].into();
        };

        column![indicator, widget.map(ToolOptionsDockMessage::ToolFunction)]
            .spacing(4)
            .into()
    }

    fn update(&mut self, message: Self::Message, services: &mut Services) -> Task<Self::Message> {
        match message {
            ToolOptionsDockMessage::ToolFunction(message) => {
                let Some(canvas) = services.service::<CanvasManager>().current() else {
                    return Task::none();
                };

                let tool_proxy_id = canvas.tool_proxy_id();
                services
                    .service_scope::<ToolProxies, _>(|tool_proxies, services| {
                        tool_proxies
                            .get_mut(&tool_proxy_id)
                            .handle_message(message, services)
                    })
                    .map(ToolOptionsDockMessage::ToolFunction)
            }
        }
    }
}

pub struct BrushPresetDock {
    brushes: BrushPresetListDelegate,
}

#[derive(Clone)]
pub enum BrushPresetDockMessage {
    SelectBrush(usize),
}

impl BrushPresetDock {
    pub fn new(services: &Services) -> Self {
        Self {
            brushes: BrushPresetListDelegate::new(
                services.assets().all_handles_of::<BrushPreset>().unwrap(),
            ),
        }
    }
}

impl Dock<Theme, Renderer> for BrushPresetDock {
    type Message = BrushPresetDockMessage;

    fn id(&self) -> DockId {
        DockId::new(BRUSH_PRESETS_DOCK_ID.into())
    }

    fn view<'a>(
        &'a self,
        window_id: window::Id,
        services: &'a Services,
    ) -> Element<'a, Self::Message, Theme, Renderer> {
        let buttons = self
            .brushes
            .items()
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut brush_button = button(text(item.name.clone()))
                    .width(Length::Fill)
                    .on_press(BrushPresetDockMessage::SelectBrush(index));
                if item.selected {
                    brush_button = brush_button.style(move |theme: &Theme, _| {
                        let palette = theme.extended_palette();
                        button::Style {
                            background: Some(palette.primary.strong.color.into()),
                            text_color: palette.primary.strong.text,
                            ..Default::default()
                        }
                    });
                }
                brush_button.into()
            })
            .collect::<Vec<Element<'a, _, Theme, Renderer>>>();

        column(buttons).spacing(2).into()
    }

    fn update(&mut self, message: Self::Message, services: &mut Services) -> Task<Self::Message> {
        match message {
            BrushPresetDockMessage::SelectBrush(index) => {
                self.brushes.select(index);
                let handle = self.brushes.get(index).map(|item| item.brush.clone());
                if let Some(handle) = handle {
                    services.set_current_brush_preset(handle);
                }
                Task::none()
            }
        }
    }
}
