use std::{any::TypeId, collections::HashMap, path::Path, sync::Arc};

use lapiz_runtime::global::{Global, Globals};

use crate::bundle::ErasedAssetBundle;

pub trait AssetSource: 'static {
    fn scan(root: &Path) -> Vec<Arc<dyn ErasedAssetBundle>>;
}

#[derive(Default)]
pub struct AssetSourceRegistry {
    inner: HashMap<TypeId, Box<dyn Fn(&Path) -> Vec<Arc<dyn ErasedAssetBundle>>>>,
}

impl Global for AssetSourceRegistry {}

impl AssetSourceRegistry {
    pub fn register<T: AssetSource>(&mut self) {
        self.inner.insert(TypeId::of::<T>(), Box::new(T::scan));
    }

    pub fn scan_all(&self, root: &Path) -> Vec<Arc<dyn ErasedAssetBundle>> {
        self.inner.values().flat_map(|scan| scan(root)).collect()
    }
}

pub trait AssetSourceRegistryExt {
    fn add_asset_source<T: AssetSource>(&mut self) -> &mut Self;
}

impl AssetSourceRegistryExt for Globals {
    fn add_asset_source<T: AssetSource>(&mut self) -> &mut Self {
        self.global_mut::<AssetSourceRegistry>().register::<T>();
        self
    }
}
