use crate::shell::CShell;

pub trait ActionTask: Send + Sync + 'static {
    fn apply(self: Box<Self>, shell: &mut CShell);
}
