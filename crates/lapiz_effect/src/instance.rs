use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use lapiz_shader_graph::graph::{
    Graph, GraphResources, function::ASSET_GRAPH_FUNCTION_STORAGE, slot::ErasedGraphValueType,
    variable::GraphShaderLiteral,
};

use crate::{
    asset::*,
    nodes::{DispatchIndexNode, PassInput, PassInputNode, PassOutput, PassOutputNode},
    render::{EffectPassInputSlotSource, EffectPassOutputSlotTarget},
};

pub type EffectInputsDecl = HashMap<EffectInputSlotId, Arc<dyn ErasedGraphValueType>>;
pub type EffectOutputsDecl = HashMap<EffectOutputSlotId, Arc<dyn ErasedGraphValueType>>;
pub type EffectPassInputsDecl = HashMap<EffectPassInputSlotId, EffectPassInputSlotSource>;
pub type EffectPassOutputsDecl = HashMap<EffectPassOutputSlotId, EffectPassOutputSlotTarget>;

pub type EffectInputs = HashMap<EffectInputSlotId, GraphShaderLiteral>;
pub type EffectOutputs = HashMap<EffectOutputSlotId, GraphShaderLiteral>;
pub type EffectPassOutputs = HashMap<EffectPassOutputSlotId, GraphShaderLiteral>;
pub type EffectPassOutputTypes = HashMap<EffectPassOutputSlotId, Arc<dyn ErasedGraphValueType>>;

// Editor ui edits this instance and creates renderer or serializes into assets.
pub struct EffectInstance {
    pub name: String,
    pub passes: IndexMap<EffectPassId, EffectPass>,
    pub inputs: IndexMap<EffectInputSlotId, EffectInputSlot>,
    pub outputs: IndexMap<EffectOutputSlotId, EffectOutputSlot>,
}

impl EffectInstance {
    pub fn from_asset(asset: &EffectAsset, resources: GraphResources) -> Result<Self> {
        let inputs = asset
            .inputs
            .iter()
            .map(|slot| {
                Ok((
                    slot.id,
                    EffectInputSlot {
                        name: slot.name.clone(),
                        id: slot.id,
                        ty: resources
                            .type_registry
                            .resolve_type(&slot.ty)
                            .with_context(|| format!("Unknown effect input type '{}'", slot.ty))?
                            .clone(),
                    },
                ))
            })
            .collect::<Result<IndexMap<_, _>>>()?;
        let outputs = asset
            .outputs
            .iter()
            .map(|slot| {
                Ok((
                    slot.id,
                    EffectOutputSlot {
                        name: slot.name.clone(),
                        id: slot.id,
                        ty: resources
                            .type_registry
                            .resolve_type(&slot.ty)
                            .with_context(|| format!("Unknown effect output type '{}'", slot.ty))?
                            .clone(),
                    },
                ))
            })
            .collect::<Result<IndexMap<_, _>>>()?;

        let mut passes = IndexMap::with_capacity(asset.passes.len());
        // TODO(editor adapter): cached port types must eventually be injected before
        // graph deserialization so slot reconciliation does not depend on serialized caches.
        for pass in &asset.passes {
            let (graph, errors) = Graph::from_serialized(
                &pass.graph,
                GraphResources {
                    type_registry: resources.type_registry.clone(),
                    node_registry: resources.node_registry.clone(),
                    functions: ASSET_GRAPH_FUNCTION_STORAGE.clone(),
                },
            );
            if let Some(error) = errors.first() {
                bail!("Pass '{}' graph failed to deserialize: {error}", pass.name);
            }
            let graph = graph.context("Pass graph failed to deserialize")?;
            passes.insert(
                pass.id,
                EffectPass {
                    name: pass.name.clone(),
                    graph,
                    dispatch_strategy: pass.dispatch_strategy,
                },
            );
        }

        let mut instance = Self {
            name: asset.name.clone(),
            passes,
            inputs,
            outputs,
        };
        instance.sync_pass_graph_effect_properties()?;
        Ok(instance)
    }

