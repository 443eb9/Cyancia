use std::path::{Path, PathBuf};

use anyhow::Result;
#[cfg(target_os = "android")]
use lapiz_runtime::android::AndroidApp;

#[cfg(target_os = "android")]
mod android;

pub struct LocalFile {
    path: PathBuf,
    name: String,
    #[cfg(target_os = "android")]
    app: AndroidApp,
    #[cfg(target_os = "android")]
    uri: String,
    #[cfg(target_os = "android")]
    _temp: tempfile::TempDir,
}

impl LocalFile {
    #[cfg(not(target_os = "android"))]
    pub fn native(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        LocalFile { path, name }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source(&self) -> String {
        #[cfg(target_os = "android")]
        {
            self.uri.clone()
        }
        #[cfg(not(target_os = "android"))]
        {
            self.path.to_string_lossy().into_owned()
        }
    }

    pub fn commit(&self) -> Result<()> {
        #[cfg(target_os = "android")]
        android::write_file(&self.app, &self.uri, &self.path)?;

        Ok(())
    }
}

#[cfg(target_os = "android")]
pub fn open_file(app: AndroidApp, uri: String, name: String) -> Result<LocalFile> {
    let (path, directory) = android::temp_file(&name)?;
    android::copy_file(&app, &uri, &path)?;
    Ok(LocalFile {
        path,
        name,
        app,
        uri,
        _temp: directory,
    })
}

#[cfg(target_os = "android")]
pub async fn pick_file(app: AndroidApp) -> Result<Option<LocalFile>> {
    let Some(document) = android::pick(&app, false, "").await? else {
        return Ok(None);
    };
    open_file(app, document.uri, document.name).map(Some)
}

#[cfg(target_os = "android")]
pub async fn create_file(app: AndroidApp, name: &str) -> Result<Option<LocalFile>> {
    let Some(document) = android::pick(&app, true, name).await? else {
        return Ok(None);
    };
    let (path, directory) = android::temp_file(&document.name)?;
    Ok(Some(LocalFile {
        path,
        name: document.name,
        app,
        uri: document.uri,
        _temp: directory,
    }))
}
