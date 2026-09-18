//! End-to-end effect execution test.
//!
//! Builds a three-pass effect (histogram, auto exposure, apply), round trips it
//! through the `.lef` asset format, runs it on the GPU, and compares the
//! histogram and output layer against a CPU reference implementation of the
//! same shader semantics.

use std::{collections::HashMap, io::Cursor, sync::Arc};

use anyhow::{Context, Result};
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
use wesl::syntax::*;
use wesl_quote::{quote_declaration, quote_statement};
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
        let struct_decl = quote_declaration! {
            struct EffectParam { ca_intensity: f32, texture_intensity: f32 }
        };
        let name_ident = Ident::new(name.to_string());
        let var_decl = if stage == GraphShaderStage::Input {
            quote_declaration! {
                @group(#group) @binding(#binding) var<storage, read> #name_ident: EffectParam;
            }
        } else {
            quote_declaration! {
                @group(#group) @binding(#binding) var<storage, read_write> #name_ident: EffectParam;
            }
        };
        shader.push_str(&format!("{struct_decl}\n{var_decl}\n"));
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
    fn wgsl_type_name(&self) -> Option<&'static str> {
        None
    }
    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(EffectParamType)
    }
    fn view_literal(&self, _data: &EffectParam) -> Element<'static, (), GraphTheme, GraphRenderer> {
        Void.into()
    }
    fn update_literal(&self, _data: &mut EffectParam, _message: ()) {}
    fn literal_to_code(&self, _data: &EffectParam) -> Option<Expression> {
        None
    }
}

macro_rules! parameter_node {
    ($name:ident, $id:literal, $field:ident) => {
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
                Ok(quote_statement! { let #output = #input.$field; }.to_string())
            }
        }
    };
}

