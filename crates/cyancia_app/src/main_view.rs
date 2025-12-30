use std::sync::Arc;

use cyancia_assets::store::{AssetLoaderRegistry, AssetRegistry};
use cyancia_canvas::widget::CanvasWidget;
use cyancia_input::action::{
    ActionCollection, ActionManifest,
    matching::{ActionChange, ActionMatcher},
};
use iced::{
    Element, Renderer, Subscription, Task, Theme,
    keyboard::{self, key},
};

pub struct MainView {
    pub assets: AssetRegistry,
    pub action_matcher: ActionMatcher,
}

#[derive(Debug)]
pub enum MainViewMessage {
    KeyPressed(key::Code),
    KeyReleased(key::Code),
}

impl MainView {
    pub fn new() -> Self {
        let mut loaders = AssetLoaderRegistry::new();
        cyancia_input::register_loaders(&mut loaders);
        let assets = AssetRegistry::new("assets", &loaders);

        Self {
            action_matcher: ActionMatcher::new(ActionCollection::new(
                assets.store::<ActionManifest>().clone(),
            )),
            assets,
        }
    }

    pub fn view(&self) -> Element<'_, MainViewMessage, Theme, Renderer> {
        CanvasWidget {}.into()
    }

    pub fn update(&mut self, message: MainViewMessage) -> Task<MainViewMessage> {
        match message {
            MainViewMessage::KeyPressed(key) => {
                let change = self.action_matcher.key_pressed(key);
                self.handle_action_change(change);
            }
            MainViewMessage::KeyReleased(key) => {
                let change = self.action_matcher.key_released(key);
                self.handle_action_change(change);
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<MainViewMessage> {
        keyboard::listen().filter_map(|event| match event {
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
        })
    }

    fn handle_action_change(&mut self, change: ActionChange) {}
}
