use std::{collections::HashMap, env, fs::File, path::Path, sync::Arc};

use anyhow::{Context, Result, ensure};
use encase::{ShaderType, StorageBuffer};
use futures::executor::block_on;
use iced_core::{Element, Point, widget::Void};
use image::{GrayImage, RgbaImage};
use indexmap::IndexMap;
use lapiz_assets::loader::AssetSerializer;
use lapiz_effect::{
    asset::*,
    instance::{EffectInputSlot, EffectInstance, EffectOutputSlot, EffectPass},
    nodes::{
        DispatchIndexNode, PassInput, PassInputNode, PassOutput, PassOutputDef, PassOutputNode,
        effect_nodes,
    },
    render::EffectRenderer,
};
use lapiz_image::{
    layer::LayerId,
    texel::TexelType,
    tile::{GpuTileStorage, GpuTileStorageInner},
};
use lapiz_render::{
    bind_group_entries::DynamicBindGroupEntries,
    bind_group_layout_entries::{DynamicBindGroupLayoutEntries, binding_types},
    util::DevicePollExt,
};
use lapiz_runtime::renderer::RenderContext;
use lapiz_shader_graph::{
    GraphRenderer, GraphTheme,
    graph::{
        Graph, GraphResources,
        function::ASSET_GRAPH_FUNCTION_STORAGE,
        node::{
            GraphNode, GraphNodeCodeGenContext, GraphNodeCodeGenError, GraphNodeCreateSlotsContext,
            GraphNodeDefaultStateContext, GraphNodeId, GraphNodeUpdateContext,
            GraphNodeViewContext, StatelessCommonGraphNode, stateless,
        },
        slot::{GraphDefaultInputSlot, GraphDefaultOutputSlot, GraphShaderStage, GraphValueType},
        variable::GraphShaderLiteral,
    },
    save::GraphValueTypeId,
    wgsl_std::{builtin_types, types::*},
};
use lapiz_utils::random_oklch_hue_chroma;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, Device, Queue, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureViewDescriptor, util::DeviceExt,
};

const HISTOGRAM_SIZE: u32 = 256;

#[derive(Clone, Copy, Serialize, Deserialize, ShaderType)]
struct EffectParam {
    ca_intensity: f32,
    texture_intensity: f32,
}

#[derive(Default, Clone)]
struct EffectParamType;

impl GraphValueType for EffectParamType {
    type AssociatedLiteralType = EffectParam;
    type PreparedShaderType = Buffer;
    type Message = ();

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new("example_effect_param")
    }

    fn push_shader_layout(
        &self,
        name: &str,
        stage: GraphShaderStage,
        group: u32,
        binding: u32,
        bindings: DynamicBindGroupLayoutEntries,
        mut shader: String,
    ) -> Result<(u32, DynamicBindGroupLayoutEntries, String)> {
        let access = if stage == GraphShaderStage::Input {
            "read"
        } else {
            "read_write"
        };
        shader.push_str(&format!("struct EffectParam {{ ca_intensity: f32, texture_intensity: f32 }};\n@group({group}) @binding({binding}) var<storage, {access}> {name}: EffectParam;\n"));
        let entry = if stage == GraphShaderStage::Input {
            binding_types::storage_buffer_read_only_sized(false, None)
        } else {
            binding_types::storage_buffer_sized(false, None)
        };
        Ok((
            binding + 1,
            bindings.extend_with_indices(((binding, entry),)),
            shader,
        ))
    }

    fn push_shader_binding<'a>(
        &self,
        _stage: GraphShaderStage,
        value: &'a Buffer,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        Ok((
            binding + 1,
            bindings.extend_with_indices(((binding, value.as_entire_binding()),)),
        ))
    }

    fn prepare_to_shader(
        &self,
        data: &EffectParam,
        device: &Device,
        _queue: &Queue,
    ) -> Result<Buffer> {
        let mut storage = StorageBuffer::new(Vec::new());
        storage.write(data)?;
        Ok(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("example effect parameters"),
                contents: storage.as_ref(),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            }),
        )
    }

    fn default_literal(&self) -> EffectParam {
        EffectParam {
            ca_intensity: 1.0,
            texture_intensity: 0.25,
        }
    }
    fn wgsl_type(&self) -> Option<(&'static str, u64)> {
        None
    }
    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(EffectParamType)
    }
    fn view_literal(&self, _data: &EffectParam) -> Element<'static, (), GraphTheme, GraphRenderer> {
        Void.into()
    }
    fn update_literal(&self, _data: &mut EffectParam, _message: ()) {}
    fn literal_to_code(&self, _data: &EffectParam) -> Option<String> {
        None
    }
}

