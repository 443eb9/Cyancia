//! Persistent effect definitions. Pass ports live in graph nodes, not in this asset.

use lapiz_shader_graph::save::SerializableGraph;
use lapiz_utils::wrapper;
use serde::{Deserialize, Serialize};
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

// SerializableGraph does not implement Debug/Eq/Hash.
#[derive(Clone, Serialize, Deserialize)]
pub struct EffectAsset {
    pub name: String,
    pub passes: Vec<SerializableEffectPass>,
    pub inputs: Vec<SerializableEffectInputSlot>,
    pub outputs: Vec<SerializableEffectOutputSlot>,
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
    /// The array exposed by a PassInputNode in this pass.
    EveryBufferElement(EffectPassInputSlotId),
    /// A Layer defined by a PassOutputNode in this pass, after bounds evaluation.
    /// This is an effect port ID, not the graph input carrying its color/bounds.
    EveryOutputLayerPixel(EffectPassOutputSlotId),
    /// A Layer exposed by a PassInputNode in this pass.
    EveryInputLayerPixel(EffectPassInputSlotId),
}
