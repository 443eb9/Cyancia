use cyancia_assets::AssetAppExt;
use cyancia_runtime::{Application, plugin::Plugin};
use cyancia_tools::ToolsAppExt;

use crate::{asset::BrushPresetSerializer, tool::BrushTool};

pub mod asset;
pub mod editor;
pub mod input_processing;
pub mod instance;
pub mod render;
pub mod tool;
pub mod widget;

pub struct BrushPlugin;

impl Plugin for BrushPlugin {
    fn build(&self, app: &mut Application) {
        let mut runtime = app.runtime_mut();
        let services = runtime.services_mut();
        services.add_asset_serializer::<BrushPresetSerializer>();
        services.add_tool_function::<BrushTool>();
    }
}
