use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub trait Global: 'static {}

pub trait FromGlobals {
    fn from_globals(globals: &Globals) -> Self;
}

impl<T: Default> FromGlobals for T {
    fn from_globals(_globals: &Globals) -> Self {
        Self::default()
    }
}

#[derive(Default)]
pub struct Globals {
    globals: HashMap<TypeId, Box<dyn Any>>,
}

impl Globals {
    pub fn global<T: Global>(&self) -> &T {
        self.globals
            .get(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Global of type {} not found", std::any::type_name::<T>()))
            .downcast_ref()
            .unwrap_or_else(|| {
                panic!(
                    "Global of type {} has wrong type. This should not happen.",
                    std::any::type_name::<T>()
                )
            })
    }

    pub fn global_mut<T: Global>(&mut self) -> &mut T {
        self.globals
            .get_mut(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Global of type {} not found", std::any::type_name::<T>()))
            .downcast_mut()
            .unwrap_or_else(|| {
                panic!(
                    "Global of type {} has wrong type. This should not happen.",
                    std::any::type_name::<T>()
                )
            })
    }

    pub fn has_global<T: Global>(&self) -> bool {
        self.globals.contains_key(&TypeId::of::<T>())
    }

    pub fn get_global<T: Global>(&self) -> Option<&T> {
        self.globals
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref())
    }

    pub fn get_global_mut<T: Global>(&mut self) -> Option<&mut T> {
        self.globals
            .get_mut(&TypeId::of::<T>())
            .and_then(|value| value.downcast_mut())
    }

    pub fn remove_global<T: Global>(&mut self) -> T {
        let s = self
            .globals
            .remove(&TypeId::of::<T>())
            .unwrap_or_else(|| panic!("Global of type {} not found", std::any::type_name::<T>()));

        match s.downcast() {
            Ok(s) => *s,
            Err(_) => {
                panic!(
                    "Global of type {} has wrong type. This should not happen.",
                    std::any::type_name::<T>()
                )
            }
        }
    }

    pub fn try_remove_global<T: Global>(&mut self) -> Option<T> {
        let s = self.globals.remove(&TypeId::of::<T>())?;

        match s.downcast() {
            Ok(s) => Some(*s),
            Err(_) => {
                panic!(
                    "Global of type {} has wrong type. This should not happen.",
                    std::any::type_name::<T>()
                )
            }
        }
    }

    pub fn insert_global<T: Global>(&mut self, value: T) {
        self.globals.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn update_global<T: Global, O>(&mut self, f: impl FnOnce(&mut T, &mut Self) -> O) -> O {
        let mut s = self.remove_global::<T>();
        let result = f(&mut s, self);
        self.insert_global(s);
        result
    }

    pub fn try_update_scope<T: Global, O>(
        &mut self,
        f: impl FnOnce(&mut T, &mut Self) -> O,
    ) -> Option<O> {
        let mut s = self.try_remove_global::<T>()?;
        let result = f(&mut s, self);
        self.insert_global(s);
        Some(result)
    }
}
