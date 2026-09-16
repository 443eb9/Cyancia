pub mod brush_preset;
pub mod canvas;
pub mod color_selector;
pub mod landing;
pub mod layers;
pub mod tool_box;
pub mod tool_options;

lapiz_i18n::define_i18n!("builtin_docks");

use lapiz_dock::DockRegistry;
use lapiz_runtime::{Application, plugin::Plugin};

use crate::{
    brush_preset::BrushPresetDock, color_selector::ColorSelectorDock, landing::LandingDock,
    layers::LayersDock, tool_box::ToolBoxDock, tool_options::ToolOptionsDock,
};

pub struct BuiltinDocksPlugin;

impl Plugin for BuiltinDocksPlugin {
    fn build(&self, _app: &mut Application) {
        i18n::init();
    }

    fn finish(&self, app: &mut Application) {
        let mut registry = DockRegistry::default();

        {
            let mut runtime = app.runtime_mut();
            let services = runtime.services_mut();

            registry.register(BrushPresetDock::new(services));
            registry.register(ColorSelectorDock::new(services));
            registry.register(ToolOptionsDock::new(services));
        }

        registry.register(LandingDock::new());
        registry.register(LayersDock::new());
        registry.register(ToolBoxDock::new());

        app.add_service_instance(registry);
    }
}
