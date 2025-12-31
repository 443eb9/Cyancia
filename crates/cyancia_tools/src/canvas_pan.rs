use cyancia_actions::{ActionFunction, control::CanvasPanAction, shell::CShell};
use cyancia_canvas::CCanvas;
use cyancia_id::Id;
use cyancia_input::{action::Action, key::KeySequence};
use iced_core::Point;

use crate::CanvasTool;

#[derive(Default)]
pub struct CanvasPanTool {
    action: CanvasPanAction,
}

impl CanvasTool for CanvasPanTool {}

impl ActionFunction for CanvasPanTool {
    type State = <CanvasPanAction as ActionFunction>::State;

    fn id(&self) -> Id<Action> {
        Id::from_str("canvas_pan_tool")
    }

    fn default_state(&self) -> Self::State {
        self.action.default_state()
    }

    fn activate(&self, shortcut: KeySequence, shell: &mut CShell, state: &mut Self::State) {
        self.action.activate(shortcut, shell, state)
    }

    fn begin(&self, shortcut: KeySequence, cursor: Point, shell: &mut CShell, state: &mut Self::State) {
        self.action.begin(shortcut, cursor, shell, state)
    }

    fn update(&self, shortcut: KeySequence, cursor: Point, shell: &mut CShell, state: &mut Self::State) {
        self.action.update(shortcut, cursor, shell, state)
    }

    fn end(&self, shortcut: KeySequence, cursor: Point, shell: &mut CShell, state: &mut Self::State) {
        self.action.end(shortcut, cursor, shell, state)
    }

    fn deactivate(&self, shortcut: KeySequence, shell: &mut CShell, state: &mut Self::State) {
        self.action.deactivate(shortcut, shell, state)
    }
}
