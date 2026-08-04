use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    time::Instant,
};

use cyancia_runtime::{Application, Services, plugin::Plugin, service::Service};
use cyancia_utils::{Deref, DerefMut, log_err::LogErr};
use downcast_rs::Downcast;
use futures::channel::oneshot::{self, Canceled, Receiver, Sender};
use tracing::info;
use uuid::Uuid;

pub struct UndoPlugin;

impl Plugin for UndoPlugin {
    fn build(&self, app: &mut Application) {
        app.add_service::<UndoStacks>();
    }
}

pub struct QueuedUndoCommand {
    stack_id: Uuid,
    tx: Sender<Box<dyn UndoCommand>>,
}

impl QueuedUndoCommand {
    pub fn send(self, cmd: Box<dyn UndoCommand>, services: &mut Services) -> anyhow::Result<()> {
        let _ = self.tx.send(cmd);
        services.service_scope::<UndoStacks, _>(|stacks, services| {
            stacks
                .get_mut(&self.stack_id)
                .expect("Undo stack for queued command should exist")
                .poll(services)
        })
    }
}

#[derive(Default, Deref, DerefMut)]
pub struct UndoStacks {
    stacks: HashMap<Uuid, UndoStack>,
}

impl Service for UndoStacks {}

pub struct UndoStack {
    id: Uuid,
    cursor: usize,
    history: VecDeque<UndoCommandData>,
    queue: VecDeque<Receiver<Box<dyn UndoCommand>>>,
    max_history: usize,
}

impl UndoStack {
    pub fn new(id: Uuid, max_history: usize) -> Self {
        Self {
            id,
            cursor: 0,
            history: VecDeque::new(),
            queue: VecDeque::new(),
            max_history,
        }
    }

    pub fn queue(&mut self) -> QueuedUndoCommand {
        let (tx, rx) = oneshot::channel();
        self.queue.push_back(rx);
        QueuedUndoCommand {
            stack_id: self.id,
            tx,
        }
    }

    pub fn push<C: UndoCommand>(&mut self, cmd: C, services: &mut Services) -> anyhow::Result<()> {
        self.push_boxed(Box::new(cmd), services)
    }

