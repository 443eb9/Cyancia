use lapiz_assets::AssetAppExt as _;
use lapiz_runtime::{Application, plugin::Plugin};

use crate::{asset::FilterPresetSerializer, editor::FilterEditor, panel::FilterPanel};

pub mod asset;
pub mod editor;
pub mod instance;
pub mod panel;
pub mod render;

lapiz_i18n::define_i18n!("filter");

pub struct FilterPlugin;

impl Plugin for FilterPlugin {
    fn build(&self, app: &mut Application) {
        crate::i18n::init();
        let mut runtime = app.runtime_mut();
        runtime.window_manager_mut().register_view::<FilterPanel>();
        runtime.window_manager_mut().register_view::<FilterEditor>();

        let services = runtime.services_mut();
        services.add_asset_serializer::<FilterPresetSerializer>();
    }
}
