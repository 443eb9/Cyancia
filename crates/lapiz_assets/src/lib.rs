use std::{path::PathBuf, sync::Arc};

use lapiz_runtime::{Globals, Runtime, plugin::Plugin};

use crate::{
    bundle::ErasedAssetBundle,
    loader::{AssetRegistryBuilder, AssetSerializer},
    store::AssetRegistry,
};

pub mod asset;
pub mod bundle;
pub mod embedded;
pub mod error;
pub mod index_db;
pub mod loader;
pub mod store;
pub mod tag;

pub struct AssetsPlugin {
    pub asset_root: PathBuf,
    pub bundles: Vec<Arc<dyn ErasedAssetBundle>>,
}

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut Runtime) {
        let mut builder = AssetRegistryBuilder::default();
        builder.set_root(self.asset_root.clone());

        for bundle in &self.bundles {
            builder.add_bundle(bundle.clone());
        }

        app.add_global_instance(builder);
    }

    fn finish(&self, app: &mut Runtime) {
        let builder = app.globals_mut().remove_global::<AssetRegistryBuilder>();
        app.add_global_instance(builder.build());
    }
}

pub trait AssetAppExt {
    fn add_asset_serializer<A: AssetSerializer + Default>(&mut self);
    fn add_asset_bundle(&mut self, bundle: Arc<dyn ErasedAssetBundle>);
    fn assets(&self) -> &AssetRegistry;
}

impl AssetAppExt for Globals {
    fn add_asset_serializer<A: AssetSerializer + Default>(&mut self) {
        self.global_mut::<AssetRegistryBuilder>()
            .add_serializer::<A>();
    }

    fn add_asset_bundle(&mut self, bundle: Arc<dyn ErasedAssetBundle>) {
        self.global_mut::<AssetRegistryBuilder>().add_bundle(bundle);
    }

    fn assets(&self) -> &AssetRegistry {
        self.global::<AssetRegistry>()
    }
}
