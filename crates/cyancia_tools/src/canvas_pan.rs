use cyancia_canvas::{
    CCanvas,
    action::{CanvasAction, control::CanvasPanAction},
};
use cyancia_id::Id;
use cyancia_input::{action::Action, key::KeySequence};
use iced_core::Point;

use crate::CanvasTool;

#[derive(Default)]
pub struct CanvasPanTool {
    action: CanvasPanAction,
}

impl CanvasTool for CanvasPanTool {}

impl CanvasAction for CanvasPanTool {
    type State = <CanvasPanAction as CanvasAction>::State;

    fn id(&self) -> Id<Action> {
        Id::from_str("canvas_pan_tool")
    }

    fn default_state(&self) -> Self::State {
        self.action.default_state()
    }

    fn activate(&self, shortcut: KeySequence, canvas: &CCanvas, state: &mut Self::State) {
        self.action.activate(shortcut, canvas, state)
    }

    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &CCanvas,
        state: &mut Self::State,
    ) {
        self.action.begin(shortcut, cursor, canvas, state)
    }

    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &CCanvas,
        state: &mut Self::State,
    ) {
        self.action.update(shortcut, cursor, canvas, state)
    }

    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &CCanvas,
        state: &mut Self::State,
    ) {
        self.action.end(shortcut, cursor, canvas, state)
    }

    fn deactivate(&self, shortcut: KeySequence, canvas: &CCanvas, state: &mut Self::State) {
        self.action.deactivate(shortcut, canvas, state)
    }
}
