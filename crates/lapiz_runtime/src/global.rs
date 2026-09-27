use crate::Globals;

pub trait Global: 'static {}

pub trait FromGlobals {
    fn from_globals(globals: &Globals) -> Self;
}

impl<T: Default> FromGlobals for T {
    fn from_globals(_globals: &Globals) -> Self {
        Self::default()
    }
}
