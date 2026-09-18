use indexmap::IndexMap;
use lapiz_assets::asset::{AssetHandle, AssetId};
use lapiz_effect::{asset::EffectInputSlotId, instance::EffectInstance};
use lapiz_shader_graph::{
    graph::{
        slot::{ErasedGraphLiteralUpdateMessage, ErasedGraphValueType},
        variable::{GraphLiteral, GraphShaderLiteral},
    },
    wgsl_std::types::LayerType,
};

use crate::asset::{FilterPreset, FilterPresetMetadata, SerializableFilterParameter};

#[derive(Clone)]
pub struct FilterParameter {
    pub name: String,
    pub value: GraphLiteral,
}

pub struct FilterInstance {
    filter_id: Option<AssetId<FilterPreset>>,
    metadata: FilterPresetMetadata,
    effect: EffectInstance,
    parameters: IndexMap<EffectInputSlotId, FilterParameter>,
}

impl FilterInstance {
    pub fn from_asset(handle: &AssetHandle<FilterPreset>) -> anyhow::Result<Self> {
        let preset = handle
            .get()
            .map_err(|e| anyhow::anyhow!("Filter preset asset is not loaded yet: {e}"))?;
        let mut instance = Self::new(&preset, crate::render::graph::filter_graph_resources())?;
        instance.filter_id = Some(handle.id());
        Ok(instance)
    }

    pub fn new(
        preset: &FilterPreset,
        resources: lapiz_shader_graph::graph::GraphResources,
    ) -> anyhow::Result<Self> {
        let effect = EffectInstance::from_asset(&preset.effect, resources.clone())?;

        // Every non-layer input is a parameter. Persisted values win; new or
        // missing inputs fall back to the type's default literal. Persisted
        // ids that no longer match an effect input are dropped.
        let mut parameters = IndexMap::new();
        for (id, slot) in &effect.inputs {
            if slot.ty.is::<LayerType>() {
                continue;
            }
            if let Some(persisted) = preset.parameters.get(id) {
                match persisted.value.deserialize(&resources.type_registry) {
                    Ok(value) => {
                        parameters.insert(
                            *id,
                            FilterParameter {
                                name: slot.name.clone(),
                                value,
                            },
                        );
                        continue;
                    }
                    Err(e) => {
                        log::warn!(
                            "Filter parameter '{}' failed to deserialize, using default: {e}",
                            slot.name
                        );
                    }
                }
            }
            parameters.insert(
                *id,
                FilterParameter {
                    name: slot.name.clone(),
                    value: default_parameter_literal(&slot.ty),
                },
            );
        }

        for id in preset.parameters.keys() {
            if !effect.inputs.contains_key(id) {
                log::warn!("Filter preset has a parameter with no matching effect input");
            }
        }

        Ok(Self {
            filter_id: None,
            metadata: preset.metadata.clone(),
            effect,
            parameters,
        })
    }

    pub fn as_asset(&self) -> anyhow::Result<FilterPreset> {
        let mut parameters = IndexMap::new();
        for (id, parameter) in &self.parameters {
            parameters.insert(
                *id,
                SerializableFilterParameter {
                    name: parameter.name.clone(),
                    value: lapiz_shader_graph::save::SerializableGraphLiteral::serialize(
                        &parameter.value,
                    )?,
                },
            );
        }
        Ok(FilterPreset {
            metadata: self.metadata.clone(),
            effect: self.effect.as_asset()?,
            parameters,
        })
    }

    pub fn asset_id(&self) -> Option<AssetId<FilterPreset>> {
        self.filter_id
    }

    pub fn metadata(&self) -> &FilterPresetMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut FilterPresetMetadata {
        &mut self.metadata
    }

    pub fn effect(&self) -> &EffectInstance {
        &self.effect
    }

    pub fn effect_mut(&mut self) -> &mut EffectInstance {
        &mut self.effect
    }

    pub fn parameters(&self) -> &IndexMap<EffectInputSlotId, FilterParameter> {
        &self.parameters
    }

    pub fn parameters_mut(&mut self) -> &mut IndexMap<EffectInputSlotId, FilterParameter> {
        &mut self.parameters
    }

    pub fn update_parameter(
        &mut self,
        id: &EffectInputSlotId,
        message: ErasedGraphLiteralUpdateMessage,
    ) {
        if let Some(parameter) = self.parameters.get_mut(id) {
            parameter.value.update(message);
        }
    }
}

fn default_parameter_literal(ty: &std::sync::Arc<dyn ErasedGraphValueType>) -> GraphLiteral {
    GraphLiteral::new_boxed(ty.default_literal(), ty.clone())
}