macro_rules! parameter_node {
    ($name:ident, $id:literal, $field:literal) => {
        #[derive(Default, Clone)]
        struct $name;
        #[stateless]
        impl StatelessCommonGraphNode for $name {
            fn id(&self) -> &'static str {
                $id
            }
            fn header_hue_chroma(&self) -> (f32, f32) {
                random_oklch_hue_chroma!($name)
            }
            fn create_inputs(
                &self,
                _: GraphNodeCreateSlotsContext<'_>,
            ) -> Vec<GraphDefaultInputSlot> {
                vec![GraphDefaultInputSlot::new::<EffectParamType>(
                    "parameters".into(),
                )]
            }
            fn create_outputs(
                &self,
                _: GraphNodeCreateSlotsContext<'_>,
            ) -> Vec<GraphDefaultOutputSlot> {
                vec![GraphDefaultOutputSlot::new::<F32Type>("value".into())]
            }
            fn generate_code(
                &self,
                mut ctx: GraphNodeCodeGenContext<'_>,
            ) -> Result<String, GraphNodeCodeGenError> {
                let input = ctx.get_input(0)?;
                let output = ctx.get_output(0)?;
                Ok(format!("let {output} = {input}.{};\n", $field))
            }
        }
    };
}

parameter_node!(
    ChromaticAbberrationIntensityNode,
    "Chromatic Abberration Intensity",
    "ca_intensity"
);
parameter_node!(
    TextureIntensityNode,
    "Texture Intensity",
    "texture_intensity"
);

#[derive(Default, Clone)]
struct HistogramNode;

#[stateless]
impl StatelessCommonGraphNode for HistogramNode {
    fn id(&self) -> &'static str {
        "example_histogram"
    }
    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(HistogramNode)
    }
    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<ColorType>("color".into()),
            GraphDefaultInputSlot::new::<RectType>("bounds".into()),
        ]
    }
    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<I32Type>("bin".into()),
            GraphDefaultOutputSlot::new::<U32Type>("increment".into()),
            GraphDefaultOutputSlot::new::<U32Type>("luminance sum".into()),
            GraphDefaultOutputSlot::new::<U32Type>("pixel count".into()),
        ]
    }
    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let color = ctx.get_input(0)?;
        let bounds = ctx.get_input(1)?;
        let bin = ctx.get_output(0)?;
        let increment = ctx.get_output(1)?;
        let sum = ctx.get_output(2)?;
        let count = ctx.get_output(3)?;
        Ok(format!(
            "let example_inside = all(vec2f(dispatch_index) >= ({bounds}).min) && all(vec2f(dispatch_index) < ({bounds}).max);\nlet example_luminance = clamp(dot(({color}).rgb, vec3f(0.2126, 0.7152, 0.0722)), 0.0, 1.0);\nlet {bin} = i32(round(example_luminance * 255.0));\nlet {increment} = select(0u, 1u, example_inside);\nlet {sum} = select(0u, u32(round(example_luminance * 65535.0)), example_inside);\nlet {count} = {increment};\n"
        ))
    }
}

#[derive(Default, Clone)]
struct AutoExposureNode;

#[stateless]
impl StatelessCommonGraphNode for AutoExposureNode {
    fn id(&self) -> &'static str {
        "example_auto_exposure"
    }
    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(AutoExposureNode)
    }
    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<U32Type>("luminance sum".into()),
            GraphDefaultInputSlot::new::<U32Type>("pixel count".into()),
        ]
    }
    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<F32Type>("exposure".into())]
    }
    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let sum = ctx.get_input(0)?;
        let count = ctx.get_input(1)?;
        let exposure = ctx.get_output(0)?;
        Ok(format!(
            "let example_average_luminance = f32({sum}) / max(f32({count}) * 65535.0, 1.0);\nlet {exposure} = clamp(0.18 / max(example_average_luminance, 0.001), 0.25, 4.0);\n"
        ))
    }
}

#[derive(Default, Clone)]
struct ApplyEffectNode;

#[derive(Clone, Serialize, Deserialize)]
struct ApplyEffectNodeState {
    target: EffectPassInputSlotId,
}

impl GraphNode for ApplyEffectNode {
    type State = ApplyEffectNodeState;
    type Message = ();

