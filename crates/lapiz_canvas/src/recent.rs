use std::path::{Path, PathBuf};

use lapiz_config::Configuration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentFiles {
    pub files: Vec<RecentFileRecord>,
}

impl RecentFiles {
    pub fn update(&mut self, path: &Path) {
        if let Some(index) = self.files.iter().position(|r| r.path == path) {
            let rec = self.files.remove(index);
            self.files.push(rec);
        } else {
            self.files.push(RecentFileRecord {
                path: path.to_path_buf(),
            });
        }
    }
}

impl Configuration for RecentFiles {
    const NAME: &'static str = "recent_files.toml";

    const DEFAULT: &'static str = "files = []";
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RecentFileRecord {
    pub path: PathBuf,
}
