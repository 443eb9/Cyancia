#![expect(
    clippy::pub_use,
    reason = "stateless attribute macro is exposed for graph node definitions"
)]

use std::{
    any::Any,
    collections::{BTreeMap, HashMap, hash_map::Entry},
    sync::Arc,
};

use anyhow::Result;
use downcast_rs::Downcast;
use dyn_clone::DynClone;
use iced_core::{Length, Point};
use iced_widget::Column;
use lapiz_i18n::t;
pub use lapiz_shader_graph_derive::stateless;
use lapiz_utils::{cloneable_any::ClonableAnySync, wrapper};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    GraphElement,
    editor::slot::{input_slot, output_slot},
    graph::{
        Graph, GraphResources, GraphSignature, GraphVarIdentGenerator,
        function::GraphFunctionStorage,
        slot::{
            ErasedGraphLiteralUpdateMessage, GraphDefaultInputSlot, GraphDefaultOutputSlot,
            GraphInputSlotData, GraphInputSlotId, GraphOutputSlotData, GraphOutputSlotId,
            GraphSlots,
        },
        texture::GraphTextureUsageRecorder,
        variable::{GraphLiteralValue, GraphVariable},
    },
    save::GraphSerializable,
};

wrapper! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub GraphNodeId : Uuid
}

pub trait GraphNode: Send + Sync + 'static + DynClone {
    type State: Send + Sync + 'static + GraphSerializable;
    type Message: Send + Sync + 'static + Clone;

    fn id(&self) -> &'static str;
    fn default_state(&self, ctx: GraphNodeDefaultStateContext<'_>) -> Self::State;
    fn header_hue_chroma(&self) -> (f32, f32);
    fn create_inputs(
        &self,
        state: &Self::State,
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot>;
    fn create_outputs(
        &self,
        state: &Self::State,
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot>;
    fn update_signature(&self, _: &Self::State, _: GraphNodeUpdateSignatureContext<'_>) {}
    fn view<'a>(
        &self,
        state: &'a Self::State,
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, Self::Message>;
    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        ctx: GraphNodeUpdateContext<'_>,
    );
    fn generate_code(
        &self,
        state: &Self::State,
        ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError>;
    fn serialize_state(&self, state: &Self::State) -> Result<toml::Value> {
        state.to_toml()
    }
    fn deserialize_state(
        &self,
        value: toml::Value,
        resources: &GraphResources,
    ) -> Result<Self::State> {
        Self::State::from_toml(value, resources)
    }
    fn subgraphs<'a>(&self, _state: &'a Self::State) -> Vec<&'a Graph> {
        Vec::new()
    }
    fn subgraphs_mut<'a>(&mut self, _state: &'a mut Self::State) -> Vec<&'a mut Graph> {
        Vec::new()
    }
}

#[derive(Clone)]
pub struct ErasedGraphNodeMessage {
    pub inner: Box<dyn ClonableAnySync>,
    pub id: GraphNodeId,
}

impl std::fmt::Debug for ErasedGraphNodeMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErasedGraphNodeMessage")
            .field("id", &self.id)
            .finish()
    }
}

