pub mod brush_preset;
pub mod canvas;
pub mod color_selector;
pub mod layers;
pub mod tool_box;
pub mod tool_options;

lapiz_i18n::define_i18n!("builtin_docks");

pub use brush_preset::{BRUSH_PRESETS_DOCK_ID, BrushPresetDock, BrushPresetDockMessage};
pub use canvas::{CanvasDock, CanvasDockMessage, construct_canvas_dock_id};
pub use color_selector::{COLOR_SELECTOR_DOCK_ID, ColorSelectorDock, ColorSelectorDockMessage};
use lapiz_dock::DockRegistry;
use lapiz_runtime::{Application, plugin::Plugin};
pub use layers::{LAYER_DOCK_ID, LayersDock, LayersDockMessage};
pub use tool_box::{TOOL_BOX_DOCK_ID, ToolBoxDock, ToolBoxDockMessage};
pub use tool_options::{TOOL_OPTIONS_DOCK_ID, ToolOptionsDock, ToolOptionsDockMessage};

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

        registry.register(LayersDock::new());
        registry.register(ToolBoxDock::new());

        app.add_service_instance(registry);
    }
}
