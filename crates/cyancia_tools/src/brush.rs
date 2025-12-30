use cyancia_canvas::{CCanvas, action::CanvasAction};
use cyancia_id::Id;
use cyancia_input::{action::Action, key::KeySequence};
use iced_core::Point;

use crate::CanvasTool;

#[derive(Default)]
pub struct BrushTool;

impl CanvasTool for BrushTool {}

impl CanvasAction for BrushTool {
    type State = ();

    fn id(&self) -> Id<Action> {
        Id::from_str("brush")
    }

    fn default_state(&self) -> Self::State {
        ()
    }

    fn activate(&self, shortcut: KeySequence, canvas: &mut CCanvas, state: &mut Self::State) {
        println!("Switched to brush!");
    }

    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        println!("Painting at: {:?}", cursor);
    }

    fn deactivate(&self, shortcut: KeySequence, canvas: &mut CCanvas, state: &mut Self::State) {
        println!("Exited brush!");
    }
}
