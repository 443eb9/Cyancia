//! End-to-end filter test: builds a parameterless invert filter, round trips
//! it through the `.lfp` preset asset, runs it on multiple generated layers
//! and compares the GPU output against a CPU reference.

use std::sync::Arc;

use anyhow::Result;
use bevy_math::IRect;
use futures::executor::block_on;
use iced_core::Point;
use indexmap::IndexMap;
use lapiz_assets::loader::AssetSerializer;
use lapiz_effect::{
    asset::*,
    instance::{EffectInputSlot, EffectInstance, EffectOutputSlot, EffectPass},
    nodes::{PassInput, PassInputNode, PassOutput, PassOutputNode},
};
use lapiz_filter::{
    asset::{FilterPreset, FilterPresetMetadata, FilterPresetSerializer},
    instance::FilterInstance,
    render::{FilterLayer, FilterRenderer, graph::filter_graph_nodes},
};
use lapiz_image::{
    layer::LayerId,
    texel::TexelType,
    tile::{DynamicLayerStorage, GpuTileStorage, GpuTileStorageInner},
};
use lapiz_runtime::renderer::RenderContext;
use lapiz_shader_graph::{
    graph::{
        Graph, GraphResources,
        function::ASSET_GRAPH_FUNCTION_STORAGE,
        node::{
            GraphNodeCodeGenContext, GraphNodeCodeGenError, GraphNodeCreateSlotsContext,
            GraphNodeId, StatelessCommonGraphNode, stateless,
        },
        slot::{ErasedGraphValueType, GraphDefaultInputSlot, GraphDefaultOutputSlot},
    },
    wgsl_std::{builtin_types, types::*},
};
use lapiz_utils::random_oklch_hue_chroma;
use uuid::Uuid;
use wesl::syntax::*;
use wesl_quote::quote_statement;

#[derive(Default, Clone)]
struct InvertNode;

#[stateless]
impl StatelessCommonGraphNode for InvertNode {
    fn id(&self) -> &'static str {
        "test_invert_node"
    }
    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(InvertNode)
    }
    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<ColorType>("color".into()),
            GraphDefaultInputSlot::new::<F32Type>("amount".into()),
        ]
    }
    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }
    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let input = ctx.get_input(0)?;
        let amount = ctx.get_input(1)?;
        let output = ctx.get_output(0)?;
        Ok(quote_statement! {
            let #output = vec4f(mix((#input).rgb, vec3f(1.0) - (#input).rgb, #amount), (#input).a);
        }
        .to_string())
    }
}

// Registry includes the test node so the asset round trip can resolve it.
fn graph_resources() -> GraphResources {
    let mut nodes = filter_graph_nodes();
    nodes.register::<InvertNode>();
    GraphResources {
        type_registry: Arc::new(builtin_types()),
        node_registry: Arc::new(nodes),
        functions: ASSET_GRAPH_FUNCTION_STORAGE.clone(),
    }
}

