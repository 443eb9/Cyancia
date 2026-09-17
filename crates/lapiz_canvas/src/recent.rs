use std::path::PathBuf;

use lapiz_config::Configuration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentFiles {
    pub files: Vec<RecentFileRecord>,
}

impl Configuration for RecentFiles {
    const NAME: &'static str = "recent_files.toml";

    const DEFAULT: &'static str = "files = []";
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentFileRecord {
    pub path: PathBuf,
}
