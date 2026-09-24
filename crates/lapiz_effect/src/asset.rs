//! Persistent effect definitions. Pass ports live in graph nodes, not in this asset.

use std::io::{self, Read, Write};

use lapiz_assets::{asset::Asset, loader::AssetSerializer};
use lapiz_shader_graph::save::SerializableGraph;
use lapiz_utils::wrapper;
use serde::{Deserialize, Serialize};
use toml::{de, ser};
use uuid::Uuid;

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub EffectPassId : Uuid
}

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub EffectInputSlotId : Uuid
}

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub EffectOutputSlotId : Uuid
}

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub EffectPassInputSlotId : Uuid
}

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub EffectPassOutputSlotId : Uuid
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EffectAsset {
    pub name: String,
    pub passes: Vec<SerializableEffectPass>,
    pub inputs: Vec<SerializableEffectInputSlot>,
    pub outputs: Vec<SerializableEffectOutputSlot>,
}

impl Asset for EffectAsset {
    const TYPE_NAME: &'static str = "effect";
}

#[derive(Default)]
pub struct EffectAssetSerializer;

#[derive(Debug, thiserror::Error)]
pub enum EffectAssetSerializerError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    TomlDe(#[from] de::Error),
    #[error(transparent)]
    TomlSer(#[from] ser::Error),
}

impl AssetSerializer for EffectAssetSerializer {
    type Asset = EffectAsset;
    type Error = EffectAssetSerializerError;

    fn file_extension() -> &'static str {
        "lef"
    }

    fn read(&self, reader: &mut dyn Read) -> Result<Self::Asset, Self::Error> {
        let mut source = String::new();
        reader.read_to_string(&mut source)?;
        Ok(toml::from_str(&source)?)
    }

    fn write(&self, asset: &Self::Asset, writer: &mut dyn Write) -> Result<(), Self::Error> {
        writer.write_all(toml::to_string(asset)?.as_bytes())?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SerializableEffectPass {
    pub id: EffectPassId,
    pub name: String,
    pub graph: SerializableGraph,
    pub dispatch_strategy: EffectPassDispatchStrategy,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SerializableEffectInputSlot {
    pub name: String,
    pub id: EffectInputSlotId,
    pub ty: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SerializableEffectOutputSlot {
    pub name: String,
    pub id: EffectOutputSlotId,
    pub ty: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectPassDispatchStrategy {
    Once,
    EveryBufferElement(EffectPassInputSlotId),
    EveryOutputLayerPixel(EffectPassOutputSlotId),
    EveryInputLayerPixel(EffectPassInputSlotId),
}
