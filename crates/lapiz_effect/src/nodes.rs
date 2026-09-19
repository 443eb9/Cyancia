//! Node state is the source of truth for pass ports. Names are display-only.
use std::sync::{Arc, LazyLock};

use anyhow::{Context, Result};
use iced_core::{Length, widget::Void};
use iced_widget::{Column, column, row};
use lapiz_i18n::t;
use lapiz_shader_graph::{
    GraphElement,
    graph::{
        GraphResources,
        node::{
            GraphNode, GraphNodeCodeGenContext, GraphNodeCodeGenError, GraphNodeCreateSlotsContext,
            GraphNodeDefaultStateContext, GraphNodeRegistry, GraphNodeUpdateContext,
            GraphNodeViewContext,
        },
        slot::{
            ErasedGraphLiteralUpdateMessage, ErasedGraphValueType, GraphDefaultInputSlot,
            GraphDefaultOutputSlot,
        },
        variable::GraphTypeRegistry,
    },
    save::GraphSerializable,
    wgsl_std::{
        builtin_nodes, builtin_types,
        types::{U32Type, Vec2IType},
    },
};
use lapiz_utils::random_oklch_hue_chroma;
use lapiz_widgets::{combo_box::ComboBox, label::Label, text_input::TextInput};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wesl::syntax::*;
use wesl_quote::quote_statement;

use crate::{
    asset::*,
    render::{pass_input_ident, pass_output_ident},
};

#[derive(Default, Clone)]
pub struct PassInputNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassInput {
    Pass(EffectPassOutputSlotId),
    Effect(EffectInputSlotId),
}

#[derive(Clone, PartialEq)]
pub struct PassInputChoice {
    pub label: String,
    pub input: Option<PassInput>,
}

impl std::fmt::Debug for PassInputChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassInputChoice")
            .field("label", &self.label)
            .finish()
    }
}

impl std::fmt::Display for PassInputChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassOutputChoiceTarget {
    Unbound,
    Effect(EffectOutputSlotId),
    LocalBuffer,
}

#[derive(Clone, PartialEq)]
pub struct PassOutputChoice {
    pub label: String,
    pub target: PassOutputChoiceTarget,
}

impl std::fmt::Debug for PassOutputChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassOutputChoice")
            .field("label", &self.label)
            .field("target", &self.target)
            .finish()
    }
}

impl std::fmt::Display for PassOutputChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Clone)]
pub struct TypeChoice {
    pub label: String,
    pub ty: Arc<dyn ErasedGraphValueType>,
}

impl std::fmt::Debug for TypeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeChoice")
            .field("label", &self.label)
            .finish()
    }
}

impl PartialEq for TypeChoice {
    fn eq(&self, other: &Self) -> bool {
        self.ty.id() == other.ty.id()
    }
}

impl std::fmt::Display for TypeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

#[derive(Clone)]
pub struct PassInputNodeState {
    pub id: EffectPassInputSlotId,
    pub input: Option<PassInput>,
    pub cached_ty: Option<Arc<dyn ErasedGraphValueType>>,
    // Refreshed by EffectInstance::sync_pass_graph_effect_properties, not serialized.
    pub available_sources: Arc<[PassInputChoice]>,
}

#[derive(Serialize, Deserialize)]
struct SerializablePassInputNodeState {
    id: EffectPassInputSlotId,
    input: Option<PassInput>,
    ty: Option<String>,
}

impl GraphSerializable for PassInputNodeState {
    fn to_toml(&self) -> Result<toml::Value> {
        SerializablePassInputNodeState {
            id: self.id,
            input: self.input,
            ty: self.cached_ty.as_ref().map(|ty| ty.id().id),
        }
        .to_toml()
    }

