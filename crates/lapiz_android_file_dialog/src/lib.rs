use std::path::{Path, PathBuf};

use anyhow::Result;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
pub struct AndroidFileDialog {
    app: winit::platform::android::activity::AndroidApp,
}

#[cfg(target_os = "android")]
impl lapiz_runtime::service::Service for AndroidFileDialog {}

#[cfg(target_os = "android")]
impl AndroidFileDialog {
    pub fn new(app: winit::platform::android::activity::AndroidApp) -> Self {
        Self { app }
    }

    pub fn app(&self) -> &winit::platform::android::activity::AndroidApp {
        &self.app
    }
}

pub struct LocalFile {
    path: PathBuf,
    #[cfg(target_os = "android")]
    uri: Option<String>,
    #[cfg(target_os = "android")]
    app: Option<winit::platform::android::activity::AndroidApp>,
    #[cfg(target_os = "android")]
    _temporary: Option<tempfile::TempDir>,
}

impl LocalFile {
    pub fn from_path(path: PathBuf) -> Self {
        Self {
            path,
            #[cfg(target_os = "android")]
            uri: None,
            #[cfg(target_os = "android")]
            app: None,
            #[cfg(target_os = "android")]
            _temporary: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_temporary(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            self._temporary.is_some()
        }
        #[cfg(not(target_os = "android"))]
        {
            false
        }
    }

    pub fn for_save(&self) -> Result<Self> {
        #[cfg(target_os = "android")]
        if let Some(uri) = &self.uri {
            let name = self.path.file_name().unwrap_or_default().to_string_lossy();
            return android::destination(self.app.as_ref().unwrap().clone(), uri.clone(), &name);
        }
        Ok(Self::from_path(self.path.clone()))
    }

    pub fn sync(&self) -> Result<()> {
        #[cfg(target_os = "android")]
        if let Some(uri) = &self.uri {
            return android::write_file(self.app.as_ref().unwrap(), uri, &self.path);
        }
        Ok(())
    }
}

#[cfg(target_os = "android")]
pub async fn open_file(app: winit::platform::android::activity::AndroidApp) -> Result<Option<LocalFile>> {
    let Some(document) = android::pick(&app, false, "").await? else {
        return Ok(None);
    };
    android::open(app, document).map(Some)
}

#[cfg(target_os = "android")]
pub async fn create_file(
    app: winit::platform::android::activity::AndroidApp,
    name: &str,
) -> Result<Option<LocalFile>> {
    let Some(document) = android::pick(&app, true, name).await? else {
        return Ok(None);
    };
    android::destination(app, document.uri, document.name.as_ref()).map(Some)
}
