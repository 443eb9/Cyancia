use crate::Runtime;

pub trait Plugin: 'static {
    fn build(&self, app: &mut Runtime);
    fn finish(&self, _app: &mut Runtime) {}
}