    fn id(&self) -> &'static str {
        "example_apply_effect"
    }
    fn default_state(&self, _: GraphNodeDefaultStateContext<'_>) -> Self::State {
        ApplyEffectNodeState {
            target: EffectPassInputSlotId::new(Uuid::nil()),
        }
    }
    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(ApplyEffectNode)
    }
    fn create_inputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<Vec2IType>("pixel".into()),
            GraphDefaultInputSlot::new::<RectType>("bounds".into()),
            GraphDefaultInputSlot::new::<F32Type>("chromatic aberration".into()),
            GraphDefaultInputSlot::new::<F32Type>("texture intensity".into()),
            GraphDefaultInputSlot::new::<F32Type>("exposure".into()),
            GraphDefaultInputSlot::new_boxed(
                "texture".into(),
                Arc::new(TextureType {
                    texel_type: TexelType::A8,
                }),
            ),
        ]
    }
    fn create_outputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }
    fn view<'a>(
        &self,
        _: &'a Self::State,
        _: GraphNodeViewContext<'_>,
    ) -> Element<'a, Self::Message, GraphTheme, GraphRenderer> {
        Void.into()
    }
    fn update(&self, _: &mut Self::State, _: Self::Message, _: GraphNodeUpdateContext<'_>) {}
    fn generate_code(
        &self,
        state: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let pixel = ctx.get_input(0)?;
        let _bounds = ctx.get_input(1)?;
        let ca = ctx.get_input(2)?;
        let texture_intensity = ctx.get_input(3)?;
        let exposure = ctx.get_input(4)?;
        let texture = ctx.get_input(5)?;
        let output = ctx.get_output(0)?;
        let target = format!("pass_input_{}_load", state.target.0.simple());
        Ok(format!(
            "let example_shift = max(i32(round(abs({ca}) * 3.0)), 0);\nlet example_center = {target}({pixel});\nlet example_r = {target}({pixel} + vec2i(example_shift, 0)).r;\nlet example_b = {target}({pixel} - vec2i(example_shift, 0)).b;\nlet example_tex_size = vec2i(textureDimensions({texture}));\nlet example_tex_coord = vec2u((({pixel} % example_tex_size) + example_tex_size) % example_tex_size);\nlet example_gray = textureLoad({texture}, example_tex_coord, 0).r;\nlet example_modulation = mix(1.0, example_gray, clamp({texture_intensity}, 0.0, 1.0));\nlet {output} = vec4f(vec3f(example_r, example_center.g, example_b) * {exposure} * example_modulation, example_center.a);\n"
        ))
    }
}

fn resources(histogram_type: ArrayAtomicU32Type) -> GraphResources {
    let mut types = builtin_types();
    types.register_type::<EffectParamType>();
    types.register_type_value(histogram_type);
    let mut nodes = effect_nodes();
    nodes.register::<ChromaticAbberrationIntensityNode>();
    nodes.register::<TextureIntensityNode>();
    nodes.register::<HistogramNode>();
    nodes.register::<AutoExposureNode>();
    nodes.register::<ApplyEffectNode>();
    GraphResources {
        type_registry: Arc::new(types),
        node_registry: Arc::new(nodes),
        functions: ASSET_GRAPH_FUNCTION_STORAGE.clone(),
    }
}

fn input_node(
    graph: &mut Graph,
    source: PassInput,
    ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
) -> (GraphNodeId, EffectPassInputSlotId) {
    let node = graph.add_node(Point::ORIGIN, PassInputNode);
    let id = graph
        .get_node(&node)
        .unwrap()
        .data
        .state::<PassInputNode>()
        .unwrap()
        .id;
    graph.update_node_state::<PassInputNode>(node, |state| {
        state.input = Some(source);
        state.cached_ty = Some(ty);
    });
    (node, id)
}

fn output_node(
    graph: &mut Graph,
    output: PassOutput,
    ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
) -> (GraphNodeId, EffectPassOutputSlotId) {
    let node = graph.add_node(Point::ORIGIN, PassOutputNode);
    let id = graph
        .get_node(&node)
        .unwrap()
        .data
        .state::<PassOutputNode>()
        .unwrap()
        .id;
    graph.update_node_state::<PassOutputNode>(node, |state| {
        state.output = Some(output);
        state.cached_ty = Some(ty);
    });
    (node, id)
}

