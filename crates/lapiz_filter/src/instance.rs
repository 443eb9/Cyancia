use indexmap::IndexMap;
use lapiz_assets::asset::{AssetHandle, AssetId};
use lapiz_effect::{asset::EffectInputSlotId, instance::EffectInstance};
use lapiz_shader_graph::graph::{slot::ErasedGraphLiteralUpdateMessage, variable::GraphLiteral};

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

        let mut parameters = IndexMap::new();
        for (id, slot) in &effect.inputs {
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
                    value: GraphLiteral::new_boxed_default(slot.ty.clone()),
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

    pub fn resync_parameters(&mut self) {
        self.parameters
            .retain(|id, _| self.effect.inputs.contains_key(id));
        for (id, slot) in &self.effect.inputs {
            let parameter = self
                .parameters
                .entry(*id)
                .or_insert_with(|| FilterParameter {
                    name: slot.name.clone(),
                    value: GraphLiteral::new_boxed_default(slot.ty.clone()),
                });
            parameter.name = slot.name.clone();
        }
    }
}
