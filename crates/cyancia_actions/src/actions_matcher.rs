use cyancia_input::key::KeyboardState;
use iced_core::keyboard::{self, key};

use crate::{ActionId, manifest::ActionCollection};

pub struct ActionsMatcher {
    actions: ActionCollection,
    pub keyboard_state: KeyboardState,
}

impl ActionsMatcher {
    pub fn new(actions: ActionCollection) -> Self {
        Self {
            actions,
            keyboard_state: KeyboardState::default(),
        }
    }

    pub fn key_pressed(&mut self, code: key::Code) -> Option<ActionId> {
        self.keyboard_state.press(code);
        let seq = self.keyboard_state.get_sequence();
        self.actions.get_action_id(seq)
    }

    pub fn key_released(&mut self, code: key::Code) {
        self.keyboard_state.release(code);
    }

    pub fn reset_keyboard_state(&mut self) {
        self.keyboard_state = Default::default();
    }
}