fn generate_effect(
    resources: GraphResources,
) -> (
    EffectInstance,
    EffectInputSlotId,
    EffectInputSlotId,
    EffectInputSlotId,
    EffectOutputSlotId,
    EffectOutputSlotId,
) {
    let target_input = EffectInputSlotId::new(Uuid::new_v4());
    let params_input = EffectInputSlotId::new(Uuid::new_v4());
    let texture_input = EffectInputSlotId::new(Uuid::new_v4());
    let layer_output = EffectOutputSlotId::new(Uuid::new_v4());
    let histogram_output = EffectOutputSlotId::new(Uuid::new_v4());

    let layer_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(LayerType {
            texel_type: TexelType::RGBA8,
        });
    let params_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(EffectParamType);
    let texture_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(TextureType {
            texel_type: TexelType::A8,
        });
    let histogram_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(ArrayAtomicU32Type {
            len: HISTOGRAM_SIZE,
        });
    let atomic_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(AtomicU32Type);
    let f32_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> = Arc::new(F32Type);

    let mut histogram_graph = Graph::new(resources.clone());
    let (target, target_port) = input_node(
        &mut histogram_graph,
        PassInput::Effect(target_input),
        layer_ty.clone(),
    );
    let histogram = histogram_graph.add_node(Point::ORIGIN, HistogramNode);
    let (hist_out, _) = output_node(
        &mut histogram_graph,
        PassOutput::Effect(histogram_output),
        histogram_ty.clone(),
    );
    let (sum_out, sum_id) = output_node(
        &mut histogram_graph,
        PassOutput::Pass(PassOutputDef {
            name: "luminance sum".into(),
            ty: atomic_ty.clone(),
        }),
        atomic_ty.clone(),
    );
    let (count_out, count_id) = output_node(
        &mut histogram_graph,
        PassOutput::Pass(PassOutputDef {
            name: "pixel count".into(),
            ty: atomic_ty.clone(),
        }),
        atomic_ty.clone(),
    );
    histogram_graph.connect_slots_by_index(target, 0, histogram, 0);
    histogram_graph.connect_slots_by_index(target, 1, histogram, 1);
    histogram_graph.connect_slots_by_index(histogram, 0, hist_out, 0);
    histogram_graph.connect_slots_by_index(histogram, 1, hist_out, 1);
    histogram_graph.connect_slots_by_index(histogram, 2, sum_out, 0);
    histogram_graph.connect_slots_by_index(histogram, 3, count_out, 0);

    let mut exposure_graph = Graph::new(resources.clone());
    let (sum_in, _) = input_node(
        &mut exposure_graph,
        PassInput::Pass(sum_id),
        atomic_ty.clone(),
    );
    let (count_in, _) = input_node(
        &mut exposure_graph,
        PassInput::Pass(count_id),
        atomic_ty.clone(),
    );
    let exposure = exposure_graph.add_node(Point::ORIGIN, AutoExposureNode);
    let (exposure_out, exposure_id) = output_node(
        &mut exposure_graph,
        PassOutput::Pass(PassOutputDef {
            name: "auto exposure".into(),
            ty: f32_ty.clone(),
        }),
        f32_ty.clone(),
    );
    exposure_graph.connect_slots_by_index(sum_in, 0, exposure, 0);
    exposure_graph.connect_slots_by_index(count_in, 0, exposure, 1);
    exposure_graph.connect_slots_by_index(exposure, 0, exposure_out, 0);

    let mut apply_graph = Graph::new(resources);
    let (target, target_apply_port) = input_node(
        &mut apply_graph,
        PassInput::Effect(target_input),
        layer_ty.clone(),
    );
    let (params, _) = input_node(
        &mut apply_graph,
        PassInput::Effect(params_input),
        params_ty.clone(),
    );
    let (texture, _) = input_node(
        &mut apply_graph,
        PassInput::Effect(texture_input),
        texture_ty.clone(),
    );
    let (exposure_in, _) = input_node(&mut apply_graph, PassInput::Pass(exposure_id), f32_ty);
    let dispatch = apply_graph.add_node(Point::ORIGIN, DispatchIndexNode);
    apply_graph.update_node_state::<DispatchIndexNode>(dispatch, |state| {
        state.cached_dispatch_strategy = EffectPassDispatchStrategy::EveryOutputLayerPixel(
            EffectPassOutputSlotId::new(Uuid::nil()),
        )
    });
    let ca = apply_graph.add_node(Point::ORIGIN, ChromaticAbberrationIntensityNode);
    let texture_amount = apply_graph.add_node(Point::ORIGIN, TextureIntensityNode);
    let apply = apply_graph.add_node(Point::ORIGIN, ApplyEffectNode);
    apply_graph.update_node_state::<ApplyEffectNode>(apply, |state| {
        state.target = target_apply_port;
    });
    let (layer_out, layer_out_id) = output_node(
        &mut apply_graph,
        PassOutput::Effect(layer_output),
        layer_ty.clone(),
    );
    apply_graph.update_node_state::<DispatchIndexNode>(dispatch, |state| {
        state.cached_dispatch_strategy =
            EffectPassDispatchStrategy::EveryOutputLayerPixel(layer_out_id)
    });
    apply_graph.connect_slots_by_index(params, 0, ca, 0);
    apply_graph.connect_slots_by_index(params, 0, texture_amount, 0);
    apply_graph.connect_slots_by_index(dispatch, 0, apply, 0);
    apply_graph.connect_slots_by_index(target, 1, apply, 1);
    apply_graph.connect_slots_by_index(ca, 0, apply, 2);
    apply_graph.connect_slots_by_index(texture_amount, 0, apply, 3);
    apply_graph.connect_slots_by_index(exposure_in, 0, apply, 4);
    apply_graph.connect_slots_by_index(texture, 0, apply, 5);
    apply_graph.connect_slots_by_index(apply, 0, layer_out, 0);
    apply_graph.connect_slots_by_index(target, 1, layer_out, 1);

    let mut passes = IndexMap::new();
    passes.insert(
        EffectPassId::new(Uuid::new_v4()),
        EffectPass {
            name: "Histogram".into(),
            graph: histogram_graph,
            dispatch_strategy: EffectPassDispatchStrategy::EveryInputLayerPixel(target_port),
        },
    );
    passes.insert(
        EffectPassId::new(Uuid::new_v4()),
        EffectPass {
            name: "Auto Exposure".into(),
            graph: exposure_graph,
            dispatch_strategy: EffectPassDispatchStrategy::Once,
        },
    );
    passes.insert(
        EffectPassId::new(Uuid::new_v4()),
        EffectPass {
            name: "Apply".into(),
            graph: apply_graph,
            dispatch_strategy: EffectPassDispatchStrategy::EveryOutputLayerPixel(layer_out_id),
        },
    );

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
            params_input,
            EffectInputSlot {
                name: "Parameters".into(),
                id: params_input,
                ty: params_ty,
            },
        ),
        (
            texture_input,
            EffectInputSlot {
                name: "Texture".into(),
                id: texture_input,
                ty: texture_ty,
            },
        ),
    ]);
    let outputs = IndexMap::from([
        (
            layer_output,
            EffectOutputSlot {
                name: "Layer".into(),
                id: layer_output,
                ty: layer_ty,
            },
        ),
        (
            histogram_output,
            EffectOutputSlot {
                name: "Histogram".into(),
                id: histogram_output,
                ty: histogram_ty,
            },
        ),
    ]);
    (
        EffectInstance {
            name: "Example Effect".into(),
            passes,
            inputs,
            outputs,
        },
        target_input,
        params_input,
        texture_input,
        layer_output,
        histogram_output,
    )
}

