use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use directories::BaseDirs;

static BASE_DIRS: LazyLock<Option<BaseDirs>> = LazyLock::new(BaseDirs::new);

static CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(dir) = std::env::var("CONFIG_DIR") {
        PathBuf::from(dir)
    } else if let Some(dir) = BASE_DIRS.as_ref() {
        dir.config_local_dir().join("lapiz")
    } else if let Ok(dir) = std::env::current_exe() {
        dir.parent().unwrap().join("configs")
    } else {
        PathBuf::from("configs")
    }
});

pub fn config_dir() -> &'static Path {
    &CONFIG_DIR
}
