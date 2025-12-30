use std::collections::HashSet;

use iced_core::{Point, mouse};
use smallvec::SmallVec;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MouseButtons: u32 {
        const NONE = 0;
        const LEFT = 1 << 0;
        const RIGHT = 1 << 1;
        const MIDDLE = 1 << 2;
    }
}

impl MouseButtons {
    pub fn from_iced(button: mouse::Button) -> Self {
        match button {
            mouse::Button::Left => MouseButtons::LEFT,
            mouse::Button::Right => MouseButtons::RIGHT,
            mouse::Button::Middle => MouseButtons::MIDDLE,
            _ => MouseButtons::NONE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MouseState {
    pressed: MouseButtons,
    position: Point,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            pressed: MouseButtons::NONE,
            position: Point::ORIGIN,
        }
    }

    pub fn press(&mut self, button: mouse::Button) {
        self.pressed.insert(MouseButtons::from_iced(button));
    }

    pub fn release(&mut self, button: mouse::Button) {
        self.pressed.remove(MouseButtons::from_iced(button));
    }

    pub fn move_to(&mut self, position: Point) {
        self.position = position;
    }

    pub fn is_pressed(&self, button: mouse::Button) -> bool {
        self.pressed.contains(MouseButtons::from_iced(button))
    }

    pub fn has_pressed(&self) -> bool {
        !self.pressed.is_empty()
    }

    pub fn position(&self) -> Point {
        self.position
    }
}