fn prepare_layer(
    image: image::DynamicImage,
    device: &Device,
    queue: &Queue,
) -> Result<GraphShaderLiteral> {
    let width = image.width();
    let height = image.height();
    let manager = GpuTileStorageInner::new(device.clone(), queue.clone());
    let layer_id = LayerId::new(Uuid::new_v4());
    manager.upload_image(layer_id, image);
    let ty = LayerType {
        texel_type: TexelType::RGBA8,
    };
    let mut prepared = ty.prepare_to_shader(&LayerReference, device, queue)?;
    prepared.storage = manager
        .get_layer(layer_id)
        .context("uploaded layer missing")?
        .deep_clone();
    queue.write_buffer(
        &prepared.bounds,
        0,
        bytemuck::cast_slice(&[0_i32, 0, width as i32, height as i32]),
    );
    Ok(GraphShaderLiteral::new_non_default(prepared, ty))
}

fn prepare_texture(image: GrayImage, device: &Device, queue: &Queue) -> GraphShaderLiteral {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("example grayscale texture"),
        size: wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::R8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        image.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(image.width()),
            rows_per_image: Some(image.height()),
        },
        texture.size(),
    );
    GraphShaderLiteral::new_non_default(
        texture.create_view(&TextureViewDescriptor::default()),
        TextureType {
            texel_type: TexelType::A8,
        },
    )
}