    fn from_toml(value: toml::Value, resources: &GraphResources) -> Result<Self> {
        let serializable = SerializablePassInputNodeState::from_toml(value, resources)?;
        let cached_ty = serializable
            .ty
            .map(|ty| {
                resources
                    .type_registry
                    .resolve_type(&ty)
                    .with_context(|| format!("Unknown pass input type '{ty}'"))
            })
            .transpose()?;
        Ok(Self {
            id: serializable.id,
            input: serializable.input,
            cached_ty,
            available_sources: Vec::new().into(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum PassInputNodeMessage {
    SourceSelected(PassInputChoice),
}

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
            available_sources: Vec::new().into(),
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
        state: &'a Self::State,
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, Self::Message> {
        let selected = state
            .available_sources
            .iter()
            .find(|choice| choice.input == state.input)
            .cloned();
        let selector = ComboBox::new(
            state.available_sources.iter().cloned().collect::<Vec<_>>(),
            selected,
            PassInputNodeMessage::SourceSelected,
        )
        .placeholder(t!("unbound"))
        .width(Length::Fill);

        column![
            selector,
            Column::with_children(ctx.view_all_outputs()).spacing(2),
        ]
        .spacing(4)
        .width(Length::Fill)
        .into()
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        _: GraphNodeUpdateContext<'_>,
    ) {
        match message {
            PassInputNodeMessage::SourceSelected(choice) => state.input = choice.input,
        }
    }

    fn generate_code(
        &self,
        state: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let Some(ty) = &state.cached_ty else {
            return Ok(String::new());
        };
        let name = pass_input_ident(state.id);

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
    // Refreshed by EffectInstance::sync_pass_graph_effect_properties, not serialized.
    pub available_targets: Arc<[PassOutputChoice]>,
    pub available_types: Arc<[TypeChoice]>,
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
    ty: Option<String>,
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
            ty: self.cached_ty.as_ref().map(|ty| ty.id().id),
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
        let cached_ty = serializable
            .ty
            .map(|ty| {
                resources
                    .type_registry
                    .resolve_type(&ty)
                    .with_context(|| format!("Unknown pass output type '{ty}'"))
            })
            .transpose()?;
        Ok(Self {
            id: serializable.id,
            output,
            cached_ty,
            available_targets: Vec::new().into(),
            available_types: Vec::new().into(),
        })
    }
}

#[derive(Debug, Clone)]
pub enum PassOutputNodeMessage {
    TargetSelected(PassOutputChoice),
    LocalNameChanged(String),
    LocalTypeSelected(TypeChoice),
    LiteralUpdate(ErasedGraphLiteralUpdateMessage),
}

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
            available_targets: Vec::new().into(),
            available_types: Vec::new().into(),
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
        state: &'a Self::State,
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, Self::Message> {
        let selected = state
            .available_targets
            .iter()
            .find(|choice| match (&state.output, choice.target) {
                (None, PassOutputChoiceTarget::Unbound) => true,
                (Some(PassOutput::Effect(id)), PassOutputChoiceTarget::Effect(other)) => {
                    *id == other
                }
                (Some(PassOutput::Pass(_)), PassOutputChoiceTarget::LocalBuffer) => true,
                _ => false,
            })
            .cloned();
        let selector = ComboBox::new(
            state.available_targets.iter().cloned().collect::<Vec<_>>(),
            selected,
            PassOutputNodeMessage::TargetSelected,
        )
        .placeholder(t!("unbound"))
        .width(Length::Fill);

        let local_editor = match &state.output {
            Some(PassOutput::Pass(def)) => {
                let selected_type = state
                    .available_types
                    .iter()
                    .find(|choice| choice.ty.id() == def.ty.id())
                    .cloned();
                Some(
                    column![
                        row![
                            Label::new(t!("name")),
                            TextInput::new("", &def.name)
                                .on_input(PassOutputNodeMessage::LocalNameChanged)
                                .width(Length::Fill),
                        ]
                        .spacing(4),
                        ComboBox::new(
                            state.available_types.iter().cloned().collect::<Vec<_>>(),
                            selected_type,
                            PassOutputNodeMessage::LocalTypeSelected,
                        )
                        .placeholder(t!("type"))
                        .width(Length::Fill),
                    ]
                    .spacing(2),
                )
            }
            _ => None,
        };

        let content = column![selector].spacing(4).width(Length::Fill);
        let content = match local_editor {
            Some(editor) => content.push(editor),
            None => content,
        };
        content
            .push(Column::with_children(
                ctx.view_all_inputs(PassOutputNodeMessage::LiteralUpdate),
            ))
            .into()
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        mut ctx: GraphNodeUpdateContext<'_>,
    ) {
        match message {
            PassOutputNodeMessage::TargetSelected(choice) => {
                state.output = match choice.target {
                    PassOutputChoiceTarget::Unbound => None,
                    PassOutputChoiceTarget::Effect(id) => Some(PassOutput::Effect(id)),
                    PassOutputChoiceTarget::LocalBuffer => match state.output.take() {
                        Some(PassOutput::Pass(def)) => Some(PassOutput::Pass(def)),
                        _ => state.available_types.first().map(|choice| {
                            PassOutput::Pass(PassOutputDef {
                                name: "buffer".into(),
                                ty: choice.ty.clone(),
                            })
                        }),
                    },
                }
            }
            PassOutputNodeMessage::LocalNameChanged(name) => {
                if let Some(PassOutput::Pass(def)) = &mut state.output {
                    def.name = name;
                }
            }
            PassOutputNodeMessage::LocalTypeSelected(choice) => {
                if let Some(PassOutput::Pass(def)) = &mut state.output {
                    def.ty = choice.ty.clone();
                }
            }
            PassOutputNodeMessage::LiteralUpdate(message) => ctx.update_literal(message),
        }
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
        let name = pass_output_ident(state.id);

        Ok(ty.handle_output_values(&name, 0, &mut ctx)?)
    }
}

impl PassOutputNodeState {
    pub fn new(output: PassOutput) -> Self {
        Self {
            id: EffectPassOutputSlotId::new(Uuid::new_v4()),
            output: Some(output),
            cached_ty: None,
            available_targets: Vec::new().into(),
            available_types: Vec::new().into(),
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
        let output = ctx.get_output(0)?;

        Ok(format!(
            "{}\n",
            // Dispatch index is passed into graph() as a function param, and its type is dynamically aliased to
            // correct one.
            quote_statement! {
                let #output = dispatch_index;
            }
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
