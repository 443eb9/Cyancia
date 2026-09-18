use lapiz_assets::AssetAppExt;
use lapiz_runtime::{Application, plugin::Plugin};

use crate::asset::EffectAssetSerializer;

pub mod asset;
pub mod instance;
pub mod nodes;
pub mod render;

pub struct EffectPlugin;

impl Plugin for EffectPlugin {
    fn build(&self, app: &mut Application) {
        app.runtime_mut()
            .services_mut()
            .add_asset_serializer::<EffectAssetSerializer>();
    }
}