async fn read_histogram(
    value: &GraphShaderLiteral,
    device: &Device,
    queue: &Queue,
) -> Result<Vec<u32>> {
    let array = value
        .try_as_ref::<PreparedAtomicArray>()
        .context("histogram output type")?;
    let readback = device.create_buffer(&BufferDescriptor {
        label: Some("histogram readback"),
        size: u64::from(array.len) * 4,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(&array.buffer, 0, &readback, 0, u64::from(array.len) * 4);
    let submission = queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    let (sender, receiver) = futures::channel::oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device.poll_indefinitely_for(submission)?;
    receiver.await.context("histogram map callback dropped")??;
    let bytes = slice.get_mapped_range();
    let values = bytes
        .chunks_exact(4)
        .map(|x| u32::from_ne_bytes(x.try_into().unwrap()))
        .collect();
    drop(bytes);
    readback.unmap();
    Ok(values)
}

async fn save_layer(
    value: &GraphShaderLiteral,
    width: u32,
    height: u32,
    path: &str,
    device: &Device,
    queue: &Queue,
) -> Result<()> {
    let layer = value
        .try_as_ref::<PreparedLayer>()
        .context("layer output type")?;
    let tiles = layer.storage.iter_tile_indices().collect::<Vec<_>>();
    let data = layer.storage.readback(device, queue, tiles).await?;
    let mut output = RgbaImage::new(width, height);
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
    output.save(path)?;
    Ok(())
}

async fn execute(
    context: &RenderContext,
    outputs: HashMap<EffectOutputSlotId, GraphShaderLiteral>,
    layer_output: EffectOutputSlotId,
    histogram_output: EffectOutputSlotId,
    width: u32,
    height: u32,
    output_path: &str,
) -> Result<()> {
    let histogram =
        read_histogram(&outputs[&histogram_output], &context.device, &context.queue).await?;
    println!("histogram: {histogram:?}");
    save_layer(
        &outputs[&layer_output],
        width,
        height,
        output_path,
        &context.device,
        &context.queue,
    )
    .await
}

fn main() -> Result<()> {
    env_logger::init();
    let args = env::args().collect::<Vec<_>>();
    ensure!(
        args.len() == 5,
        "usage: example_effect_run <effect.lef> <input-image> <a8-texture> <output-image>"
    );
    let effect_path = Path::new(&args[1]);
    ensure!(
        effect_path.extension().and_then(|ext| ext.to_str())
            == Some(EffectAssetSerializer::file_extension()),
        "effect path must use the .{} extension",
        EffectAssetSerializer::file_extension()
    );

    let input_image = image::open(&args[2])?;
    let width = input_image.width();
    let height = input_image.height();
    let texture_image = image::open(&args[3])?.to_luma8();
    let context = RenderContext::default();
    let graph_resources = resources(ArrayAtomicU32Type {
        len: HISTOGRAM_SIZE,
    });
    let (generated, target_id, params_id, texture_id, layer_output, histogram_output) =
        generate_effect(graph_resources.clone());

    let serializer = EffectAssetSerializer;
    serializer
        .write(&generated.as_asset()?, &mut File::create(effect_path)?)
        .context("failed to save generated effect")?;
    let asset = serializer
        .read(&mut File::open(effect_path)?)
        .context("failed to load generated effect")?;
    let effect = EffectInstance::from_asset(&asset, graph_resources)?;
    let renderer =
        EffectRenderer::from_instance(&effect, context.device.clone(), context.queue.clone())?;
    let params_type = EffectParamType;
    let params = params_type.prepare_to_shader(
        &EffectParam {
            ca_intensity: 1.0,
            texture_intensity: 0.35,
        },
        &context.device,
        &context.queue,
    )?;
    let inputs = HashMap::from([
        (
            target_id,
            prepare_layer(input_image, &context.device, &context.queue)?,
        ),
        (
            params_id,
            GraphShaderLiteral::new_non_default(params, params_type),
        ),
        (
            texture_id,
            prepare_texture(texture_image, &context.device, &context.queue),
        ),
    ]);
    let outputs = renderer.run(inputs)?;
    block_on(execute(
        &context,
        outputs,
        layer_output,
        histogram_output,
        width,
        height,
        &args[4],
    ))
}