parameter_node!(
    ChromaticAbberrationIntensityNode,
    "Chromatic Abberration Intensity",
    ca_intensity
);
parameter_node!(
    TextureIntensityNode,
    "Texture Intensity",
    texture_intensity
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
        Ok([
            quote_statement! {
                let example_inside = all(vec2f(dispatch_index) >= (#bounds).min) && all(vec2f(dispatch_index) < (#bounds).max);
            },
            quote_statement! {
                let example_luminance = clamp(dot((#color).rgb, vec3f(0.2126, 0.7152, 0.0722)), 0.0, 1.0);
            },
            quote_statement! { let #bin = i32(round(example_luminance * 255.0)); },
            quote_statement! { let #increment = select(0u, 1u, example_inside); },
            quote_statement! { let #sum = select(0u, u32(round(example_luminance * 65535.0)), example_inside); },
            quote_statement! { let #count = #increment; },
        ]
        .iter()
        .map(|statement| statement.to_string())
        .collect::<Vec<_>>()
        .join("\n"))
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
        Ok([
            quote_statement! {
                let example_average_luminance = f32(#sum) / max(f32(#count) * 65535.0, 1.0);
            },
            quote_statement! {
                let #exposure = clamp(0.18 / max(example_average_luminance, 0.001), 0.25, 4.0);
            },
        ]
        .iter()
        .map(|statement| statement.to_string())
        .collect::<Vec<_>>()
        .join("\n"))
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
        let target = Ident::new(layer_load_ident(&lapiz_effect::render::pass_input_ident(state.target)));
        Ok([
            quote_statement! { let example_shift = max(i32(round(abs(#ca) * 3.0)), 0); },
            quote_statement! { let example_center = #target(#pixel); },
            quote_statement! { let example_r = #target(#pixel + vec2i(example_shift, 0)).r; },
            quote_statement! { let example_b = #target(#pixel - vec2i(example_shift, 0)).b; },
            quote_statement! { let example_tex_size = vec2i(textureDimensions(#texture)); },
            quote_statement! { let example_tex_coord = vec2u(((#pixel % example_tex_size) + example_tex_size) % example_tex_size); },
            quote_statement! { let example_gray = textureLoad(#texture, example_tex_coord, 0).r; },
            quote_statement! { let example_modulation = mix(1.0, example_gray, clamp(#texture_intensity, 0.0, 1.0)); },
            quote_statement! {
                let #output = vec4f(vec3f(example_r, example_center.g, example_b) * #exposure * example_modulation, example_center.a);
            },
        ]
        .iter()
        .map(|statement| statement.to_string())
        .collect::<Vec<_>>()
        .join("\n"))
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

async fn readback_layer(
    value: &GraphShaderLiteral,
    width: u32,
    height: u32,
    device: &Device,
    queue: &Queue,
) -> Result<RgbaImage> {
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

const W: u32 = 320;
const H: u32 = 208;
const TEX_W: u32 = 96;
const TEX_H: u32 = 160;

// Deterministic integer hash so every channel value is reproducible without fixtures.
fn channel_hash(x: u32, y: u32, salt: u32) -> u8 {
    let mut v =
        x.wrapping_mul(0x9E3779B1) ^ y.wrapping_mul(0x85EBCA77) ^ salt.wrapping_mul(0xC2B2AE3D);
    v ^= v >> 15;
    v = v.wrapping_mul(0x2545F491);
    v ^= v >> 13;
    (v >> 24) as u8
}

fn test_images() -> (image::DynamicImage, GrayImage) {
    let image = image::RgbaImage::from_fn(W, H, |x, y| {
        image::Rgba([
            channel_hash(x, y, 1),
            channel_hash(x, y, 2),
            channel_hash(x, y, 3),
            255,
        ])
    });
    let texture = GrayImage::from_fn(TEX_W, TEX_H, |x, y| image::Luma([channel_hash(x, y, 4)]));
    (image::DynamicImage::ImageRgba8(image), texture)
}

fn reference_luminance(r: u8, g: u8, b: u8) -> f32 {
    (f32::from(r) / 255.0 * 0.2126 + f32::from(g) / 255.0 * 0.7152 + f32::from(b) / 255.0 * 0.0722)
        .clamp(0.0, 1.0)
}

fn reference_exposure(input: &image::DynamicImage) -> f32 {
    let mut sum: u64 = 0;
    let pixels = input.as_rgba8().unwrap();
    for (_, _, pixel) in pixels.enumerate_pixels() {
        let luminance = reference_luminance(pixel[0], pixel[1], pixel[2]);
        sum += u64::from((luminance * 65535.0).round() as u32);
    }
    let count = u64::from(W * H);
    let average = sum as f32 / (count as f32 * 65535.0).max(1.0);
    (0.18 / average.max(0.001)).clamp(0.25, 4.0)
}

// Runtime-sized array buffers must follow WGSL strides: a vec3 element keeps
// 12 bytes of data in a 16-byte slot, so the allocation is len * 16.
#[test]
fn array_buffers_use_wgsl_strides() {
    if !gpu_available() {
        eprintln!("skipping: no wgpu adapter available");
        return;
    }

    let context = RenderContext::default();
    for (name, element, stride) in [
        (
            "f32",
            Arc::new(F32Type) as Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
            4_u64,
        ),
        ("vec2f", Arc::new(Vec2FType), 8),
        ("vec3f", Arc::new(Vec3FType), 16),
        ("vec4f", Arc::new(Vec4FType), 16),
    ] {
        let len = 10;
        let array = ArrayType {
            element_type: element,
            len,
        };
        let prepared = array
            .prepare_to_shader(&ArrayLiteral, &context.device, &context.queue)
            .unwrap();
        assert_eq!(
            prepared.buffer.size(),
            len as u64 * stride,
            "array<{name}> buffer must allocate len * stride"
        );
    }
}

#[test]
fn effect_matches_cpu_reference() {
    if !gpu_available() {
        eprintln!("skipping: no wgpu adapter available");
        return;
    }

    let (input, texture) = test_images();
    let context = RenderContext::default();
    let graph_resources = resources(ArrayAtomicU32Type {
        len: HISTOGRAM_SIZE,
    });
    let (generated, target_id, params_id, texture_id, layer_output, histogram_output) =
        generate_effect(graph_resources.clone());

    // Regression guard for asset round trips: the graph must survive serialization.
    let serializer = EffectAssetSerializer;
    let mut encoded = Vec::new();
    serializer
        .write(&generated.as_asset().unwrap(), &mut encoded)
        .unwrap();
    let asset = serializer.read(&mut Cursor::new(encoded)).unwrap();
    let effect = EffectInstance::from_asset(&asset, graph_resources).unwrap();

    let renderer =
        EffectRenderer::from_instance(&effect, context.device.clone(), context.queue.clone())
            .unwrap();
    let params_type = EffectParamType;
    let params = params_type
        .prepare_to_shader(
            &EffectParam {
                ca_intensity: 1.0,
                texture_intensity: 0.35,
            },
            &context.device,
            &context.queue,
        )
        .unwrap();
    let inputs = HashMap::from([
        (
            target_id,
            prepare_layer(input.clone(), &context.device, &context.queue).unwrap(),
        ),
        (
            params_id,
            GraphShaderLiteral::new_non_default(params, params_type),
        ),
        (
            texture_id,
            prepare_texture(texture.clone(), &context.device, &context.queue),
        ),
    ]);
    let outputs = renderer.run(&inputs).unwrap();

    let histogram = block_on(read_histogram(
        &outputs[&histogram_output],
        &context.device,
        &context.queue,
    ))
    .unwrap();
    let result = block_on(readback_layer(
        &outputs[&layer_output],
        W,
        H,
        &context.device,
        &context.queue,
    ))
    .unwrap();

    // Pass 1: every pixel lands in exactly one histogram bin inside the bounds.
    let mut expected_histogram = [0u32; HISTOGRAM_SIZE as usize];
    for (_, _, pixel) in input.as_rgba8().unwrap().enumerate_pixels() {
        let luminance = reference_luminance(pixel[0], pixel[1], pixel[2]);
        let bin = (luminance * 255.0 + 0.5).floor() as usize;
        expected_histogram[bin.min(HISTOGRAM_SIZE as usize - 1)] += 1;
    }
    assert_eq!(
        histogram.iter().sum::<u32>(),
        W * H,
        "histogram must count every pixel exactly once"
    );
    let displacement: u32 = histogram
        .iter()
        .zip(expected_histogram.iter())
        .map(|(gpu, expected)| gpu.abs_diff(*expected))
        .sum();
    assert!(
        displacement <= 32,
        "histogram diverged from CPU reference (displacement {displacement}): {histogram:?}"
    );

    // Passes 2 + 3: exposure driven chromatic aberration with texture modulation.
    let exposure = reference_exposure(&input);
    eprintln!("reference exposure: {exposure}");
    let input = input.as_rgba8().unwrap();
    let shift = 3_i32;
    let mut max_delta = 0_i32;
    let mut mismatched_channels = 0_u32;
    let mut alpha_delta = 0_i32;
    for (x, y, pixel) in result.enumerate_pixels() {
        let load = |dx: i32, channel: usize| -> f32 {
            let px = x as i32 + dx;
            if px < 0 || px >= W as i32 {
                0.0
            } else {
                f32::from(input.get_pixel(px as u32, y).0[channel]) / 255.0
            }
        };
        let gray = f32::from(texture.get_pixel(x % TEX_W, y % TEX_H).0[0]) / 255.0;
        let modulation = 1.0 + (gray - 1.0) * 0.35;
        let center = input.get_pixel(x, y);
        let expected = [
            load(shift, 0) * exposure * modulation,
            f32::from(center.0[1]) / 255.0 * exposure * modulation,
            load(-shift, 2) * exposure * modulation,
        ];
        for (expected_channel, result_channel) in expected.iter().zip(pixel.0.iter().take(3)) {
            let expected_byte = ((expected_channel * 255.0).clamp(0.0, 255.0) as u32) as u8;
            let delta = (i32::from(expected_byte) - i32::from(*result_channel)).abs();
            max_delta = max_delta.max(delta);
            mismatched_channels += u32::from(delta > 0);
        }
        alpha_delta = alpha_delta.max((i32::from(center.0[3]) - i32::from(pixel.0[3])).abs());
    }
    assert_eq!(alpha_delta, 0, "alpha must pass through untouched");
    assert!(
        max_delta <= 1,
        "color channels diverged from CPU reference by {max_delta}"
    );
    assert!(
        mismatched_channels <= W * H * 3 / 100,
        "{mismatched_channels} channels differ from the CPU reference"
    );
}
