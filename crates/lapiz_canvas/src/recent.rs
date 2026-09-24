use std::path::{Path, PathBuf};

use lapiz_config::Configuration;
use lapiz_dirs::cache_dir;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_128;

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

pub fn recent_file_thumbnail_path(path: &Path) -> PathBuf {
    cache_dir().join("file_thumbnails").join(format!(
        "{}.png",
        Uuid::from_u128(xxh3_128(path.as_os_str().as_encoded_bytes(),))
    ))
}