pub trait ErasedGraphNode: Send + Sync + 'static + DynClone + Downcast {
    fn id(&self) -> &'static str;
    fn default_state(&self, ctx: GraphNodeDefaultStateContext<'_>) -> Box<dyn Any + Send + Sync>;
    fn header_hue_chroma(&self) -> (f32, f32);
    fn create_inputs(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot>;
    fn create_outputs(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot>;
    fn update_signature(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeUpdateSignatureContext<'_>,
    );
    fn view<'a>(
        &self,
        node_id: GraphNodeId,
        state: &'a (dyn Any + Send + Sync),
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, ErasedGraphNodeMessage>;
    fn update(
        &self,
        state: &mut (dyn Any + Send + Sync),
        message: ErasedGraphNodeMessage,
        ctx: GraphNodeUpdateContext<'_>,
    );
    fn generate_code(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError>;
    fn serialize_state(&self, state: &(dyn Any + Send + Sync)) -> Result<toml::Value>;
    fn deserialize_state(
        &self,
        value: toml::Value,
        resources: &GraphResources,
    ) -> Result<Box<dyn Any + Send + Sync>>;
    fn subgraphs<'a>(&self, state: &'a (dyn Any + Send + Sync)) -> Vec<&'a Graph>;
    fn subgraphs_mut<'a>(&mut self, state: &'a mut (dyn Any + Send + Sync)) -> Vec<&'a mut Graph>;
}

dyn_clone::clone_trait_object!(ErasedGraphNode);
downcast_rs::impl_downcast!(ErasedGraphNode);

impl<T: GraphNode> ErasedGraphNode for T {
    fn id(&self) -> &'static str {
        self.id()
    }

    fn default_state(&self, ctx: GraphNodeDefaultStateContext<'_>) -> Box<dyn Any + Send + Sync> {
        Box::new(self.default_state(ctx))
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        self.header_hue_chroma()
    }

    fn create_inputs(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        self.create_inputs(
            state
                .downcast_ref::<T::State>()
                .expect("failed to downcast graph node state"),
            ctx,
        )
    }

    fn create_outputs(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        self.create_outputs(
            state
                .downcast_ref::<T::State>()
                .expect("failed to downcast graph node state"),
            ctx,
        )
    }

    fn update_signature(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeUpdateSignatureContext<'_>,
    ) {
        self.update_signature(
            state
                .downcast_ref::<T::State>()
                .expect("failed to downcast graph node state"),
            ctx,
        );
    }

    fn view<'a>(
        &self,
        node_id: GraphNodeId,
        state: &'a (dyn Any + Send + Sync),
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, ErasedGraphNodeMessage> {
        self.view(
            state
                .downcast_ref::<T::State>()
                .expect("failed to downcast graph node state"),
            ctx,
        )
        .map(move |message| ErasedGraphNodeMessage {
            inner: Box::new(message),
            id: node_id,
        })
    }

    fn update(
        &self,
        state: &mut (dyn Any + Send + Sync),
        message: ErasedGraphNodeMessage,
        ctx: GraphNodeUpdateContext<'_>,
    ) {
        let state = state
            .downcast_mut::<T::State>()
            .expect("failed to downcast graph node state");
        let message = match message.inner.downcast::<T::Message>() {
            Ok(message) => message,
            Err(_) => panic!("failed to downcast graph node message"),
        };
        self.update(state, *message, ctx);
    }

    fn generate_code(
        &self,
        state: &(dyn Any + Send + Sync),
        ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        self.generate_code(
            state
                .downcast_ref::<T::State>()
                .expect("failed to downcast graph node state"),
            ctx,
        )
    }

    fn serialize_state(&self, state: &(dyn Any + Send + Sync)) -> Result<toml::Value> {
        self.serialize_state(
            state
                .downcast_ref::<T::State>()
                .expect("failed to downcast graph node state"),
        )
    }

    fn deserialize_state(
        &self,
        value: toml::Value,
        resources: &GraphResources,
    ) -> Result<Box<dyn Any + Send + Sync>> {
        Ok(Box::new(self.deserialize_state(value, resources)?))
    }

    fn subgraphs<'a>(&self, state: &'a (dyn Any + Send + Sync)) -> Vec<&'a Graph> {
        self.subgraphs(
            state
                .downcast_ref::<T::State>()
                .expect("failed to downcast graph node state"),
        )
    }

    fn subgraphs_mut<'a>(&mut self, state: &'a mut (dyn Any + Send + Sync)) -> Vec<&'a mut Graph> {
        self.subgraphs_mut(
            state
                .downcast_mut::<T::State>()
                .expect("failed to downcast graph node state"),
        )
    }
}

pub struct StatefulGraphNode {
    state: Box<dyn Any + Send + Sync>,
    data: Box<dyn ErasedGraphNode>,
}

impl StatefulGraphNode {
    pub fn new(node: Box<dyn ErasedGraphNode>, ctx: GraphNodeDefaultStateContext<'_>) -> Self {
        Self {
            state: node.default_state(ctx),
            data: node,
        }
    }

