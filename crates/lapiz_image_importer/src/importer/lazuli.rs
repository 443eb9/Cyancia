use std::path::Path;

use anyhow::Result;
use iced_core::{Element, Theme};
use iced_runtime::Task;
use lapiz_i18n::t;
use lapiz_lazuli::LazuliArchive;
use lapiz_runtime::{Renderer, Services};

use crate::ImageFormatImporter;

#[derive(Default)]
pub struct LazuliImporter;

impl ImageFormatImporter for LazuliImporter {
    type ImportDialogMessage = ();

    fn extension() -> &'static str {
        lapiz_lazuli::EXTENSION
    }

    fn description() -> String {
        t!("lazuli_image_description")
    }

    fn has_import_options() -> bool {
        false
    }

    fn import_dialog_view(&self, _: &Services) -> Element<'_, (), Theme, Renderer> {
        iced_widget::Column::new().into()
    }

    fn import_dialog_update(&mut self, _: (), _: &mut Services) -> Task<()> {
        Task::none()
    }

    #[tracing::instrument(skip_all)]
    async fn import(&self, _: &Services, path: &Path) -> Result<LazuliArchive> {
        LazuliArchive::open(path)
    }

    fn to_toml(&self) -> Result<toml::Value> {
        Ok(toml::Value::Table(Default::default()))
    }

    fn from_toml(&mut self, _: toml::Value) -> Result<()> {
        Ok(())
    }
}
