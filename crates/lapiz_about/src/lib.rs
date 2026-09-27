use lapiz_runtime::{Application, plugin::Plugin};
use serde::Deserialize;

lapiz_i18n::define_i18n!("about");

mod view;

pub const ABOUT_VIEW_ID: &str = "about";

const GPL_LICENSE_TEXT: &str = include_str!("../../../LICENSE");
const MIT_LICENSE_TEXT: &str = include_str!("../../../LICENSES/MIT.txt");

#[derive(Debug, Deserialize)]
pub struct ThirdPartyLicenses {
    #[serde(default)]
    pub licenses: Vec<LicenseEntry>,
    #[serde(default)]
    pub crates: Vec<CrateEntry>,
}

#[derive(Debug, Deserialize)]
pub struct LicenseEntry {
    pub id: String,
    pub name: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct CrateEntry {
    pub name: String,
    pub version: String,
    pub repository: Option<String>,
    pub license: usize,
}

impl ThirdPartyLicenses {
    fn parse_embedded() -> Self {
        serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/third_party_licenses.json"
        )))
        .expect("embedded third party licenses must be valid JSON")
    }
}

pub struct AboutPlugin;

impl Plugin for AboutPlugin {
    fn build(&self, app: &mut Application) {
        i18n::init();

        app.runtime_mut()
            .window_manager_mut()
            .register_view::<view::AboutView>();
    }
}