    pub fn id(&self) -> &'static str {
        self.data.id()
    }

    pub fn header_hue_chroma(&self) -> (f32, f32) {
        self.data.header_hue_chroma()
    }

    pub fn view<'a>(
        &'a self,
        node_id: GraphNodeId,
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'a, ErasedGraphNodeMessage> {
        self.data.view(node_id, self.state.as_ref(), ctx)
    }

    pub fn update(&mut self, message: ErasedGraphNodeMessage, ctx: GraphNodeUpdateContext<'_>) {
        self.data.update(self.state.as_mut(), message, ctx);
    }

    pub fn generate_code(
        &self,
        ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        self.data.generate_code(self.state.as_ref(), ctx)
    }

    pub fn serialize_state(&self) -> Result<toml::Value> {
        self.data.serialize_state(self.state.as_ref())
    }

    pub fn deserialize_and_set_state(
        &mut self,
        value: toml::Value,
        resources: &GraphResources,
    ) -> Result<()> {
        self.state = self.data.deserialize_state(value, resources)?;
        Ok(())
    }

    pub fn subgraphs(&self) -> Vec<&Graph> {
        self.data.subgraphs(self.state.as_ref())
    }

    pub fn subgraphs_mut(&mut self) -> Vec<&mut Graph> {
        self.data.subgraphs_mut(self.state.as_mut())
    }

    pub fn create_inputs(
        &self,
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        self.data.create_inputs(self.state.as_ref(), ctx)
    }

    pub fn create_outputs(
        &self,
        ctx: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        self.data.create_outputs(self.state.as_ref(), ctx)
    }

    pub fn update_signature(&self, ctx: GraphNodeUpdateSignatureContext<'_>) {
        self.data.update_signature(self.state.as_ref(), ctx);
    }

    pub fn is<T: GraphNode>(&self) -> bool {
        self.data.downcast_ref::<T>().is_some()
    }

    pub fn state<T: GraphNode>(&self) -> Option<&T::State> {
        self.state.downcast_ref()
    }

    pub fn state_mut<T: GraphNode>(&mut self) -> Option<&mut T::State> {
        self.state.downcast_mut()
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct StatelessState {
    #[serde(skip)]
    _private: (),
}

pub trait StatelessCommonGraphNode: Send + Sync + 'static + DynClone {
    fn id(&self) -> &'static str;
    fn header_hue_chroma(&self) -> (f32, f32);
    fn create_inputs(&self, ctx: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot>;
    fn create_outputs(&self, ctx: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot>;
    fn update_signature(&self, _: GraphNodeUpdateSignatureContext<'_>) {}
    fn generate_code(
        &self,
        ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError>;
}

pub struct GraphNodeData {
    pub position: Point,
    pub data: StatefulGraphNode,
    pub inputs: Arc<[GraphInputSlotId]>,
    pub outputs: Arc<[GraphOutputSlotId]>,
}

impl GraphNodeData {
    pub fn view<'a>(
        &'a self,
        node_id: GraphNodeId,
        slots: &GraphSlots,
        resources: &GraphResources,
    ) -> GraphElement<'a, ErasedGraphNodeMessage> {
        self.data.view(
            node_id,
            GraphNodeViewContext {
                inputs: &self.inputs,
                outputs: &self.outputs,
                slots,
                resources,
            },
        )
    }
}

#[derive(Clone, Copy)]
pub struct GraphNodeDefaultStateContext<'a> {
    pub resources: &'a GraphResources,
}

pub struct GraphNodeCreateSlotsContext<'a> {
    pub resources: &'a GraphResources,
}

pub struct GraphNodeViewContext<'a> {
    pub inputs: &'a [GraphInputSlotId],
    pub outputs: &'a [GraphOutputSlotId],
    pub slots: &'a GraphSlots,
    pub resources: &'a GraphResources,
}

