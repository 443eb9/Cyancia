use std::{
    any::Any,
    collections::{HashMap, hash_map::Entry},
    rc::Rc,
    sync::Arc,
};

use cyancia_assets::AssetAppExt;
use cyancia_input::{
    key::{KeySequence, KeyboardState},
    mouse::{HoverMouseState, PressedMouseState},
};
use cyancia_runtime::{Application, Services, plugin::Plugin, service::Service};
use cyancia_utils::wrapper;
use iced::{Element, Task, keyboard::key};
use parse_display::Display;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wgpu::TextureView;

use crate::manifest::{ToolBinding, ToolBindingManifest, ToolBindingManifestSerializer};

pub mod manifest;

pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    fn build(&self, app: &mut Application) {
        app.add_service::<ToolFunctionRegistry>()
            .add_service::<ToolProxies>()
            .add_service::<GlobalToolBindings>();
        app.runtime_mut()
            .services_mut()
            .add_asset_serializer::<ToolBindingManifestSerializer>();
    }

    fn finish(&self, app: &mut Application) {
        let mut runtime = app.runtime_mut();
        let services = runtime.services_mut();
        let manifest = services
            .assets()
            .all_handles_of::<ToolBindingManifest>()
            .expect("Failed to read tool binding manifests")
            .into_iter()
            .next()
            .expect("At least one tool binding manifest should exist")
            .get()
            .expect("Failed to load tool binding manifest");

        let bindings = manifest
            .bindings
            .iter()
            .cloned()
            .map(|binding| {
                let shortcut = parse_shortcut(&binding.shortcut)
                    .unwrap_or_else(|| panic!("Invalid tool shortcut: {}", binding.shortcut));
                (shortcut, binding)
            })
            .collect();
        services.service_mut::<GlobalToolBindings>().bindings = bindings;
    }
}

pub trait ToolsAppExt {
    fn add_tool_function<T: ToolFunction + Default>(&mut self) -> &mut Self;
}

impl ToolsAppExt for Services {
    fn add_tool_function<T: ToolFunction + Default>(&mut self) -> &mut Self {
        self.service_mut::<ToolFunctionRegistry>().register::<T>();
        self
    }
}

wrapper! {
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize)]
    #[display("{0}")]
    pub ToolId : Arc<str>
}

pub trait ToolFunction: 'static {
    type Message: Send + Sync + 'static;

    fn id() -> ToolId;
    fn activate(&mut self, _: &mut Services) -> Task<Self::Message> {
        Task::none()
    }
    fn hover(
        &mut self,
        _: &KeyboardState,
        _: &HoverMouseState,
        _: &mut Services,
    ) -> Task<Self::Message> {
        Task::none()
    }
    fn begin(
        &mut self,
        _: &KeyboardState,
        _: &PressedMouseState,
        _: &mut Services,
    ) -> Task<Self::Message> {
        Task::none()
    }
    fn update(
        &mut self,
        _: &KeyboardState,
        _: &PressedMouseState,
        _: &mut Services,
    ) -> Task<Self::Message> {
        Task::none()
    }
    fn end(
        &mut self,
        _: &KeyboardState,
        _: &PressedMouseState,
        _: &mut Services,
    ) -> Task<Self::Message> {
        Task::none()
    }
    fn deactivate(&mut self, _: &mut Services) -> Task<Self::Message> {
        Task::none()
    }
    fn handle_message(&mut self, _: Self::Message, _: &mut Services) -> Task<Self::Message> {
        Task::none()
    }
    fn tool_option_widget<'a>(
        &'a self,
        _: &'a Services,
    ) -> Element<'a, Self::Message, iced_core::Theme, iced_wgpu::Renderer> {
        iced_widget::Space::new().into()
    }
    fn canvas_overlay(&mut self, _: &TextureView, _: &mut Services) -> Task<Self::Message> {
        Task::none()
    }
}

