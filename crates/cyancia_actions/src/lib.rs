use std::{any::Any, cell::UnsafeCell, collections::HashMap};

use cyancia_id::Id;
use cyancia_input::{action::Action, key::KeySequence};
use iced_core::Point;
use parking_lot::RwLock;

use crate::shell::CShell;

pub mod control;
pub mod files;
pub mod shell;

pub trait ActionFunction: Send + Sync + 'static {
    type State: Send + Sync + 'static;

    fn id(&self) -> Id<Action>;
    fn default_state(&self) -> Self::State;
    fn activate(&self, shortcut: KeySequence, shell: &mut CShell, state: &mut Self::State) {}
    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Self::State,
    ) {
    }
    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Self::State,
    ) {
    }
    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Self::State,
    ) {
    }
    fn deactivate(&self, shortcut: KeySequence, shell: &mut CShell, state: &mut Self::State) {}
}

pub trait ErasedActionFunction {
    fn id(&self) -> Id<Action>;
    fn default_state(&self) -> Box<dyn Any + Send + Sync>;
    fn activate(
        &self,
        shortcut: KeySequence,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    );
    fn deactivate(
        &self,
        shortcut: KeySequence,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    );
}

impl<T: ActionFunction> ErasedActionFunction for T {
    fn id(&self) -> Id<Action> {
        <T as ActionFunction>::id(self)
    }

    fn default_state(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(<T as ActionFunction>::default_state(self))
    }

    fn activate(
        &self,
        shortcut: KeySequence,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as ActionFunction>::activate(self, shortcut, shell, state);
    }

    fn begin(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as ActionFunction>::begin(self, shortcut, cursor, shell, state);
    }

    fn update(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as ActionFunction>::update(self, shortcut, cursor, shell, state);
    }

    fn end(
        &self,
        shortcut: KeySequence,
        cursor: Point,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as ActionFunction>::end(self, shortcut, cursor, shell, state);
    }

    fn deactivate(
        &self,
        shortcut: KeySequence,
        shell: &mut CShell,
        state: &mut Box<dyn Any + Send + Sync>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("CanvasAction state has incorrect type");
        <T as ActionFunction>::deactivate(self, shortcut, shell, state);
    }
}

pub struct StatefulActionFunction {
    action: Box<dyn ErasedActionFunction>,
    state: RwLock<Box<dyn Any + Send + Sync>>,
}

impl StatefulActionFunction {
    pub fn id(&self) -> Id<Action> {
        self.action.id()
    }

    pub fn activate(&self, shortcut: KeySequence, shell: &mut CShell) {
        self.action
            .activate(shortcut, shell, &mut self.state.write());
    }

    pub fn begin(&self, shortcut: KeySequence, cursor: Point, shell: &mut CShell) {
        self.action
            .begin(shortcut, cursor, shell, &mut self.state.write());
    }

    pub fn update(&self, shortcut: KeySequence, cursor: Point, shell: &mut CShell) {
        self.action
            .update(shortcut, cursor, shell, &mut self.state.write());
    }

    pub fn end(&self, shortcut: KeySequence, cursor: Point, shell: &mut CShell) {
        self.action
            .end(shortcut, cursor, shell, &mut self.state.write());
    }

    pub fn deactivate(&self, shortcut: KeySequence, shell: &mut CShell) {
        self.action
            .deactivate(shortcut, shell, &mut self.state.write());
    }
}

pub struct ActionFunctionCollection {
    actions: HashMap<Id<Action>, StatefulActionFunction>,
}

impl ActionFunctionCollection {
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
        }
    }

    pub fn register<A: ActionFunction + Default>(&mut self) {
        let action = A::default();
        self.actions.insert(
            action.id(),
            StatefulActionFunction {
                state: RwLock::new(Box::new(action.default_state())),
                action: Box::new(action),
            },
        );
    }

    pub fn get(&self, id: &Id<Action>) -> Option<&StatefulActionFunction> {
        self.actions.get(id)
    }
}
