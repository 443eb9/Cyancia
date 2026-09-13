use std::fs;

use chrono::Utc;
use iced_runtime::Task;
use lapiz_render::render_context::RenderContextAppExt;
use lapiz_runtime::Services;
use lapiz_utils::log_err::LogErr;

use crate::{ActionFunction, ActionId};

fn reports_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("reports")
}

fn report_filename() -> String {
    format!("report-{}.txt", Utc::now().to_rfc3339().replace(':', "-"))
}

#[derive(Default)]
pub struct GenerateReportAction;

impl ActionFunction for GenerateReportAction {
    type Message = ();

    fn id(&self) -> ActionId {
        ActionId("generate_report_action".into())
    }

    fn trigger(&self, services: &mut Services) -> Task<Self::Message> {
        let Ok(report) = lapiz_report::report(services.render_device()).logged_err() else {
            return Task::none();
        };

        log::info!("{}", report);

        let reports_path = reports_path();
        if fs::create_dir_all(&reports_path).logged_err().is_err() {
            return Task::none();
        }
        let report_path = reports_path.join(report_filename());
        if fs::write(&report_path, report).logged_err().is_err() {
            return Task::none();
        }

        log::info!("Report saved to {}", report_path.display());

        Task::none()
    }
}

#[derive(Default)]
pub struct DebugManualPanicAction;

impl ActionFunction for DebugManualPanicAction {
    type Message = ();

    fn id(&self) -> ActionId {
        ActionId("debug_manual_panic_action".into())
    }

    fn trigger(&self, services: &mut Services) -> Task<Self::Message> {
        panic!("Debug panic");
    }
}