pub trait ErasedToolFunction: 'static {
    fn id(&self) -> ToolId;
    fn activate(&mut self, services: &mut Services) -> Task<ErasedToolFunctionMessage>;
    fn hover(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &HoverMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage>;
    fn begin(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage>;
    fn update(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage>;
    fn end(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage>;
    fn deactivate(&mut self, services: &mut Services) -> Task<ErasedToolFunctionMessage>;
    fn handle_message(
        &mut self,
        message: Box<dyn Any + Send + Sync>,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage>;
    fn tool_option_widget<'a>(
        &'a self,
        services: &'a Services,
    ) -> Element<'a, ErasedToolFunctionMessage, iced_core::Theme, iced_wgpu::Renderer>;
    fn canvas_overlay(
        &mut self,
        canvas_surface: &TextureView,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage>;
}

impl<T: ToolFunction> ErasedToolFunction for T {
    fn id(&self) -> ToolId {
        T::id()
    }

    fn activate(&mut self, services: &mut Services) -> Task<ErasedToolFunctionMessage> {
        erased_task(T::id(), self.activate(services))
    }

    fn hover(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &HoverMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        erased_task(T::id(), self.hover(keyboard, mouse, services))
    }

    fn begin(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        erased_task(T::id(), self.begin(keyboard, mouse, services))
    }

    fn update(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        erased_task(T::id(), self.update(keyboard, mouse, services))
    }

    fn end(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        erased_task(T::id(), self.end(keyboard, mouse, services))
    }

    fn deactivate(&mut self, services: &mut Services) -> Task<ErasedToolFunctionMessage> {
        erased_task(T::id(), self.deactivate(services))
    }

    fn handle_message(
        &mut self,
        message: Box<dyn Any + Send + Sync>,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        let message = message
            .downcast::<T::Message>()
            .expect("Invalid message type passed to tool function");
        erased_task(T::id(), self.handle_message(*message, services))
    }

    fn tool_option_widget<'a>(
        &'a self,
        services: &'a Services,
    ) -> Element<'a, ErasedToolFunctionMessage, iced_core::Theme, iced_wgpu::Renderer> {
        let id = T::id();
        self.tool_option_widget(services)
            .map(move |message| ErasedToolFunctionMessage {
                tool_id: id.clone(),
                message: Box::new(message),
            })
    }

    fn canvas_overlay(
        &mut self,
        canvas_surface: &TextureView,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        erased_task(T::id(), self.canvas_overlay(canvas_surface, services))
    }
}

fn erased_task<T: Send + Sync + 'static>(
    tool_id: ToolId,
    task: Task<T>,
) -> Task<ErasedToolFunctionMessage> {
    task.map(move |message| ErasedToolFunctionMessage {
        tool_id: tool_id.clone(),
        message: Box::new(message),
    })
}

pub struct ErasedToolFunctionMessage {
    pub tool_id: ToolId,
    pub message: Box<dyn Any + Send + Sync>,
}

#[derive(Default)]
pub struct ToolFunctionRegistry {
    spawners: HashMap<ToolId, Rc<dyn Fn() -> Box<dyn ErasedToolFunction>>>,
}

impl ToolFunctionRegistry {
    pub fn register<T: ToolFunction + Default>(&mut self) {
        self.spawners
            .insert(T::id(), Rc::new(|| Box::new(T::default())));
    }
}

impl Service for ToolFunctionRegistry {}

struct State {
    function: ToolId,
    is_updating: bool,
}

#[derive(Default)]
pub struct ToolProxy {
    current_state: Option<State>,
    override_state: Option<State>,
    tool_functions: HashMap<ToolId, Box<dyn ErasedToolFunction>>,
}

impl ToolProxy {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_tool(&mut self, tool: &ToolId, services: &Services) -> bool {
        if self.tool_functions.contains_key(tool) {
            return true;
        }
        let spawner = services
            .service::<ToolFunctionRegistry>()
            .spawners
            .get(tool)
            .cloned();
        if let Some(spawner) = spawner {
            self.tool_functions.insert(tool.clone(), spawner());
            true
        } else {
            log::error!("Unable to switch to tool {:?}: not found in registry", tool);
            false
        }
    }

    pub fn switch_tool(
        &mut self,
        tool: ToolId,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        if Some(&tool) == self.current_tool() {
            return Task::none();
        }

        log::info!("Switched tool: {}", tool);
        let deactivate = self
            .current_state
            .take()
            .map(|state| {
                self.tool_functions
                    .get_mut(&state.function)
                    .expect("Current tool function should exist")
                    .deactivate(services)
            })
            .unwrap_or_else(Task::none);

        if !self.ensure_tool(&tool, services) {
            return deactivate;
        }
        let activate = self
            .tool_functions
            .get_mut(&tool)
            .expect("Registered tool function should exist")
            .activate(services);
        self.current_state = Some(State {
            function: tool,
            is_updating: false,
        });
        deactivate.chain(activate)
    }

    pub fn switch_override_tool(
        &mut self,
        tool: Option<ToolId>,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        if tool.as_ref() == self.override_tool() {
            return Task::none();
        }

        let deactivate = self
            .override_state
            .as_ref()
            .or(self.current_state.as_ref())
            .map(|state| {
                self.tool_functions
                    .get_mut(&state.function)
                    .expect("Active tool function should exist")
                    .deactivate(services)
            })
            .unwrap_or_else(Task::none);

        if let Some(tool) = tool {
            if !self.ensure_tool(&tool, services) {
                return deactivate;
            }
            let activate = self
                .tool_functions
                .get_mut(&tool)
                .expect("Registered tool function should exist")
                .activate(services);
            self.override_state = Some(State {
                function: tool,
                is_updating: false,
            });
            deactivate.chain(activate)
        } else {
            self.override_state = None;
            let activate = self
                .current_state
                .as_ref()
                .map(|state| {
                    self.tool_functions
                        .get_mut(&state.function)
                        .expect("Current tool function should exist")
                        .activate(services)
                })
                .unwrap_or_else(Task::none);
            deactivate.chain(activate)
        }
    }

    pub fn mouse_pressed(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        let Some(state) = self.override_state.as_mut().or(self.current_state.as_mut()) else {
            return Task::none();
        };
        if state.is_updating {
            return Task::none();
        }
        state.is_updating = true;
        self.tool_functions
            .get_mut(&state.function)
            .expect("Active tool function should exist")
            .begin(keyboard, mouse, services)
    }

    pub fn mouse_moved(
        &mut self,
        keyboard: &KeyboardState,
        position: iced::Point,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        let Some(state) = self.override_state.as_ref().or(self.current_state.as_ref()) else {
            return Task::none();
        };
        let function = self
            .tool_functions
            .get_mut(&state.function)
            .expect("Active tool function should exist");
        if state.is_updating {
            function.update(keyboard, &PressedMouseState { position }, services)
        } else {
            function.hover(keyboard, &HoverMouseState { position }, services)
        }
    }

    pub fn mouse_released(
        &mut self,
        keyboard: &KeyboardState,
        mouse: &PressedMouseState,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        let Some(state) = self.override_state.as_mut().or(self.current_state.as_mut()) else {
            return Task::none();
        };
        if !state.is_updating {
            return Task::none();
        }
        state.is_updating = false;
        self.tool_functions
            .get_mut(&state.function)
            .expect("Active tool function should exist")
            .end(keyboard, mouse, services)
    }

    pub fn handle_message(
        &mut self,
        message: ErasedToolFunctionMessage,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        let Some(function) = self.tool_functions.get_mut(&message.tool_id) else {
            return Task::none();
        };
        function.handle_message(message.message, services)
    }

    pub fn tool_option_widget<'a>(
        &'a self,
        services: &'a Services,
    ) -> Option<Element<'a, ErasedToolFunctionMessage, iced_core::Theme, iced_wgpu::Renderer>> {
        let state = self
            .override_state
            .as_ref()
            .or(self.current_state.as_ref())?;
        Some(
            self.tool_functions
                .get(&state.function)
                .expect("Active tool function should exist")
                .tool_option_widget(services),
        )
    }

    pub fn canvas_overlay(
        &mut self,
        canvas_surface: &TextureView,
        services: &mut Services,
    ) -> Task<ErasedToolFunctionMessage> {
        let Some(state) = self.override_state.as_ref().or(self.current_state.as_ref()) else {
            return Task::none();
        };
        self.tool_functions
            .get_mut(&state.function)
            .expect("Active tool function should exist")
            .canvas_overlay(canvas_surface, services)
    }

    pub fn current_tool(&self) -> Option<&ToolId> {
        Some(&self.current_state.as_ref()?.function)
    }

    pub fn override_tool(&self) -> Option<&ToolId> {
        Some(&self.override_state.as_ref()?.function)
    }
}

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub ToolProxyId : Uuid
}

