//! Node state is the source of truth for pass ports. Names are display-only.
use std::sync::{Arc, LazyLock};

use anyhow::{Context, Result};
use iced_core::widget::Void;
use lapiz_image::texel::TexelType;
use lapiz_shader_graph::{
    GraphElement,
    graph::{
        GraphResources,
        node::{
            GraphNode, GraphNodeCodeGenContext, GraphNodeCodeGenError, GraphNodeCreateSlotsContext,
            GraphNodeDefaultStateContext, GraphNodeRegistry, GraphNodeUpdateContext,
            GraphNodeViewContext,
        },
        slot::{ErasedGraphValueType, GraphDefaultInputSlot, GraphDefaultOutputSlot},
        variable::GraphTypeRegistry,
    },
    save::GraphSerializable,
    wgsl_std::{
        builtin_nodes, builtin_types,
        types::{ArrayType, BoolType, LayerType, TextureType, U32Type, Vec2IType},
    },
};
use lapiz_utils::random_oklch_hue_chroma;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wesl::syntax::*;
use wesl_quote::quote_statement;

use crate::asset::*;

#[derive(Default, Clone)]
pub struct PassInputNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassInput {
    Pass(EffectPassOutputSlotId),
    Effect(EffectInputSlotId),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PassInputNodeState {
    pub id: EffectPassInputSlotId,
    pub input: Option<PassInput>,

    #[serde(skip)]
    pub cached_ty: Option<Arc<dyn ErasedGraphValueType>>,
}

// UI source/target selectors arrive with the editor adapter; nothing can move
// these states through the graph until then.
#[derive(Clone)]
pub enum PassInputNodeMessage {}

impl GraphNode for PassInputNode {
    type State = PassInputNodeState;
    type Message = PassInputNodeMessage;

    fn id(&self) -> &'static str {
        "pass_input_node"
    }

    fn default_state(&self, _: GraphNodeDefaultStateContext<'_>) -> Self::State {
        Self::State {
            id: EffectPassInputSlotId::new(Uuid::new_v4()),
            input: None,
            cached_ty: None,
        }
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(PassInputNode)
    }

    fn create_inputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(
        &self,
        state: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        match &state.cached_ty {
            None => vec![],
            Some(ty) => ty.push_output_slots(),
        }
    }

    fn view<'a>(
        &self,
        _: &'a Self::State,
        _: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, Self::Message> {
        // TODO editor
        todo!()
    }

    fn update(&self, _: &mut Self::State, message: Self::Message, _: GraphNodeUpdateContext<'_>) {
        match message {}
    }

    fn generate_code(
        &self,
        state: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let Some(ty) = &state.cached_ty else {
            return Ok(String::new());
        };
        let name = format!("pass_input_{}", state.id.0.simple());

        Ok(ty.handle_input_values(&name, 0, &mut ctx)?)
    }
}

#[derive(Default, Clone)]
pub struct PassOutputNode;

#[derive(Clone)]
pub struct PassOutputDef {
    pub name: String,
    pub ty: Arc<dyn ErasedGraphValueType>,
}

#[derive(Serialize, Deserialize)]
struct SerializablePassOutputDef {
    name: String,
    ty: String,
}

impl GraphSerializable for PassOutputDef {
    fn to_toml(&self) -> Result<toml::Value> {
        let serializable = SerializablePassOutputDef {
            name: self.name.clone(),
            ty: self.ty.id().id,
        };
        serializable.to_toml()
    }

    fn from_toml(value: toml::Value, resources: &GraphResources) -> Result<Self> {
        let serializable = SerializablePassOutputDef::from_toml(value, resources)?;
        Ok(PassOutputDef {
            name: serializable.name,
            ty: resources
                .type_registry
                .resolve_type(&serializable.ty)
                .with_context(|| format!("Unknown pass output type '{}'", serializable.ty))?
                .clone(),
        })
    }
}

#[derive(Clone)]
pub enum PassOutput {
    Pass(PassOutputDef),
    Effect(EffectOutputSlotId),
}

pub struct PassOutputNodeState {
    pub id: EffectPassOutputSlotId,
    pub output: Option<PassOutput>,

    pub cached_ty: Option<Arc<dyn ErasedGraphValueType>>,
}

#[derive(Serialize, Deserialize)]
enum SerializablePassOutput {
    Pass(SerializablePassOutputDef),
    Effect(EffectOutputSlotId),
}

#[derive(Serialize, Deserialize)]
struct SerializablePassOutputNodeState {
    id: EffectPassOutputSlotId,
    output: Option<SerializablePassOutput>,
}

impl GraphSerializable for PassOutputNodeState {
    fn to_toml(&self) -> Result<toml::Value> {
        SerializablePassOutputNodeState {
            id: self.id,
            output: self.output.as_ref().map(|output| match output {
                PassOutput::Pass(def) => SerializablePassOutput::Pass(SerializablePassOutputDef {
                    name: def.name.clone(),
                    ty: def.ty.id().id,
                }),
                PassOutput::Effect(id) => SerializablePassOutput::Effect(*id),
            }),
        }
        .to_toml()
    }