fn generate_invert_filter() -> Result<FilterInstance> {
    let layer_ty: Arc<dyn ErasedGraphValueType> = Arc::new(LayerType {
        texel_type: TexelType::RGBA8,
    });

    let target_input = EffectInputSlotId::new(Uuid::new_v4());
    let amount_input = EffectInputSlotId::new(Uuid::new_v4());
    let layer_output = EffectOutputSlotId::new(Uuid::new_v4());
    let f32_ty: Arc<dyn ErasedGraphValueType> = Arc::new(F32Type);

    let mut graph = Graph::new(graph_resources());
    let target = graph.add_node(Point::ORIGIN, PassInputNode);
    graph.update_node_state::<PassInputNode>(target, |state| {
        state.input = Some(PassInput::Effect(target_input));
        state.cached_ty = Some(layer_ty.clone());
    });
    let amount = graph.add_node(Point::ORIGIN, PassInputNode);
    graph.update_node_state::<PassInputNode>(amount, |state| {
        state.input = Some(PassInput::Effect(amount_input));
        state.cached_ty = Some(f32_ty.clone());
    });
    let invert = graph.add_node(Point::ORIGIN, InvertNode);
    let output: GraphNodeId = graph.add_node(Point::ORIGIN, PassOutputNode);
    graph.update_node_state::<PassOutputNode>(output, |state| {
        state.output = Some(PassOutput::Effect(layer_output));
        state.cached_ty = Some(layer_ty.clone());
    });
    graph.connect_slots_by_index(target, 0, invert, 0);
    graph.connect_slots_by_index(amount, 0, invert, 1);
    graph.connect_slots_by_index(invert, 0, output, 0);
    graph.connect_slots_by_index(target, 1, output, 1);

    let output_pass_port = graph
        .get_node(&output)
        .unwrap()
        .data
        .state::<PassOutputNode>()
        .unwrap()
        .id;

    let passes = IndexMap::from([(
        EffectPassId::new(Uuid::new_v4()),
        EffectPass {
            name: "Invert".into(),
            graph,
            dispatch_strategy: EffectPassDispatchStrategy::EveryOutputLayerPixel(output_pass_port),
        },
    )]);
    let inputs = IndexMap::from([
        (
            target_input,
            EffectInputSlot {
                name: "Target".into(),
                id: target_input,
                ty: layer_ty.clone(),
            },
        ),
        (
            amount_input,
            EffectInputSlot {
                name: "Amount".into(),
                id: amount_input,
                ty: f32_ty.clone(),
            },
        ),
    ]);
    let outputs = IndexMap::from([(
        layer_output,
        EffectOutputSlot {
            name: "Layer".into(),
            id: layer_output,
            ty: layer_ty,
        },
    )]);

    let preset = FilterPreset {
        metadata: FilterPresetMetadata {
            name: "Invert Test".into(),
        },
        effect: EffectInstance {
            name: "Invert".into(),
            passes,
            inputs,
            outputs,
        }
        .as_asset()?,
        parameters: IndexMap::new(),
    };
    FilterInstance::new(&preset, graph_resources())
}

// Deterministic integer hash so every channel value is reproducible.
fn channel_hash(x: u32, y: u32, salt: u32) -> u8 {
    let mut v = x
        .wrapping_mul(0x9E3779B1)
        ^ y.wrapping_mul(0x85EBCA77)
        ^ salt.wrapping_mul(0xC2B2AE3D);
    v ^= v >> 15;
    v = v.wrapping_mul(0x2545F491);
    v ^= v >> 13;
    (v >> 24) as u8
}

fn test_image(width: u32, height: u32) -> image::DynamicImage {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([
            channel_hash(x, y, 1),
            channel_hash(x, y, 2),
            channel_hash(x, y, 3),
            channel_hash(x, y, 4),
        ])
    });
    image::DynamicImage::ImageRgba8(image)
}

