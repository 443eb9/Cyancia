use cyancia_assets::AssetAppExt;
use cyancia_runtime::{Application, plugin::Plugin};

pub mod key;
pub mod mouse;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut Application) {}

    fn finish(&self, app: &mut Application) {}
}
