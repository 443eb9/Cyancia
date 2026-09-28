use std::env;

use lapiz_about::AboutPlugin;
use lapiz_abr_bridge::AbrBridgePlugin;
use lapiz_actions::ActionPlugin;
use lapiz_assets::AssetsPlugin;
use lapiz_brush::BrushPlugin;
use lapiz_bucket_tool::BucketPlugin;
use lapiz_builtin_docks::BuiltinDocksPlugin;
use lapiz_canvas::CanvasPlugin;
use lapiz_color::ColorPlugin;
use lapiz_color_selector::ColorSelectorPlugin;
use lapiz_dirs::assets_dir;
use lapiz_eye_dropper::EyeDropperPlugin;
use lapiz_filter::FilterPlugin;
use lapiz_image::ImagePlugin;
use lapiz_image_exporter::ImageExporterPlugin;
use lapiz_image_importer::ImageImporterPlugin;
use lapiz_input::InputPlugin;
use lapiz_main_view::MainViewPlugin;
use lapiz_render::RenderPlugin;
use lapiz_runtime::Application;
use lapiz_selection_tool::SelectionPlugin;
use lapiz_shader_graph::ShaderGraphPlugin;
use lapiz_tools::ToolsPlugin;
use lapiz_transform_tool::FreeTransformPlugin;
use lapiz_undo::UndoPlugin;
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(not(target_os = "android"))]
fn main() {
    run();
}

pub(crate) fn run(#[cfg(target_os = "android")] android_app: AndroidApp) {
    #[cfg(target_os = "android")]
    {
        lapiz_dirs::set_android_data_dir(
            android_app.external_data_path().expect("Android data path"),
        );
    }

    lapiz_report::setup_panic_hook();

    tracing_subscriber::fmt()
        .with_env_filter("info,wgpu_hal=warn,iced_winit=warn,iced_wgpu=warn")
        .init();

    log::info!("Running at {}", env::current_dir().unwrap().display());

    let mut app = Application::new(
        #[cfg(target_os = "android")]
        android_app,
    );

    app.add_plugin(AssetsPlugin::new(assets_dir()))
        .add_plugin(AbrBridgePlugin)
        .add_plugin(UndoPlugin)
        .add_plugin(RenderPlugin)
        .add_plugin(ShaderGraphPlugin)
        .add_plugin(ToolsPlugin)
        .add_plugin(ImagePlugin)
        .add_plugin(CanvasPlugin)
        .add_plugin(InputPlugin)
        .add_plugin(BrushPlugin)
        .add_plugin(FilterPlugin)
        .add_plugin(BucketPlugin)
        .add_plugin(EyeDropperPlugin)
        .add_plugin(SelectionPlugin)
        .add_plugin(FreeTransformPlugin)
        .add_plugin(ColorPlugin)
        .add_plugin(ActionPlugin)
        .add_plugin(ColorSelectorPlugin)
        .add_plugin(BuiltinDocksPlugin)
        .add_plugin(ImageImporterPlugin)
        .add_plugin(ImageExporterPlugin)
        .add_plugin(AboutPlugin)
        .add_plugin(MainViewPlugin);
    app.build_plugins();

    lapiz_i18n::init();

    app.run().unwrap();
}
