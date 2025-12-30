use cyancia_id::Id;
use cyancia_input::{action::Action, key::KeySequence};
use iced_core::Point;

use crate::{CCanvas, action::CanvasAction};

#[derive(Default)]
pub struct CanvasPanAction;

pub struct CanvasPanActionState {}

impl CanvasAction for CanvasPanAction {
    type State = CanvasPanActionState;

    fn id(&self) -> Id<Action> {
        Id::from_str("canvas_pan_action")
    }

    fn default_state(&self) -> Self::State {
        CanvasPanActionState {}
    }

    fn prepare(&self, shortcut: KeySequence, canvas: &mut CCanvas, state: &mut Self::State) {
        dbg!();
    }

    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }

    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }

    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }
}

#[derive(Default)]
pub struct CanvasRotateAction;

pub struct CanvasRotateActionState {}

impl CanvasAction for CanvasRotateAction {
    type State = CanvasRotateActionState;

    fn id(&self) -> Id<Action> {
        Id::from_str("canvas_rotate_action")
    }

    fn default_state(&self) -> Self::State {
        CanvasRotateActionState {}
    }

    fn prepare(&self, shortcut: KeySequence, canvas: &mut CCanvas, state: &mut Self::State) {
        dbg!();
    }

    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }

    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }

    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }
}

#[derive(Default)]
pub struct CanvasZoomAction;

pub struct CanvasZoomActionState {}

impl CanvasAction for CanvasZoomAction {
    type State = CanvasZoomActionState;

    fn id(&self) -> Id<Action> {
        Id::from_str("canvas_zoom_action")
    }

    fn default_state(&self) -> Self::State {
        CanvasZoomActionState {}
    }

    fn prepare(&self, shortcut: KeySequence, canvas: &mut CCanvas, state: &mut Self::State) {
        dbg!();
    }

    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }

    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }

    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
        dbg!();
    }
}
