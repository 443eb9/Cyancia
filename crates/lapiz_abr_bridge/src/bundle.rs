use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::DateTime;
use lapiz_abr::Abr;
use lapiz_assets::{
    asset::{ErasedAsset, UntypedAssetId},
    bundle::{AssetBundle, AssetBundleMetadata, BundleId, BundleManifest},
    loader::ErasedAssetSerializer,
    tag::{AssetTags, TagFile},
};
use thiserror::Error;
use tracing::{info, trace};
use uuid::Uuid;

pub struct AbrAssetBundle {
    path: PathBuf,
    metadata: AssetBundleMetadata,
    manifest: BundleManifest,
    assets: HashMap<PathBuf, Arc<dyn ErasedAsset>>,
}

impl AbrAssetBundle {
    pub fn parse(path: impl AsRef<Path>) -> anyhow::Result<(Self, Vec<anyhow::Error>)> {
        let mut bytes = Vec::new();
        let path = path.as_ref();
        let mut file = File::open(path)?;
        file.read_to_end(&mut bytes)?;
        let abr = Abr::parse(&bytes)?;
        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        let metadata = AssetBundleMetadata {
            bundle_id: BundleId::new(Uuid::from_u128(xxhash_rust::xxh3::xxh3_128(
                name.as_bytes(),
            ))),
            name,
            last_modified: file.metadata()?.modified()?.into(),
        };

        let mut assets = HashMap::with_capacity(abr.samples.len() + abr.patterns.len());
        let mut asset_manifest = BTreeMap::new();
        let mut errs = Vec::new();

        let mut n_samples = 0;
        let mut n_patterns = 0;

        for raw in abr.samples {
            match crate::samp::parse_samp(&raw) {
                Ok(asset) => {
                    let name = format!("samp-{}.lig", raw.id);
                    let path = PathBuf::new().join(&name);
                    asset_manifest.insert(
                        UntypedAssetId::new(Uuid::new_v5(&metadata.bundle_id, name.as_bytes())),
                        path.clone(),
                    );
                    assets.insert(path, Arc::new(asset) as _);
                    trace!("Loaded sample image {}.", raw.id);
                    n_samples += 1;
                }
                Err(err) => errs.push(err),
            }
        }

        for raw in abr.patterns {
            match crate::patt::parse_patt(&raw) {
                Ok(asset) => {
                    let name = format!("patt-{}.lig", raw.id);
                    let path = PathBuf::new().join(&name);
                    asset_manifest.insert(
                        UntypedAssetId::new(Uuid::new_v5(&metadata.bundle_id, name.as_bytes())),
                        path.clone(),
                    );
                    assets.insert(path, Arc::new(asset) as _);
                    trace!("Loaded pattern image {}.", raw.id);
                    n_patterns += 1;
                }
                Err(err) => errs.push(err),
            }
        }

        let manifest = BundleManifest {
            assets: asset_manifest,
            tags: BTreeMap::new(),
        };

        trace!("Loaded {} samples and {} patterns.", n_samples, n_patterns);

        Ok((
            Self {
                path: path.to_path_buf(),
                metadata,
                manifest,
                assets,
            },
            errs,
        ))
    }

    pub fn scan_bundles(
        root: impl AsRef<Path>,
    ) -> (Vec<Self>, Vec<(PathBuf, AbrAssetBundleError)>) {
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
    errors: &mut Vec<(PathBuf, AbrAssetBundleError)>,
) {
    let root = root.as_ref();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) => {
            errors.push((root.to_path_buf(), AbrAssetBundleError::Io(e)));
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
                    Ok((bundle, err)) => {
                        bundles.push(bundle);
                        if !err.is_empty() {
                            errors.push((path.to_path_buf(), AbrAssetBundleError::Parse(err)));
                        }
                    }
                    Err(e) => errors.push((path.to_path_buf(), AbrAssetBundleError::Open(e))),
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
    #[error("Open error: {0}")]
    Open(anyhow::Error),
    #[error("Parse error: {0:?}")]
    Parse(Vec<anyhow::Error>),
}

impl AssetBundle for AbrAssetBundle {
    const READONLY: bool = true;

    type Error = AbrAssetBundleError;

    fn metadata(&self) -> Result<AssetBundleMetadata, Self::Error> {
        Ok(self.metadata.clone())
    }

    fn manifest(&self) -> Result<BundleManifest, Self::Error> {
        Ok(self.manifest.clone())
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