impl GraphNodeViewContext<'_> {
    pub fn get_input(&self, index: usize) -> Option<&GraphInputSlotData> {
        self.slots.get_input(self.inputs.get(index)?)
    }

    pub fn get_output(&self, index: usize) -> Option<&GraphOutputSlotData> {
        self.slots.get_output(self.outputs.get(index)?)
    }

    pub fn view_input_slot<'a, Message: 'static>(
        &self,
        index: usize,
        map_literal: impl Fn(ErasedGraphLiteralUpdateMessage) -> Message + Copy + 'static,
    ) -> Option<GraphElement<'a, Message>> {
        let slot_id = *self.inputs.get(index)?;
        let slot = self.slots.get_input(&slot_id)?;
        Some(input_slot(slot_id, t!(&slot.name), slot).map(map_literal))
    }

    pub fn view_output_slot<'a, Message: 'static>(
        &self,
        index: usize,
    ) -> Option<GraphElement<'a, Message>> {
        let slot_id = *self.outputs.get(index)?;
        let slot = self.slots.get_output(&slot_id)?;
        Some(output_slot(slot_id, t!(&slot.name), slot))
    }

    pub fn view_all_inputs<'a, Message: 'static>(
        &self,
        map_literal: impl Fn(ErasedGraphLiteralUpdateMessage) -> Message + Copy + 'static,
    ) -> Vec<GraphElement<'a, Message>> {
        self.inputs
            .iter()
            .filter_map(|id| {
                let slot = self.slots.get_input(id)?;
                Some(input_slot(*id, t!(&slot.name), slot).map(map_literal))
            })
            .collect()
    }

    pub fn view_all_outputs<'a, Message: 'static>(&self) -> Vec<GraphElement<'a, Message>> {
        self.outputs
            .iter()
            .filter_map(|id| {
                let slot = self.slots.get_output(id)?;
                Some(output_slot(*id, t!(&slot.name), slot))
            })
            .collect()
    }

    pub fn view_all_slots<'a, Message: 'static>(
        &self,
        map_literal: impl Fn(ErasedGraphLiteralUpdateMessage) -> Message + Copy + 'static,
    ) -> GraphElement<'a, Message> {
        Column::new()
            .push(
                Column::with_children(self.view_all_inputs(map_literal))
                    .width(Length::Fill)
                    .spacing(4),
            )
            .push(
                Column::with_children(self.view_all_outputs())
                    .width(Length::Fill)
                    .spacing(4),
            )
            .width(Length::Fill)
            .spacing(2)
            .into()
    }

    pub fn view_all_slots_with_header<'a, Message: 'static>(
        &self,
        header: impl Into<GraphElement<'a, Message>>,
        map_literal: impl Fn(ErasedGraphLiteralUpdateMessage) -> Message + Copy + 'static,
    ) -> GraphElement<'a, Message> {
        Column::new()
            .push(header)
            .push(self.view_all_slots(map_literal))
            .spacing(2)
            .into()
    }

    pub fn all_inputs(&self) -> impl Iterator<Item = (&GraphInputSlotId, &GraphInputSlotData)> {
        self.inputs
            .iter()
            .filter_map(move |id| self.slots.get_input(id).map(|slot| (id, slot)))
    }

    pub fn all_outputs(&self) -> impl Iterator<Item = (&GraphOutputSlotId, &GraphOutputSlotData)> {
        self.outputs
            .iter()
            .filter_map(move |id| self.slots.get_output(id).map(|slot| (id, slot)))
    }
}

pub struct GraphNodeUpdateContext<'a> {
    pub inputs: &'a [GraphInputSlotId],
    pub slots: &'a mut GraphSlots,
    pub resources: &'a GraphResources,
}

impl GraphNodeUpdateContext<'_> {
    pub fn get_input(&self, index: usize) -> Option<&GraphInputSlotData> {
        self.slots.get_input(self.inputs.get(index)?)
    }

    pub fn get_input_mut(&mut self, index: usize) -> Option<&mut GraphInputSlotData> {
        let slot_id = self.inputs.get(index)?;
        self.slots.inputs.get_mut(slot_id)
    }

    pub fn update_literal(&mut self, message: ErasedGraphLiteralUpdateMessage) {
        let Some(slot) = self.slots.inputs.get_mut(&message.id) else {
            return;
        };
        slot.data.update(message);
    }
}

pub struct GraphNodeUpdateSignatureContext<'a> {
    pub inputs: &'a [GraphInputSlotId],
    pub outputs: &'a [GraphOutputSlotId],
    pub slots: &'a GraphSlots,
    pub signature: &'a mut GraphSignature,
    pub resources: &'a GraphResources,
}

impl GraphNodeUpdateSignatureContext<'_> {
    pub fn require_output_slot_as_graph_input(&mut self, index: usize, name: String) {
        let Some(slot_id) = self.outputs.get(index) else {
            return;
        };
        let Some(slot) = self.slots.outputs.get(slot_id) else {
            return;
        };

        self.signature.inputs.insert(
            *slot_id,
            GraphVariable::new_boxed(name, slot.data_ty.clone()),
        );
    }

    pub fn require_input_slot_as_graph_output(&mut self, index: usize, name: String) {
        let Some(slot_id) = self.inputs.get(index) else {
            return;
        };
        let Some(slot) = self.slots.inputs.get(slot_id) else {
            return;
        };

        self.signature.outputs.insert(
            *slot_id,
            GraphVariable::new_boxed(name, slot.data.ty().clone()),
        );
    }
}

