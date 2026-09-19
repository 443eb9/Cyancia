use std::io::{Cursor, Read, Write as _};

use indexmap::IndexMap;
use lapiz_assets::{asset::Asset, loader::AssetSerializer};
use lapiz_effect::asset::{EffectAsset, EffectAssetSerializer, EffectInputSlotId};
use lapiz_shader_graph::save::SerializableGraphLiteral;
use serde::{Deserialize, Serialize};
use zip::{ZipArchive, ZipWriter, write::FileOptions};

pub struct BrushPreset {
    pub metadata: BrushPresetMetadata,
    pub spacing_effect: EffectAsset,
    pub main_effect: EffectAsset,
    pub postprocess_effect: EffectAsset,
    pub parameters: IndexMap<EffectInputSlotId, SerializableBrushParameter>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableBrushParameter {
    pub name: String,
    pub value: SerializableGraphLiteral,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BrushPresetMetadata {
    pub name: String,
}

impl Asset for BrushPreset {
    const TYPE_NAME: &'static str = "brush_preset";
}

#[derive(Serialize, Deserialize)]
struct BrushToml {
    name: String,
}

#[derive(Default)]
pub struct BrushPresetSerializer;

#[derive(Debug, thiserror::Error)]
pub enum BrushPresetSerializerError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),
    #[error(transparent)]
    TomlSer(#[from] toml::ser::Error),
    #[error(transparent)]
    Effect(#[from] lapiz_effect::asset::EffectAssetSerializerError),
    #[error("Invalid brush preset: {0}")]
    Invalid(String),
}

impl AssetSerializer for BrushPresetSerializer {
    type Asset = BrushPreset;

    type Error = BrushPresetSerializerError;

    fn file_extension() -> &'static str {
        "lapiz"
    }

    fn read(&self, reader: &mut dyn Read) -> Result<Self::Asset, Self::Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        let mut archive = ZipArchive::new(Cursor::new(buf.as_slice()))?;

        let mut brush_buffer = String::new();
        archive
            .by_name("brush.toml")?
            .read_to_string(&mut brush_buffer)?;
        let brush_toml = toml::from_str::<BrushToml>(&brush_buffer)?;

        let mut read_effect = |name: &str| {
            let mut buffer = String::new();
            archive.by_name(name)?.read_to_string(&mut buffer)?;
            Result::<_, BrushPresetSerializerError>::Ok(
                EffectAssetSerializer.read(&mut Cursor::new(buffer))?,
            )
        };

        let spacing_effect = read_effect("spacing.lef")?;
        let main_effect = read_effect("main.lef")?;
        let postprocess_effect = read_effect("postprocess.lef")?;

        let parameters = match archive.by_name("parameters.toml") {
            Ok(mut file) => {
                let mut parameters_buffer = String::new();
                file.read_to_string(&mut parameters_buffer)?;
                toml::from_str::<IndexMap<EffectInputSlotId, SerializableBrushParameter>>(
                    &parameters_buffer,
                )?
            }
            Err(_) => IndexMap::new(),
        };

        Ok(BrushPreset {
            metadata: BrushPresetMetadata {
                name: brush_toml.name,
            },
            spacing_effect,
            main_effect,
            postprocess_effect,
            parameters,
        })
    }

    fn write(
        &self,
        asset: &Self::Asset,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        let mut buf = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buf));

            zip.start_file("brush.toml", FileOptions::<()>::default())?;
            let toml_buffer = toml::to_string(&BrushToml {
                name: asset.metadata.name.clone(),
            })?;
            zip.write_all(toml_buffer.as_bytes())?;

            zip.start_file("spacing.lef", FileOptions::<()>::default())?;
            EffectAssetSerializer.write(&asset.spacing_effect, &mut zip)?;
            zip.start_file("main.lef", FileOptions::<()>::default())?;
            EffectAssetSerializer.write(&asset.main_effect, &mut zip)?;
            zip.start_file("postprocess.lef", FileOptions::<()>::default())?;
            EffectAssetSerializer.write(&asset.postprocess_effect, &mut zip)?;

            if !asset.parameters.is_empty() {
                zip.start_file("parameters.toml", FileOptions::<()>::default())?;
                let parameters_buffer = toml::to_string(&asset.parameters)?;
                zip.write_all(parameters_buffer.as_bytes())?;
            }

            zip.finish()?;
        }
        writer.write_all(&buf)?;

        Ok(())
    }
}