    pub fn push_boxed(
        &mut self,
        cmd: Box<dyn UndoCommand>,
        services: &mut Services,
    ) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.push_internal(cmd, services)?;
        } else {
            let (tx, rx) = oneshot::channel();
            self.queue.push_back(rx);
            let _ = tx.send(cmd);
            self.poll(services)?;
        }

        Ok(())
    }

    fn push_internal(
        &mut self,
        mut cmd: Box<dyn UndoCommand>,
        services: &mut Services,
    ) -> anyhow::Result<()> {
        info!("Push command {}", cmd.label());
        self.history.truncate(self.cursor);

        if let Some(rhs) = self.history.back()
            && rhs.command.can_cancel_out(cmd.as_ref())
        {
            cmd.redo(services)?;
            self.history.pop_back();
        } else {
            if self.history.len() == self.max_history {
                self.history.pop_front();
            }

            cmd.redo(services)?;
            self.history.push_back(UndoCommandData {
                _pushed_at: Instant::now(),
                command: cmd,
            });
        }

        self.cursor = self.len();
        Ok(())
    }

    pub fn poll(&mut self, services: &mut Services) -> anyhow::Result<()> {
        while let Some(first) = self.queue.front_mut() {
            let cmd = match first.try_recv() {
                Ok(Some(cmd)) => cmd,
                Ok(None) => break,
                Err(Canceled) => {
                    self.queue.pop_front();
                    continue;
                }
            };

            self.queue.pop_front();
            self.push_internal(cmd, services)?;
        }

        Ok(())
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: usize, services: &mut Services) -> anyhow::Result<()> {
        if cursor > self.len() {
            return Err(anyhow::anyhow!(
                "cursor {} out of bounds {}",
                cursor,
                self.len()
            ));
        }

        while self.cursor < cursor {
            self.cursor += 1;

            let data = &mut self.history[self.cursor - 1];
            info!("Redo {}", data.command.label());
            data.command.redo(services).logged_err()?;
        }
        while self.cursor > cursor {
            self.cursor -= 1;

            let data = &mut self.history[self.cursor];
            info!("Undo {}", data.command.label());
            data.command.undo(services).logged_err()?;
        }

        Ok(())
    }

    pub fn undo(&mut self, services: &mut Services) -> anyhow::Result<()> {
        if self.cursor == 0 {
            return Err(anyhow::anyhow!("undo stack is empty"));
        }
        self.set_cursor(self.cursor - 1, services)
    }

    pub fn redo(&mut self, services: &mut Services) -> anyhow::Result<()> {
        if self.cursor == self.len() {
            return Err(anyhow::anyhow!("undo stack reached the end"));
        }
        self.set_cursor(self.cursor + 1, services)
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

impl std::fmt::Debug for UndoStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UndoStack")
            .field("cursor", &self.cursor)
            .field("len", &self.len())
            .field(
                "history",
                &self
                    .history
                    .iter()
                    .map(|data| data.command.label())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

pub struct UndoCommandData {
    _pushed_at: Instant,
    command: Box<dyn UndoCommand>,
}

pub trait UndoCommand: 'static + Downcast {
    fn label(&self) -> Cow<'static, str>;
    fn redo(&mut self, services: &mut Services) -> anyhow::Result<()>;
    fn undo(&mut self, services: &mut Services) -> anyhow::Result<()>;
    fn can_cancel_out(&self, _: &dyn UndoCommand) -> bool {
        false
    }
}
downcast_rs::impl_downcast!(UndoCommand);

pub struct BatchedUndoCommand {
    label: Cow<'static, str>,
    commands: Vec<Box<dyn UndoCommand>>,
}

impl BatchedUndoCommand {
    pub fn new<T: UndoCommand>(label: Cow<'static, str>, commands: Vec<T>) -> Self {
        Self {
            label,
            commands: commands.into_iter().map(|c| Box::new(c) as _).collect(),
        }
    }

    pub fn new_boxed(label: Cow<'static, str>, commands: Vec<Box<dyn UndoCommand>>) -> Self {
        Self { label, commands }
    }
}

impl UndoCommand for BatchedUndoCommand {
    fn label(&self) -> Cow<'static, str> {
        self.label.clone()
    }

    fn redo(&mut self, services: &mut Services) -> anyhow::Result<()> {
        let mut success = 0;
        for i in 0..self.commands.len() {
            match self.commands[i].redo(services) {
                Ok(_) => success += 1,
                Err(err) => {
                    for i in (0..success).rev() {
                        self.commands[i].undo(services)?;
                    }
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    fn undo(&mut self, services: &mut Services) -> anyhow::Result<()> {
        let mut success = 0;
        for i in (0..self.commands.len()).rev() {
            match self.commands[i].undo(services) {
                Ok(_) => success += 1,
                Err(err) => {
                    for i in self.commands.len() - success..self.commands.len() {
                        self.commands[i].redo(services)?;
                    }
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Counter(i32);

    impl Service for Counter {}

    struct Add(i32);

    impl UndoCommand for Add {
        fn label(&self) -> Cow<'static, str> {
            "Add".into()
        }

        fn redo(&mut self, services: &mut Services) -> anyhow::Result<()> {
            services.service_mut::<Counter>().0 += self.0;
            Ok(())
        }

        fn undo(&mut self, services: &mut Services) -> anyhow::Result<()> {
            services.service_mut::<Counter>().0 -= self.0;
            Ok(())
        }
    }

    #[test]
    fn pushes_undoes_and_redoes() {
        let mut services = Services::default();
        services.insert_service(Counter::default());
        let mut stack = UndoStack::new(Uuid::new_v4(), 10);

        stack.push(Add(3), &mut services).unwrap();
        assert_eq!(services.service::<Counter>().0, 3);
        stack.undo(&mut services).unwrap();
        assert_eq!(services.service::<Counter>().0, 0);
        stack.redo(&mut services).unwrap();
        assert_eq!(services.service::<Counter>().0, 3);
    }

    #[test]
    fn queued_command_is_applied_in_order() {
        let id = Uuid::new_v4();
        let mut services = Services::default();
        services.insert_service(Counter::default());
        let mut stacks = UndoStacks::default();
        stacks.insert(id, UndoStack::new(id, 10));
        let queued = stacks.get_mut(&id).unwrap().queue();
        services.insert_service(stacks);

        queued.send(Box::new(Add(4)), &mut services).unwrap();

        assert_eq!(services.service::<Counter>().0, 4);
        assert_eq!(services.service::<UndoStacks>().get(&id).unwrap().len(), 1);
    }
}
