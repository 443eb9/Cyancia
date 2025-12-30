use std::sync::Arc;

use cyancia_assets::store::{AssetLoaderRegistry, AssetRegistry};
use cyancia_canvas::{
    CCanvas,
    action::{
        CanvasActionCollection,
        control::{CanvasPanAction, CanvasRotateAction, CanvasZoomAction},
    },
    widget::CanvasWidget,
};
use cyancia_id::Id;
use cyancia_input::{
    action::{Action, ActionCollection, ActionManifest, ActionType, matching::ActionMatcher},
    key::KeySequence,
    mouse::MouseState,
};
use cyancia_tools::{brush::BrushTool, canvas_pan::CanvasPanTool};
use iced::{
    Element, Point, Renderer, Subscription, Task, Theme, event,
    keyboard::{self, key},
    mouse,
};

pub struct MainView {
    pub assets: AssetRegistry,
    pub action_matcher: ActionMatcher,
    pub canvas_actions: CanvasActionCollection,
    pub canvas: CCanvas,
    pub mouse_state: MouseState,
    pub current_action: Option<(Id<Action>, Arc<Action>, KeySequence)>,
}

#[derive(Debug)]
pub enum MainViewMessage {
    KeyPressed(key::Code),
    KeyReleased(key::Code),
    MousePressed(mouse::Button),
    MouseMoved(Point),
    MouseReleased(mouse::Button),
}

impl MainView {
    pub fn new() -> Self {
        let mut loaders = AssetLoaderRegistry::new();
        cyancia_input::register_loaders(&mut loaders);
        let assets = AssetRegistry::new("assets", &loaders);

        let mut canvas_actions = CanvasActionCollection::new();
        canvas_actions.register::<CanvasPanAction>();
        canvas_actions.register::<CanvasRotateAction>();
        canvas_actions.register::<CanvasZoomAction>();
        canvas_actions.register::<CanvasPanTool>();
        canvas_actions.register::<BrushTool>();

        Self {
            action_matcher: ActionMatcher::new(ActionCollection::new(
                assets.store::<ActionManifest>().clone(),
            )),
            canvas_actions,
            assets,
            canvas: CCanvas {},
            mouse_state: MouseState::new(),
            current_action: None,
        }
    }

    pub fn view(&self) -> Element<'_, MainViewMessage, Theme, Renderer> {
        CanvasWidget {}.into()
    }

    pub fn update(&mut self, message: MainViewMessage) -> Task<MainViewMessage> {
        match message {
            MainViewMessage::KeyPressed(key) => {
                let previous = self.action_matcher.key_pressed(key);
                self.handle_action_change(previous);
            }
            MainViewMessage::KeyReleased(key) => {
                let previous = self.action_matcher.key_released(key);
                self.handle_action_change(previous);
            }
            MainViewMessage::MousePressed(button) => {
                self.mouse_state.press(button);
                self.try_begin_current_action();
            }
            MainViewMessage::MouseMoved(position) => {
                self.mouse_state.move_to(position);
                self.try_update_current_action();
            }
            MainViewMessage::MouseReleased(button) => {
                self.mouse_state.release(button);
                self.try_end_current_action();
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<MainViewMessage> {
        event::listen().filter_map(|event| match event {
            iced::Event::Keyboard(event) => match event {
                keyboard::Event::KeyPressed {
                    physical_key,
                    repeat,
                    ..
                } => {
                    if repeat {
                        return None;
                    }

                    let key::Physical::Code(key) = physical_key else {
                        log::warn!("Unknown key pressed: {:?}", physical_key);
                        return None;
                    };

                    Some(MainViewMessage::KeyPressed(key))
                }
                keyboard::Event::KeyReleased { physical_key, .. } => {
                    let key::Physical::Code(key) = physical_key else {
                        log::warn!("Unknown key released: {:?}", physical_key);
                        return None;
                    };

                    Some(MainViewMessage::KeyReleased(key))
                }
                _ => None,
            },
            iced::Event::Mouse(event) => match event {
                mouse::Event::CursorMoved { position } => {
                    Some(MainViewMessage::MouseMoved(position))
                }
                mouse::Event::ButtonPressed(button) => Some(MainViewMessage::MousePressed(button)),
                mouse::Event::ButtonReleased(button) => {
                    Some(MainViewMessage::MouseReleased(button))
                }
                _ => None,
            },
            _ => None,
        })
    }

    fn handle_action_change(&mut self, previous: Option<(Id<Action>, Arc<Action>, KeySequence)>) {
        if let Some((id, _, keys)) = previous
            && !self.mouse_state.is_pressed(mouse::Button::Left)
            && let Some(canvas_action) = self.canvas_actions.get(&id)
        {
            canvas_action.deactivate(keys, &mut self.canvas);
            self.current_action = self.action_matcher.current_action();
        }

        if let Some((id, action, keys)) = self.action_matcher.current_action()
            && !self.mouse_state.is_pressed(mouse::Button::Left)
            && let Some(canvas_action) = self.canvas_actions.get(&id)
        {
            if self
                .current_action
                .as_ref()
                .is_some_and(|(old, _, _)| *old == id)
                && action.ty == ActionType::Toggle
            {
                canvas_action.deactivate(keys, &mut self.canvas);
                self.current_action = None;
            } else {
                canvas_action.prepare(keys, &mut self.canvas);
                self.current_action = self.action_matcher.current_action();
            }
        }
    }

    fn try_begin_current_action(&mut self) {
        if let Some((id, action, keys)) = &self.current_action
            && action.ty != ActionType::OneShot
            && let Some(canvas_action) = self.canvas_actions.get(&id)
        {
            canvas_action.begin(*keys, self.mouse_state.position(), &mut self.canvas);
        }
    }

    fn try_update_current_action(&mut self) {
        if self.mouse_state.is_pressed(mouse::Button::Left)
            && let Some((id, action, keys)) = &self.current_action
            && action.ty != ActionType::OneShot
            && let Some(canvas_action) = self.canvas_actions.get(&id)
        {
            canvas_action.update(*keys, self.mouse_state.position(), &mut self.canvas);
        }
    }

    fn try_end_current_action(&mut self) {
        if let Some((id, action, keys)) = &self.current_action
            && let Some(canvas_action) = self.canvas_actions.get(&id)
        {
            canvas_action.end(*keys, self.mouse_state.position(), &mut self.canvas);

            let action_changed = self
                .action_matcher
                .current_action()
                .as_ref()
                .is_none_or(|(new, _, _)| new != id);
            if action.ty == ActionType::Hold && action_changed {
                canvas_action.deactivate(*keys, &mut self.canvas);
                self.current_action = self.action_matcher.current_action();
            }
        }
    }
}
