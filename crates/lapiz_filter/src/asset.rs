use std::io::{self, Cursor, Read, Write};

use indexmap::IndexMap;
use lapiz_assets::{asset::Asset, loader::AssetSerializer};
use lapiz_effect::asset::{
    EffectAsset, EffectAssetSerializer, EffectAssetSerializerError, EffectInputSlotId,
};
use lapiz_shader_graph::save::SerializableGraphLiteral;
use serde::{Deserialize, Serialize};
use toml::{de, ser};
use zip::{ZipArchive, result::ZipError, write::FileOptions};

pub struct FilterPreset {
    pub metadata: FilterPresetMetadata,
    pub effect: EffectAsset,
    pub parameters: IndexMap<EffectInputSlotId, SerializableFilterParameter>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SerializableFilterParameter {
    pub name: String,
    pub value: SerializableGraphLiteral,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FilterPresetMetadata {
    pub name: String,
}

impl Asset for FilterPreset {
    const TYPE_NAME: &'static str = "filter_preset";
}

#[derive(Serialize, Deserialize)]
struct FilterToml {
    name: String,
}

#[derive(Default)]
pub struct FilterPresetSerializer;

#[derive(Debug, thiserror::Error)]
pub enum FilterPresetSerializerError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Zip(#[from] ZipError),
    #[error(transparent)]
    TomlDe(#[from] de::Error),
    #[error(transparent)]
    TomlSer(#[from] ser::Error),
    #[error(transparent)]
    Effect(#[from] EffectAssetSerializerError),
    #[error("Invalid filter preset: {0}")]
    Invalid(String),
}

impl AssetSerializer for FilterPresetSerializer {
    type Asset = FilterPreset;

    type Error = FilterPresetSerializerError;

    fn file_extension() -> &'static str {
        "lfp"
    }

    fn read(&self, reader: &mut dyn Read) -> Result<Self::Asset, Self::Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        let mut archive = ZipArchive::new(Cursor::new(buf))?;

        let mut filter_buffer = String::new();
        archive
            .by_name("filter.toml")?
            .read_to_string(&mut filter_buffer)?;
        let filter_toml = toml::from_str::<FilterToml>(&filter_buffer)?;

        let mut effect_buffer = String::new();
        archive
            .by_name("effect.lef")?
            .read_to_string(&mut effect_buffer)?;
        let effect = EffectAssetSerializer.read(&mut Cursor::new(effect_buffer))?;

        let parameters = match archive.by_name("parameters.toml") {
            Ok(mut file) => {
                let mut parameters_buffer = String::new();
                file.read_to_string(&mut parameters_buffer)?;
                toml::from_str::<IndexMap<EffectInputSlotId, SerializableFilterParameter>>(
                    &parameters_buffer,
                )?
            }
            Err(_) => IndexMap::new(),
        };

        Ok(FilterPreset {
            metadata: FilterPresetMetadata {
                name: filter_toml.name,
            },
            effect,
            parameters,
        })
    }

    fn write(&self, asset: &Self::Asset, writer: &mut dyn Write) -> Result<(), Self::Error> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));

            zip.start_file("filter.toml", FileOptions::<()>::default())?;
            let toml_buffer = toml::to_string(&FilterToml {
                name: asset.metadata.name.clone(),
            })?;
            zip.write_all(toml_buffer.as_bytes())?;

            zip.start_file("effect.lef", FileOptions::<()>::default())?;
            EffectAssetSerializer.write(&asset.effect, &mut zip)?;

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
