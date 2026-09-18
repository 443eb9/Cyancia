use lapiz_assets::AssetAppExt;
use lapiz_runtime::{Application, plugin::Plugin};

use crate::asset::EffectAssetSerializer;

lapiz_i18n::define_i18n!("effect");

pub mod asset;
pub mod editor;
pub mod instance;
pub mod nodes;
pub mod render;

pub fn init_i18n() {
    crate::i18n::init();
}

pub struct EffectPlugin;

impl Plugin for EffectPlugin {
    fn build(&self, app: &mut Application) {
        crate::i18n::init();
        app.runtime_mut()
            .services_mut()
            .add_asset_serializer::<EffectAssetSerializer>();
    }
}
