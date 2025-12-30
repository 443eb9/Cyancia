use std::{any::Any, cell::UnsafeCell, collections::HashMap};

use cyancia_id::Id;
use cyancia_input::{action::Action, key::KeySequence};
use iced_core::Point;
use parking_lot::RwLock;

use crate::CCanvas;

pub mod control;

pub trait CanvasAction: Send + Sync + 'static {
    type State: Send + Sync + 'static;

    fn id(&self) -> Id<Action>;
    fn default_state(&self) -> Self::State;
    fn activate(&self, shortcut: KeySequence, canvas: &mut CCanvas, state: &mut Self::State) {}
    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
    }
    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
    }
    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Self::State,
    ) {
    }
    fn deactivate(&self, shortcut: KeySequence, canvas: &mut CCanvas, state: &mut Self::State) {}
}

pub trait ErasedCanvasAction {
    fn id(&self) -> Id<Action>;
    fn default_state(&self) -> Box<dyn Any + Send + Sync>;
    fn prepare(
        &self,
        shortcut: KeySequence,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn deactivate(
        &self,
        shortcut: KeySequence,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    );
}

impl<T: CanvasAction> ErasedCanvasAction for T {
    fn id(&self) -> Id<Action> {
        <T as CanvasAction>::id(self)
    }

    fn default_state(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(<T as CanvasAction>::default_state(self))
    }

    fn prepare(
        &self,
        shortcut: KeySequence,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as CanvasAction>::activate(self, shortcut, canvas, state);
    }

    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as CanvasAction>::begin(self, shortcut, cursor, canvas, state);
    }

    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as CanvasAction>::update(self, shortcut, cursor, canvas, state);
    }

    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as CanvasAction>::end(self, shortcut, cursor, canvas, state);
    }

    fn deactivate(
        &self,
        shortcut: KeySequence,
        canvas: &mut CCanvas,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as CanvasAction>::deactivate(self, shortcut, canvas, state);
    }
}

pub struct StatefulCanvasAction {
    action: Box<dyn ErasedCanvasAction>,
    state: RwLock<Box<dyn Any + Send + Sync>>,
}

impl StatefulCanvasAction {
    pub fn id(&self) -> Id<Action> {
        self.action.id()
    }

    pub fn prepare(&self, shortcut: KeySequence, canvas: &mut CCanvas) {
        self.action
            .prepare(shortcut, canvas, &mut self.state.write());
    }

    pub fn begin(&self, shortcut: KeySequence, cursor: Point, canvas: &mut CCanvas) {
        self.action
            .begin(shortcut, cursor, canvas, &mut self.state.write());
    }

    pub fn update(&self, shortcut: KeySequence, cursor: Point, canvas: &mut CCanvas) {
        self.action
            .update(shortcut, cursor, canvas, &mut self.state.write());
    }

    pub fn end(&self, shortcut: KeySequence, cursor: Point, canvas: &mut CCanvas) {
        self.action
            .end(shortcut, cursor, canvas, &mut self.state.write());
    }

    pub fn deactivate(&self, shortcut: KeySequence, canvas: &mut CCanvas) {
        self.action
            .deactivate(shortcut, canvas, &mut self.state.write());
    }
}

pub struct CanvasActionCollection {
    actions: HashMap<Id<Action>, StatefulCanvasAction>,
}

impl CanvasActionCollection {
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
        }
    }

    pub fn register<A: CanvasAction + Default>(&mut self) {
        let action = A::default();
        self.actions.insert(
            action.id(),
            StatefulCanvasAction {
                state: RwLock::new(Box::new(action.default_state())),
                action: Box::new(action),
            },
        );
    }

    pub fn get(&self, id: &Id<Action>) -> Option<&StatefulCanvasAction> {
        self.actions.get(id)
    }
}