async fn readback_layer(
    storage: &DynamicLayerStorage,
    width: u32,
    height: u32,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<image::RgbaImage> {
    let tiles = storage.iter_tile_indices().collect::<Vec<_>>();
    let data = storage.readback(device, queue, tiles).await?;
    let mut output = image::RgbaImage::new(width, height);
    for (tile, bytes) in data {
        let origin = tile * GpuTileStorage::TILE_SIZE as i32;
        for y in 0..GpuTileStorage::TILE_SIZE.min(height.saturating_sub(origin.y as u32)) {
            for x in 0..GpuTileStorage::TILE_SIZE.min(width.saturating_sub(origin.x as u32)) {
                let offset = ((y * GpuTileStorage::TILE_SIZE + x) * 4) as usize;
                output.put_pixel(
                    origin.x as u32 + x,
                    origin.y as u32 + y,
                    image::Rgba(bytes[offset..offset + 4].try_into().unwrap()),
                );
            }
        }
    }
    Ok(output)
}

fn gpu_available() -> bool {
    let backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    !block_on(instance.enumerate_adapters(backends)).is_empty()
}

#[test]
fn filter_round_trips_and_runs_on_layers() -> Result<()> {
    if !gpu_available() {
        eprintln!("skipping: no wgpu adapter available");
        return Ok(());
    }

    let mut generated = generate_invert_filter()?;

    // Edit the parameter away from its default so the persisted value is
    // distinguishable from a fallback.
    const AMOUNT: f32 = 0.75;
    let amount_id = generated
        .parameters()
        .iter()
        .find(|(_, parameter)| parameter.name == "Amount")
        .map(|(id, _)| *id)
        .expect("Amount parameter exists");
    *generated
        .parameters_mut()
        .get_mut(&amount_id)
        .unwrap()
        .value
        .as_mut::<f32>() = AMOUNT;

    // Serialize -> deserialize round trip through the .lfp zip container;
    // the parameter value must survive it.
    let serializer = FilterPresetSerializer;
    let mut encoded = Vec::new();
    serializer.write(&generated.as_asset()?, &mut encoded)?;
    let preset = serializer.read(&mut std::io::Cursor::new(&encoded))?;
    assert_eq!(preset.metadata.name, "Invert Test");
    let instance = FilterInstance::new(&preset, graph_resources())?;
    assert_eq!(instance.parameters()[&amount_id].name, "Amount");
    assert_eq!(*instance.parameters()[&amount_id].value.as_ref::<f32>(), AMOUNT);

    let context = RenderContext::default();
    let renderer = FilterRenderer::from_context(
        &instance,
        context.device.clone(),
        context.queue.clone(),
    )?;

    // Multiple layers: small, multi-tile (crosses the 256 tile boundary), square.
    let sizes: [(u32, u32); 3] = [(64, 48), (300, 180), (96, 96)];
    let uploads = GpuTileStorageInner::new(context.device.clone(), context.queue.clone());
    let mut layers = Vec::with_capacity(sizes.len());
    let mut originals = Vec::with_capacity(sizes.len());
    let mut layer_ids = Vec::with_capacity(sizes.len());
    for (width, height) in sizes {
        let image = test_image(width, height);
        let id = LayerId::new(Uuid::new_v4());
        uploads.upload_image(id, image.clone());
        layer_ids.push(id);
        layers.push(FilterLayer {
            id,
            storage: uploads.get_layer(id).unwrap().deep_clone(),
            bounds: IRect::new(0, 0, width as i32, height as i32),
        });
        originals.push((width, height, image));
    }
    let layers_snapshot = layer_ids.clone();

    let layer_ids = layers_snapshot;
    let results = renderer.run_layers(layers, instance.parameters().clone())?;
    assert_eq!(results.len(), originals.len());

    for (index, (width, height, original)) in originals.iter().enumerate() {
        let storage = &results[&layer_ids[index]];
        let result = block_on(readback_layer(
            storage,
            *width,
            *height,
            &context.device,
            &context.queue,
        ))?;
        let original = original.as_rgba8().unwrap();

        // CPU reference mirrors the shader: rgb' = trunc(clamp(mix(c, 1 - c,
        // amount) * 255)), alpha passes through.
        let mut max_delta = 0_i32;
        for ((_, _, original_pixel), (_, _, result_pixel)) in
            original.enumerate_pixels().zip(result.enumerate_pixels())
        {
            for channel in 0..4 {
                let value = f32::from(original_pixel.0[channel]) / 255.0;
                let inverted = if channel < 3 {
                    value + (1.0 - value - value) * AMOUNT
                } else {
                    value
                };
                let expected = ((inverted * 255.0).clamp(0.0, 255.0) as u32) as u8;
                let delta = (i32::from(expected) - i32::from(result_pixel.0[channel])).abs();
                max_delta = max_delta.max(delta);
            }
        }
        assert!(
            max_delta <= 1,
            "layer {index} ({width}x{height}) diverged from CPU reference by {max_delta}"
        );
        eprintln!("layer {index} ({width}x{height}): max delta {max_delta}");
    }

    Ok(())
}