    pub fn as_asset(&self) -> Result<EffectAsset> {
        Ok(EffectAsset {
            name: self.name.clone(),
            inputs: self
                .inputs
                .values()
                .map(|slot| SerializableEffectInputSlot {
                    name: slot.name.clone(),
                    id: slot.id,
                    ty: slot.ty.id().id,
                })
                .collect(),
            outputs: self
                .outputs
                .values()
                .map(|slot| SerializableEffectOutputSlot {
                    name: slot.name.clone(),
                    id: slot.id,
                    ty: slot.ty.id().id,
                })
                .collect(),
            passes: self
                .passes
                .iter()
                .map(|(id, pass)| {
                    Ok(SerializableEffectPass {
                        id: *id,
                        name: pass.name.clone(),
                        graph: pass.graph.as_serialized()?,
                        dispatch_strategy: pass.dispatch_strategy,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        })
    }

    pub fn sync_pass_graph_effect_properties(&mut self) -> Result<()> {
        let effect_input_types = self
            .inputs
            .iter()
            .map(|(id, input)| (*id, input.ty.clone()))
            .collect::<HashMap<_, _>>();
        let effect_output_types = self
            .outputs
            .iter()
            .map(|(id, output)| (*id, output.ty.clone()))
            .collect::<HashMap<_, _>>();

        let mut all_pass_outputs = EffectPassOutputTypes::new();
        for pass in self.passes.values() {
            for node in pass.graph.iter_nodes() {
                let Some(state) = node.data.state::<PassOutputNode>() else {
                    continue;
                };
                let Some(output) = &state.output else {
                    continue;
                };
                let ty = match output {
                    PassOutput::Pass(def) => def.ty.clone(),
                    PassOutput::Effect(id) => effect_output_types
                        .get(id)
                        .with_context(|| format!("Unknown effect output {id:?}"))?
                        .clone(),
                };
                if all_pass_outputs.insert(state.id, ty).is_some() {
                    bail!("Duplicate pass output ID {:?}", state.id);
                }
            }
        }

        for pass in self.passes.values_mut() {
            for node in pass.graph.iter_nodes_mut() {
                if let Some(state) = node.data.state_mut::<DispatchIndexNode>() {
                    state.cached_dispatch_strategy = pass.dispatch_strategy;
                } else if let Some(state) = node.data.state_mut::<PassInputNode>() {
                    state.cached_ty = match state.input {
                        Some(PassInput::Effect(id)) => Some(
                            effect_input_types
                                .get(&id)
                                .with_context(|| format!("Unknown effect input {id:?}"))?
                                .clone(),
                        ),
                        Some(PassInput::Pass(id)) => Some(
                            all_pass_outputs
                                .get(&id)
                                .with_context(|| format!("Unknown pass output {id:?}"))?
                                .clone(),
                        ),
                        None => None,
                    };
                } else if let Some(state) = node.data.state_mut::<PassOutputNode>() {
                    state.cached_ty = match &state.output {
                        Some(PassOutput::Pass(def)) => Some(def.ty.clone()),
                        Some(PassOutput::Effect(id)) => Some(
                            effect_output_types
                                .get(id)
                                .with_context(|| format!("Unknown effect output {id:?}"))?
                                .clone(),
                        ),
                        None => None,
                    };
                }
            }
            pass.graph.reconcile_all_node_slots();
        }
        Ok(())
    }
}

pub struct EffectPass {
    pub name: String,
    pub graph: Graph,
    pub dispatch_strategy: EffectPassDispatchStrategy,
}

#[derive(Clone)]
pub struct EffectInputSlot {
    pub name: String,
    pub id: EffectInputSlotId,
    pub ty: Arc<dyn ErasedGraphValueType>,
}

#[derive(Clone)]
pub struct EffectOutputSlot {
    pub name: String,
    pub id: EffectOutputSlotId,
    pub ty: Arc<dyn ErasedGraphValueType>,
}
