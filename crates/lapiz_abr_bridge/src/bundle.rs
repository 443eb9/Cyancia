use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use lapiz_assets::{
    asset::{ErasedAsset, UntypedAssetId},
    bundle::{AssetBundle, AssetBundleMetadata, BundleManifest},
    loader::ErasedAssetSerializer,
    tag::{AssetTags, TagFile},
};
use thiserror::Error;
use tracing::info;

pub struct AbrAssetBundle {
    path: PathBuf,
    assets: HashMap<PathBuf, Arc<dyn ErasedAsset>>,
}

impl AbrAssetBundle {
    pub fn parse(root: impl AsRef<Path>) -> anyhow::Result<Self> {
        info!("parse {}", root.as_ref().display());
        Err(anyhow::anyhow!(""))
    }

    pub fn scan_bundles(root: impl AsRef<Path>) -> (Vec<Self>, Vec<AbrAssetBundleError>) {
        let mut bundles = Vec::new();
        let mut errors = Vec::new();
        scan_bundles(root, &mut bundles, &mut errors);
        (bundles, errors)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn scan_bundles(
    root: impl AsRef<Path>,
    bundles: &mut Vec<AbrAssetBundle>,
    errors: &mut Vec<AbrAssetBundleError>,
) {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) => {
            errors.push(AbrAssetBundleError::Io(e));
            return;
        }
    };

    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };

        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|ext| ext.to_str());
            if ext == Some("abr") {
                match AbrAssetBundle::parse(&path) {
                    Ok(bundle) => bundles.push(bundle),
                    Err(e) => errors.push(AbrAssetBundleError::Parse(e)),
                }
            }
        } else if path.is_dir() {
            scan_bundles(path, bundles, errors);
        }
    }
}

#[derive(Debug, Error)]
pub enum AbrAssetBundleError {
    #[error("Unsupported writing to standard asset bundle")]
    UnsupportedWriting,
    #[error("Asset not found: {0}")]
    AssetNotFound(PathBuf),
    #[error("Tag not found: {0}")]
    TagNotFound(PathBuf),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(anyhow::Error),
}

impl AssetBundle for AbrAssetBundle {
    const READONLY: bool = true;

    type Error = AbrAssetBundleError;

    fn metadata(&self) -> Result<AssetBundleMetadata, Self::Error> {
        todo!()
    }

    fn manifest(&self) -> Result<BundleManifest, Self::Error> {
        todo!()
    }

    fn read_asset(
        &self,
        path: &Path,
        serializer: &dyn ErasedAssetSerializer,
    ) -> Result<Arc<dyn ErasedAsset>, Self::Error> {
        self.assets
            .get(path)
            .cloned()
            .ok_or_else(|| AbrAssetBundleError::AssetNotFound(path.to_path_buf()))
    }

    fn add_asset(
        &self,
        path: &Path,
        asset: &dyn ErasedAsset,
        serializer: &dyn ErasedAssetSerializer,
    ) -> Result<UntypedAssetId, Self::Error> {
        Err(AbrAssetBundleError::UnsupportedWriting)
    }

    fn read_tag(&self, tag: &Path) -> Result<TagFile, Self::Error> {
        Err(AbrAssetBundleError::TagNotFound(tag.to_path_buf()))
    }

    fn add_tag(&self, path: &Path, tag: &TagFile) -> Result<(), Self::Error> {
        Err(AbrAssetBundleError::UnsupportedWriting)
    }

    // TODO every asset has a same tag which is the bundle name
    fn read_asset_tags(&self, path: &Path) -> Result<Option<AssetTags>, Self::Error> {
        Ok(None)
    }

    fn write_asset_tags(&self, path: &Path, tags: &AssetTags) -> Result<(), Self::Error> {
        Err(AbrAssetBundleError::UnsupportedWriting)
    }
}
