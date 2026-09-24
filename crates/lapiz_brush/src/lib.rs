use futures::StreamExt as _;
use iced_futures::{Executor as _, backend::default};
use iced_runtime::{Action, Task, task::into_stream};
use lapiz_assets::AssetAppExt as _;
use lapiz_runtime::{Application, plugin::Plugin};
use lapiz_tools::ToolsAppExt as _;

use crate::{
    asset::{BrushPreset, BrushPresetSerializer},
    editor::BrushEditor,
    render::stroke_preview::load_cached_stroke_preview_or_generate,
    tool::BrushTool,
};

pub mod asset;
pub mod editor;
pub mod input_processing;
pub mod instance;
pub mod render;
pub mod tool;
pub mod widget;

wesl::wesl_pkg!(pub brush);

lapiz_i18n::define_i18n!("brush");

pub struct BrushPlugin;

impl Plugin for BrushPlugin {
    fn build(&self, app: &mut Application) {
        i18n::init();
        let mut runtime = app.runtime_mut();
        runtime.window_manager_mut().register_view::<BrushEditor>();

        let services = runtime.services_mut();
        services.add_asset_serializer::<BrushPresetSerializer>();
        services.add_tool_function::<BrushTool>();
    }

    fn finish(&self, app: &mut Application) {
        let runtime = app.runtime();
        let services = runtime.services();
        let assets = services.assets().clone();

        let brushes = assets
            .all_handles_of::<BrushPreset>()
            .expect("Failed to enumerate brush presets");

        let preview_tasks = brushes.into_iter().filter_map(|brush| {
            let brush_id = brush.id();
            match load_cached_stroke_preview_or_generate(&brush, &assets, services) {
                Ok(task) => Some(task.map(move |result| (brush_id, result))),
                Err(error) => {
                    log::error!("Failed to prepare preview for brush {brush_id}: {error:#}");
                    None
                }
            }
        });
        let task = Task::batch(preview_tasks);

        let executor = default::Executor::new().expect("Failed to create preview task executor");
        if let Some(stream) = into_stream(task) {
            executor.spawn(stream.for_each(|action| async move {
                if let Action::Output((brush_id, result)) = action
                    && let Err(error) = result
                {
                    log::error!("Failed to generate preview for brush {brush_id}: {error:#}");
                }
            }));
        }
    }
}