    fn from_toml(value: toml::Value, resources: &GraphResources) -> Result<Self> {
        let serializable = SerializablePassOutputNodeState::from_toml(value, resources)?;
        let output = serializable
            .output
            .map(|output| -> Result<PassOutput> {
                match output {
                    SerializablePassOutput::Pass(def) => Ok(PassOutput::Pass(PassOutputDef {
                        name: def.name,
                        ty: resources
                            .type_registry
                            .resolve_type(&def.ty)
                            .with_context(|| format!("Unknown pass output type '{}'", def.ty))?
                            .clone(),
                    })),
                    SerializablePassOutput::Effect(id) => Ok(PassOutput::Effect(id)),
                }
            })
            .transpose()?;
        Ok(Self {
            id: serializable.id,
            output,
            cached_ty: None,
        })
    }
}

#[derive(Clone)]
pub enum PassOutputNodeMessage {}

impl GraphNode for PassOutputNode {
    type State = PassOutputNodeState;
    type Message = PassOutputNodeMessage;

    fn id(&self) -> &'static str {
        "pass_output_node"
    }

    fn default_state(&self, _: GraphNodeDefaultStateContext<'_>) -> Self::State {
        Self::State {
            id: EffectPassOutputSlotId::new(Uuid::new_v4()),
            output: None,
            cached_ty: None,
        }
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(PassOutputNode)
    }

    fn create_inputs(
        &self,
        state: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        match &state.output {
            None => vec![],
            Some(PassOutput::Pass(def)) => def.ty.push_input_slots(),
            Some(PassOutput::Effect(_)) => state
                .cached_ty
                .as_ref()
                .map_or_else(Vec::new, |ty| ty.push_input_slots()),
        }
    }

    fn create_outputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        vec![]
    }

    fn view<'a>(
        &self,
        _: &'a Self::State,
        _: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, Self::Message> {
        // TODO editor
        todo!()
    }

    fn update(&self, _: &mut Self::State, message: Self::Message, _: GraphNodeUpdateContext<'_>) {
        match message {}
    }

    fn generate_code(
        &self,
        state: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let Some(output) = &state.output else {
            return Ok(String::new());
        };
        let ty = match output {
            PassOutput::Pass(def) => def.ty.clone(),
            PassOutput::Effect(_) => state
                .cached_ty
                .clone()
                .context("Effect output type has not been synchronized")?,
        };
        let name = format!("pass_output_{}", state.id.0.simple());

        Ok(ty.handle_output_values(&name, 0, &mut ctx)?)
    }
}

impl PassOutputNodeState {
    pub fn new(output: PassOutput) -> Self {
        Self {
            id: EffectPassOutputSlotId::new(Uuid::new_v4()),
            output: Some(output),
            cached_ty: None,
        }
    }
}

// Outputs current dispatch strategy, for once, it outputs nothing, for every element in
// buffer, it outputs u32 index, for layer pixels, it outputs pixel vec2i index
#[derive(Default, Clone)]
pub struct DispatchIndexNode;

#[derive(Clone, Serialize, Deserialize)]
pub struct DispatchIndexNodeState {
    pub cached_dispatch_strategy: EffectPassDispatchStrategy,
}

#[derive(Clone)]
pub enum DispatchIndexNodeMessage {}

impl GraphNode for DispatchIndexNode {
    type State = DispatchIndexNodeState;
    type Message = DispatchIndexNodeMessage;

    fn id(&self) -> &'static str {
        "dispatch_index_node"
    }

    fn default_state(&self, _: GraphNodeDefaultStateContext<'_>) -> Self::State {
        DispatchIndexNodeState {
            cached_dispatch_strategy: EffectPassDispatchStrategy::Once,
        }
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(DispatchIndexNode)
    }

    fn create_inputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(
        &self,
        state: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        match state.cached_dispatch_strategy {
            EffectPassDispatchStrategy::Once => vec![],
            EffectPassDispatchStrategy::EveryBufferElement(_) => {
                vec![GraphDefaultOutputSlot::new::<U32Type>("index".into())]
            }
            EffectPassDispatchStrategy::EveryOutputLayerPixel(_)
            | EffectPassDispatchStrategy::EveryInputLayerPixel(_) => {
                vec![GraphDefaultOutputSlot::new::<Vec2IType>(
                    "pixel_position".into(),
                )]
            }
        }
    }

    fn view<'a>(
        &self,
        _: &'a Self::State,
        _: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, Self::Message> {
        Void.into()
    }

    fn update(&self, _: &mut Self::State, message: Self::Message, _: GraphNodeUpdateContext<'_>) {
        match message {}
    }

    fn generate_code(
        &self,
        _: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let output = Ident::new(ctx.get_output(0)?);

        Ok(format!(
            "{}\n",
            // Dispatch index is passed into graph() as a function param, and its type is dynamically aliased to
            // correct one.
            quote_statement! {
                let #output = dispatch_index;
            }
            .to_string()
        ))
    }
}

pub static EFFECT_GRAPH_TYPES: LazyLock<Arc<GraphTypeRegistry>> =
    LazyLock::new(|| Arc::new(effect_graph_types()));
pub static EFFECT_GRAPH_NODES: LazyLock<Arc<GraphNodeRegistry>> =
    LazyLock::new(|| Arc::new(effect_nodes()));

fn effect_graph_types() -> GraphTypeRegistry {
    let mut types = GraphTypeRegistry::default();
    types.merge(builtin_types());
    types
}

pub fn effect_nodes() -> GraphNodeRegistry {
    let mut nodes = GraphNodeRegistry::with_capacity();
    nodes.merge(builtin_nodes());
    nodes.register::<PassInputNode>();
    nodes.register::<PassOutputNode>();
    nodes.register::<DispatchIndexNode>();
    nodes
}
