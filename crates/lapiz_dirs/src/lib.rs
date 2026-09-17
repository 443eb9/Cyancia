use std::{
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use directories::BaseDirs;

static BASE_DIRS: LazyLock<Option<BaseDirs>> = LazyLock::new(BaseDirs::new);

static CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    #[cfg(feature = "dev_local")]
    let path = PathBuf::new().join("target").join("configs");

    #[cfg(not(feature = "dev_local"))]
    let path = if let Ok(dir) = std::env::var("CONFIG_DIR") {
        PathBuf::from(dir)
    } else if let Some(dir) = BASE_DIRS.as_ref() {
        dir.config_local_dir().join("lapiz").join("configs")
    } else if let Ok(dir) = std::env::current_exe() {
        dir.parent().unwrap().join("configs")
    } else {
        PathBuf::from("configs")
    };

    fs::create_dir_all(&path).unwrap();
    path
});

pub fn config_dir() -> &'static Path {
    &CONFIG_DIR
}

static CACHE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    #[cfg(feature = "dev_local")]
    let path = PathBuf::new().join("target").join("cache");

    #[cfg(not(feature = "dev_local"))]
    let path = if let Ok(dir) = std::env::var("CACHE_DIR") {
        PathBuf::from(dir)
    } else if let Some(dir) = BASE_DIRS.as_ref() {
        dir.cache_dir().join("lapiz")
    } else if let Ok(dir) = std::env::current_exe() {
        dir.parent().unwrap().join("cache")
    } else {
        PathBuf::from("cache")
    };

    fs::create_dir_all(&path).unwrap();
    path
});

pub fn cache_dir() -> &'static Path {
    &CACHE_DIR
}

static PANIC_REPORTS_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    #[cfg(feature = "dev_local")]
    let path = PathBuf::new().join("target").join("panic_reports");

    #[cfg(not(feature = "dev_local"))]
    let path = if let Ok(dir) = std::env::var("PANIC_REPORTS_DIR") {
        PathBuf::from(dir)
    } else if let Some(dir) = BASE_DIRS.as_ref() {
        dir.cache_dir().join("lapiz").join("panic_reports")
    } else if let Ok(dir) = std::env::current_exe() {
        dir.parent().unwrap().join("panic_reports")
    } else {
        PathBuf::from("panic_reports")
    };

    fs::create_dir_all(&path).unwrap();
    path
});

pub fn panic_reports_dir() -> &'static Path {
    &PANIC_REPORTS_DIR
}

static REPORTS_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    #[cfg(feature = "dev_local")]
    let path = PathBuf::new().join("target").join("reports");

    #[cfg(not(feature = "dev_local"))]
    let path = if let Ok(dir) = std::env::var("REPORTS_DIR") {
        PathBuf::from(dir)
    } else if let Some(dir) = BASE_DIRS.as_ref() {
        dir.cache_dir().join("lapiz").join("reports")
    } else if let Ok(dir) = std::env::current_exe() {
        dir.parent().unwrap().join("reports")
    } else {
        PathBuf::from("reports")
    };

    fs::create_dir_all(&path).unwrap();
    path
});

pub fn reports_dir() -> &'static Path {
    &REPORTS_DIR
}

static ASSETS_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    #[cfg(feature = "dev_local")]
    let path = PathBuf::from("assets");

    #[cfg(not(feature = "dev_local"))]
    let path = if let Ok(dir) = std::env::var("ASSETS_DIR") {
        PathBuf::from(dir)
    } else if let Some(dir) = BASE_DIRS.as_ref() {
        dir.data_dir().join("lapiz").join("assets")
    } else if let Ok(dir) = std::env::current_exe() {
        dir.parent().unwrap().join("assets")
    } else {
        PathBuf::from("assets")
    };

    fs::create_dir_all(&path).unwrap();
    path
});

pub fn assets_dir() -> &'static Path {
    &ASSETS_DIR
}