#[derive(Default)]
pub struct ToolProxies {
    proxies: HashMap<ToolProxyId, ToolProxy>,
}

impl Service for ToolProxies {}

impl ToolProxies {
    pub fn get(&self, id: &ToolProxyId) -> &ToolProxy {
        self.proxies.get(id).expect("Tool proxy should exist")
    }

    pub fn get_mut(&mut self, id: &ToolProxyId) -> &mut ToolProxy {
        self.proxies.get_mut(id).expect("Tool proxy should exist")
    }

    pub fn add(&mut self, tool_proxy: ToolProxy) -> ToolProxyId {
        let id = ToolProxyId::new(Uuid::new_v4());
        self.proxies.insert(id, tool_proxy);
        id
    }

    pub fn remove(&mut self, id: &ToolProxyId) -> Option<ToolProxy> {
        self.proxies.remove(id)
    }
}

#[derive(Default)]
pub struct GlobalToolBindings {
    bindings: Vec<(KeySequence, ToolBinding)>,
}

impl Service for GlobalToolBindings {}

impl GlobalToolBindings {
    pub fn binding_for(&self, shortcut: KeySequence) -> Option<&ToolBinding> {
        self.bindings
            .iter()
            .find_map(|(key, binding)| (*key == shortcut).then_some(binding))
    }
}