pub struct GraphNodeCodeGenContext<'a> {
    pub inputs: &'a [GraphInputSlotId],
    pub outputs: &'a [GraphOutputSlotId],
    pub graph_slots: &'a GraphSlots,
    pub output_slot_idents: &'a mut HashMap<GraphOutputSlotId, String>,
    pub ident_generator: &'a mut GraphVarIdentGenerator,
    pub resources: &'a GraphResources,
}

impl GraphNodeCodeGenContext<'_> {
    pub fn get_input(&self, index: usize) -> Result<String, GraphNodeCodeGenError> {
        let slot_id = self
            .inputs
            .get(index)
            .ok_or(GraphNodeCodeGenError::SlotIndexOutOfBounds)?;
        let slot = self
            .graph_slots
            .get_input(slot_id)
            .ok_or(GraphNodeCodeGenError::MissingInputSlot)?;
        let Some(connected) = slot.connected else {
            return slot
                .data
                .to_code()
                .ok_or(GraphNodeCodeGenError::LiteralToCodeFailed);
        };
        let output_slot = self
            .graph_slots
            .get_output(&connected)
            .ok_or(GraphNodeCodeGenError::MissingOutputSlot)?;
        let ident = self
            .output_slot_idents
            .get(&connected)
            .ok_or(GraphNodeCodeGenError::MissingOutputSlot)?;

        if output_slot.data_ty.id() != slot.data.ty().id() {
            self.resources
                .type_registry
                .try_wgsl_cast(&*output_slot.data_ty, slot.data.ty().as_ref(), ident)
                .ok_or(GraphNodeCodeGenError::FailedToCastVariable)
        } else {
            Ok(ident.clone())
        }
    }

    pub fn get_input_raw<T: GraphLiteralValue>(
        &self,
        index: usize,
    ) -> Result<&T, GraphNodeCodeGenError> {
        let slot_id = self
            .inputs
            .get(index)
            .ok_or(GraphNodeCodeGenError::SlotIndexOutOfBounds)?;
        let slot = self
            .graph_slots
            .get_input(slot_id)
            .ok_or(GraphNodeCodeGenError::MissingInputSlot)?;
        Ok(slot.data.as_ref::<T>())
    }

    pub fn get_output(&mut self, index: usize) -> Result<String, GraphNodeCodeGenError> {
        let slot_id = self
            .outputs
            .get(index)
            .ok_or(GraphNodeCodeGenError::SlotIndexOutOfBounds)?;
        Ok(match self.output_slot_idents.entry(*slot_id) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => entry.insert(self.ident_generator.next_output()).clone(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GraphNodeCodeGenError {
    #[error("Input slot index out of bounds")]
    SlotIndexOutOfBounds,
    #[error("Missing input slot")]
    MissingInputSlot,
    #[error("Missing output slot")]
    MissingOutputSlot,
    #[error("Failed to cast variable")]
    FailedToCastVariable,
    #[error("Failed to convert literal to code")]
    LiteralToCodeFailed,
    #[error(transparent)]
    Custom(#[from] anyhow::Error),
}

#[derive(Debug)]
pub struct ContextualGraphNodeCodeGenError {
    pub node_id: GraphNodeId,
    pub node_title: String,
    pub err: GraphNodeCodeGenError,
    pub code: String,
}

impl std::fmt::Display for ContextualGraphNodeCodeGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error in node {:?} of type {}: {}\nCode already generated:\n{}",
            self.node_id, self.node_title, self.err, self.code
        )
    }
}

pub struct GraphNodeRegistry {
    nodes: BTreeMap<&'static str, Box<dyn ErasedGraphNode>>,
}

impl Default for GraphNodeRegistry {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
        }
    }
}

impl Clone for GraphNodeRegistry {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
        }
    }
}

impl GraphNodeRegistry {
    pub fn with_capacity() -> Self {
        Self {
            nodes: BTreeMap::new(),
        }
    }

    pub fn register<T: ErasedGraphNode + Default>(&mut self) {
        let node = Box::new(T::default());
        self.nodes.insert(node.id(), node);
    }

    pub fn register_boxed(&mut self, node: Box<dyn ErasedGraphNode>) {
        self.nodes.insert(node.id(), node);
    }

    pub fn get(&self, name: &str) -> Option<Box<dyn ErasedGraphNode>> {
        self.nodes.get(name).cloned()
    }

    pub fn all(&self) -> &BTreeMap<&'static str, Box<dyn ErasedGraphNode>> {
        &self.nodes
    }

    pub fn merge(&mut self, other: GraphNodeRegistry) {
        self.nodes.extend(other.nodes);
    }
}