fn parse_shortcut(shortcut: &str) -> Option<KeySequence> {
    let mut codes = Vec::new();
    for part in shortcut.split('-') {
        let code = match part.to_ascii_lowercase().as_str() {
            "ctrl" => key::Code::ControlLeft,
            "alt" => key::Code::AltLeft,
            "shift" => key::Code::ShiftLeft,
            "super" => key::Code::SuperLeft,
            "space" => key::Code::Space,
            "a" => key::Code::KeyA,
            "b" => key::Code::KeyB,
            "c" => key::Code::KeyC,
            "d" => key::Code::KeyD,
            "e" => key::Code::KeyE,
            "f" => key::Code::KeyF,
            "g" => key::Code::KeyG,
            "h" => key::Code::KeyH,
            "i" => key::Code::KeyI,
            "j" => key::Code::KeyJ,
            "k" => key::Code::KeyK,
            "l" => key::Code::KeyL,
            "m" => key::Code::KeyM,
            "n" => key::Code::KeyN,
            "o" => key::Code::KeyO,
            "p" => key::Code::KeyP,
            "q" => key::Code::KeyQ,
            "r" => key::Code::KeyR,
            "s" => key::Code::KeyS,
            "t" => key::Code::KeyT,
            "u" => key::Code::KeyU,
            "v" => key::Code::KeyV,
            "w" => key::Code::KeyW,
            "x" => key::Code::KeyX,
            "y" => key::Code::KeyY,
            "z" => key::Code::KeyZ,
            _ => return None,
        };
        codes.push(code);
    }
    KeySequence::from_codes(codes.into_iter()).ok()
}
