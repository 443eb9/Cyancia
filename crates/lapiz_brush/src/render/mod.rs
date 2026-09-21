use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use bevy_math::IRect;
use chrono::{DateTime, Utc};
use encase::ShaderType;
use glam::{Vec2, Vec4};
use iced_runtime::Task;
use lapiz_canvas::{CanvasAppExt, CanvasId};
use lapiz_color::ForegroundBackgroundColorExt;
use lapiz_image::{
    composite::PixelPreviewOverrider,
    layer::{
        LayerId,
        properties::{LayerTexelTypePropertyExt, TexelSource},
    },
    scan_pixels::ScanPixelsPipeline,
    texel::TexelType,
    tile::{DynamicLayerStorage, LayerBinding, TileStorageAppExt},
};
use lapiz_input::mouse::PressedMouseState;
use lapiz_render::{
    buffer::DynamicBuffer,
    readback::{
        AsyncBufferReadback, create_readback_buffer_and_schedule_copy_buffer,
        readback_buffer_on_submit_async,
    },
};
use lapiz_runtime::Services;
use lapiz_shader_graph::{
    graph::{
        slot::{ErasedGraphValueType, GraphValueType},
        variable::GraphShaderLiteral,
    },
    wgsl_std::types::{LayerReference, LayerType, PreparedLayer},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use wgpu::{Buffer, BufferUsages, ComputePassDescriptor, Device, Queue};

use crate::{
    input_processing::{InputProcessor, RawPenInput},
    instance::{BrushPresetInstance, CompiledBrushPreset},
    render::{
        graph::{
            BRUSH_SAMPLE_BUILTIN, CANVAS_RESOURCES_BUILTIN, CanvasResources,
            ComputedPenInputValueType, HAS_SELECTION_BUILTIN, INITIAL_PEN_INPUT_BUILTIN,
            MAIN_ACCUMULATE_BUFFER, SELECTION_BUILTIN, TARGET_LAYER_BUILTIN,
        },
        pipeline::{BrushInputSamplingPipeline, PreparedInputSamplingPipelineData},
    },
};

pub mod graph;
pub mod pipeline;
pub mod stroke_preview;

pub const MAX_DABS_PER_STROKE: u32 = 256;

pub struct CanvasBrushStrokeSessionInfo {
    pub stroke_id: u64,
    pub stroke_begin: DateTime<Utc>,
    pub canvas_id: CanvasId,
    pub target_layer_id: LayerId,
    pub selection_layer_id: LayerId,
    pub target_layer_format: TexelType,
    pub selection_layer_format: TexelType,
}

pub struct CanvasBrushPresetOperator {
    instance: BrushPresetInstance,
    device: Device,
    queue: Queue,
    renderer: Option<BrushPresetRenderer>,
    session: Option<CanvasBrushStrokeSessionInfo>,
    input_processor: InputProcessor,
    canvas_resources: DynamicBuffer<CanvasResources>,
}

impl CanvasBrushPresetOperator {
    pub fn new(
        instance: BrushPresetInstance,
        device: Device,
        queue: Queue,
        input_processor: InputProcessor,
    ) -> Self {
        Self {
            instance,
            renderer: None,
            device,
            queue,
            session: None,
            input_processor,
            canvas_resources: DynamicBuffer::new(
                Some("canvas_resources".into()),
                BufferUsages::STORAGE,
            ),
        }
    }

    pub fn instance(&self) -> &BrushPresetInstance {
        &self.instance
    }

    pub fn instance_mut(&mut self) -> &mut BrushPresetInstance {
        self.renderer = None;
        &mut self.instance
    }

    pub fn begin_stroke(
        &mut self,
        input: &PressedMouseState,
        stroke_id: u64,
        canvas_id: CanvasId,
        services: &mut Services,
    ) -> Task<()> {
        let canvas = services
            .canvas(&canvas_id)
            .expect("Current canvas should exist");
        let position = canvas
            .transform
            .window_to_pixel(Vec2::new(input.position.x, input.position.y));
        let active_layer_id = canvas.active_layer_id();
        let selection_layer_id = canvas.image.selection_layer();
        if !canvas
            .active_layer_node()
            .properties()
            .get_texel_prop()
            .is_some_and(|property| property.source == TexelSource::DirectlyDefined)
        {
            log::warn!("Unable to paint to the active layer which cannot contain pixels.");
            return Task::none();
        }

        let xyz_to_rgb = canvas
            .image
            .profile()
            .rgb_to_xyz_matrix()
            .to_f32()
            .inverse();
        let foreground = services.foreground_color().get().into_rgb(xyz_to_rgb);
        let background = services.background_color().get().into_rgb(xyz_to_rgb);
        self.canvas_resources.clear();
        self.canvas_resources.push(&CanvasResources {
            foreground_color: Vec4::new(foreground.r, foreground.g, foreground.b, 1.0),
            background_color: Vec4::new(background.r, background.g, background.b, 1.0),
        });
        self.canvas_resources
            .write_buffer(&self.device, &self.queue);

        let tiles = services.tile_storage();
        let target_layer_info = tiles
            .get_layer_info(active_layer_id)
            .expect("Active pixel layer should have GPU storage");
        let selection_layer_info = tiles
            .get_layer_info(selection_layer_id)
            .expect("Selection layer should have GPU storage");
        let session = CanvasBrushStrokeSessionInfo {
            stroke_id,
            stroke_begin: Utc::now(),
            canvas_id,
            target_layer_id: active_layer_id,
            selection_layer_id,
            target_layer_format: target_layer_info.texel_type,
            selection_layer_format: selection_layer_info.texel_type,
        };
        if self.session.as_ref().is_some_and(|previous| {
            previous.target_layer_format != session.target_layer_format
                || previous.selection_layer_format != session.selection_layer_format
        }) {
            self.renderer = None;
        }

        if self.renderer.is_none() {
            let compiled = self
                .instance
                .compile(
                    session.target_layer_format,
                    session.selection_layer_format,
                    &self.device,
                    &self.queue,
                )
                .expect("Failed to compile brush preset");
            self.renderer = Some(BrushPresetRenderer::new(
                compiled,
                session.target_layer_format,
                session.selection_layer_format,
                &self.device,
                &self.queue,
                &self.canvas_resources,
            ));
        }
        let renderer = self.renderer.as_mut().unwrap();

        self.input_processor.reset();
        let tiles = services.tile_storage();
        let target_layer = tiles
            .get_layer_binding_or_empty(session.target_layer_id)
            .expect("Failed to bind active pixel layer");
        let selection_layer = tiles
            .get_layer_binding_or_empty(session.selection_layer_id)
            .expect("Failed to bind selection layer");
        renderer.begin(&self.device, &self.queue, target_layer, selection_layer);

        let sample =
            self.input_processor
                .push(RawPenInput::new(position, session.stroke_begin, input));
        let task = sample.map(|sample| renderer.update(&self.device, &self.queue, sample));
        self.session = Some(session);
        task.unwrap_or_else(Task::none).discard()
    }

    pub fn update_stroke(&mut self, input: &PressedMouseState, services: &Services) -> Task<()> {
        let (Some(renderer), Some(session)) = (&mut self.renderer, &self.session) else {
            return Task::none();
        };
        let canvas = services.canvas(&session.canvas_id).unwrap();
        let position = canvas
            .transform
            .window_to_pixel(Vec2::new(input.position.x, input.position.y));
        let Some(sample) =
            self.input_processor
                .push(RawPenInput::new(position, session.stroke_begin, input))
        else {
            return Task::none();
        };
        renderer.update(&self.device, &self.queue, sample).discard()
    }

    pub fn end_stroke(
        &mut self,
        input: &PressedMouseState,
        services: &mut Services,
    ) -> Task<BrushStrokeResult> {
        let (Some(renderer), Some(session)) = (&mut self.renderer, self.session.take()) else {
            return Task::none();
        };
        let canvas = services
            .canvas(&session.canvas_id)
            .expect("Stroke canvas should exist");
        let position = canvas
            .transform
            .window_to_pixel(Vec2::new(input.position.x, input.position.y));
        let updates = self
            .input_processor
            .flush(RawPenInput::new(position, session.stroke_begin, input))
            .into_iter()
            .map(|sample| renderer.update(&self.device, &self.queue, sample));
        let updates = Task::batch(updates).discard();
        let end = renderer.end().map(move |result| BrushStrokeResult {
            stroke_id: session.stroke_id,
            canvas_id: session.canvas_id,
            target_layer_id: session.target_layer_id,
            result,
        });
        updates.chain(end)
    }

    pub fn preview(&mut self) -> Task<Option<BrushStrokePreview>> {
        let (Some(renderer), Some(session)) = (&mut self.renderer, &self.session) else {
            return Task::done(None);
        };
        let stroke_id = session.stroke_id;
        let canvas_id = session.canvas_id;
        let target_layer_id = session.target_layer_id;
        renderer.generate_preview().map(move |result| {
            let result = result.filter(|result| !result.is_empty())?;
            Some(BrushStrokePreview {
                stroke_id,
                canvas_id,
                target_layer_id,
                overrider: PixelPreviewOverrider::from_layer_storage(&result),
                dirty_tiles: result.compute_tile_bounds(),
            })
        })
    }
}

struct StrokeSession {
    shared: Arc<Mutex<BrushEffectState>>,
    resource_group: wgpu::BindGroup,
    pen_input: DynamicBuffer<PenInput>,
    output_samples: DynamicBuffer<OutputSamples>,
    input_sample_prepared: PreparedInputSamplingPipelineData,
}

pub struct BrushStrokePreview {
    pub stroke_id: u64,
    pub canvas_id: CanvasId,
    pub target_layer_id: LayerId,
    pub overrider: PixelPreviewOverrider,
    pub dirty_tiles: IRect,
}

pub struct BrushStrokeResult {
    pub stroke_id: u64,
    pub canvas_id: CanvasId,
    pub target_layer_id: LayerId,
    pub result: DynamicLayerStorage,
}

pub struct BrushPresetRenderer {
    input_sample: BrushInputSamplingPipeline,
    resources: StrokeResources,
    scan_pixels: ScanPixelsPipeline,
    input_sampler: DynamicBuffer<InputSampler>,
    compiled: Arc<CompiledBrushPreset>,
    session: Option<StrokeSession>,
}

impl BrushPresetRenderer {
    pub fn new(
        brush: CompiledBrushPreset,
        target_layer_format: TexelType,
        selection_layer_format: TexelType,
        device: &Device,
        queue: &Queue,
        canvas_resources: &DynamicBuffer<CanvasResources>,
    ) -> Self {
        let resources = StrokeResources::new(
            device,
            queue,
            &brush,
            target_layer_format,
            selection_layer_format,
            canvas_resources,
        );
        let input_sample = BrushInputSamplingPipeline::new(
            device,
            &resources.resource_layout,
            brush.spacing.clone().into(),
        );
        let mut input_sampler =
            DynamicBuffer::new(Some("input sampler".into()), BufferUsages::STORAGE);
        input_sampler.push(&InputSampler::default());
        input_sampler.write_buffer(device, queue);
        Self {
            input_sample,
            resources,
            scan_pixels: ScanPixelsPipeline::new(device, selection_layer_format),
            input_sampler,
            compiled: Arc::new(brush),
            session: None,
        }
    }

    pub fn begin(
        &mut self,
        device: &Device,
        queue: &Queue,
        target_layer: LayerBinding,
        selection_layer: LayerBinding,
    ) {
        self.input_sampler.clear();
        self.input_sampler.push(&InputSampler::default());
        self.input_sampler.write_buffer(device, queue);

        let mut initial_pen_input = DynamicBuffer::new(
            Some("initial pen input".into()),
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
        );
        initial_pen_input.push(&ComputedPenInput::default());
        initial_pen_input.write_buffer(device, queue);

        let has_selection = self
            .scan_pixels
            .scan_to_binary_buffer(device, queue, &selection_layer);
        let mut pen_input = DynamicBuffer::new(Some("pen input".into()), BufferUsages::STORAGE);
        pen_input.push(&PenInput::default());
        pen_input.write_buffer(device, queue);
        let mut output_samples = DynamicBuffer::new(
            Some("output samples".into()),
            BufferUsages::COPY_SRC | BufferUsages::STORAGE,
        );
        output_samples.push(&OutputSamples::new(MAX_DABS_PER_STROKE));
        output_samples.write_buffer(device, queue);

        let input_sample_prepared = self.input_sample.prepare(
            device,
            &pen_input,
            &self.input_sampler,
            &output_samples,
            &initial_pen_input,
        );
        let resource_group = self.resources.resource_bind_group(
            device,
            &BuiltinHostValues {
                canvas_resources: &self.resources.canvas_resources,
                has_selection: &has_selection,
                selection: &selection_layer,
                target_layer: &target_layer,
            },
        );

        let compiled = self.compiled.clone();
        let empty_accumulator =
            prepare_empty_layer(self.resources.target_layer_format, device, queue)
                .expect("failed to prepare brush accumulation layer");
        self.session = Some(StrokeSession {
            shared: Arc::new(Mutex::new(BrushEffectState {
                compiled,
                target_layer,
                selection_layer,
                has_selection,
                canvas_resources: self.resources.canvas_resources.clone(),
                accumulator: Some(empty_accumulator),
                initial_sample: None,
                device: device.clone(),
                queue: queue.clone(),
                target_layer_format: self.resources.target_layer_format,
                selection_layer_format: self.resources.selection_layer_format,
            })),
            resource_group,
            pen_input,
            output_samples,
            input_sample_prepared,
        });
    }

    pub fn update(&mut self, device: &Device, queue: &Queue, input: PenInput) -> Task<Result<()>> {
        let Some(session) = &mut self.session else {
            return Task::none();
        };
        session.pen_input.clear();
        session.pen_input.push(&input);
        session.pen_input.write_buffer(device, queue);

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor::default());
            self.input_sample.dispatch(
                &mut pass,
                &session.input_sample_prepared,
                &session.resource_group,
            );
        }
        let staging = create_readback_buffer_and_schedule_copy_buffer(
            device,
            &mut encoder,
            session.output_samples.inner_buffer().unwrap(),
        );
        let readback = readback_buffer_on_submit_async(&mut encoder, &staging, ..);
        queue.submit([encoder.finish()]);
        let shared = session.shared.clone();
        Task::future(async move {
            let result = run_main_effects(shared, readback).await;
            if let Err(error) = &result {
                log::error!("Brush main effects failed: {error:#}");
            }
            result
        })
    }

    pub fn end(&mut self) -> Task<DynamicLayerStorage> {
        let Some(session) = self.session.take() else {
            return Task::none();
        };
        let shared = session.shared;
        Task::future(async move {
            let mut state = shared.lock();
            let result = run_postprocess(&mut state).expect("brush postprocess failed");
            result.downcast::<PreparedLayer>().storage
        })
    }

    pub fn generate_preview(&mut self) -> Task<Option<DynamicLayerStorage>> {
        let Some(session) = &self.session else {
            return Task::done(None);
        };
        let shared = session.shared.clone();
        Task::future(async move {
            let mut state = shared.lock();
            let Some(accumulator) = state.accumulator.as_ref() else {
                return None;
            };
            let ty = accumulator.ty().clone();
            let accumulator = accumulator.as_ref::<PreparedLayer>();
            if accumulator.pixel_bounds.is_empty() {
                return None;
            }
            let prepared = accumulator.deep_clone();
            let original = state
                .accumulator
                .replace(GraphShaderLiteral::new_boxed(Box::new(prepared), ty));
            let result = run_postprocess(&mut state).ok();
            state.accumulator = original;
            result.map(|literal| literal.downcast::<PreparedLayer>().storage)
        })
    }
}

async fn run_main_effects(
    shared: Arc<Mutex<BrushEffectState>>,
    readback: AsyncBufferReadback<OutputSamples>,
) -> Result<()> {
    let samples = readback.into_inner().await??;
    let mut state = shared.lock();
    for sample in samples.samples.into_iter().take(samples.n_samples as usize) {
        if state.initial_sample.is_none() {
            state.initial_sample = Some(sample);
        }
        let accumulator = state
            .accumulator
            .take()
            .context("missing brush accumulator")?;
        let builtins = main_builtins(&state, sample, accumulator)?;
        let mut outputs = state
            .compiled
            .main
            .run(&state.compiled.main_inputs, builtins)
            .context("main brush effect failed")?;
        state.accumulator = Some(
            outputs
                .remove(&state.compiled.main_output)
                .context("main effect did not produce its accumulation output")?,
        );
    }
    Ok(())
}

fn run_postprocess(state: &mut BrushEffectState) -> Result<GraphShaderLiteral> {
    let accumulator = state
        .accumulator
        .take()
        .context("missing brush accumulator")?;
    let builtins = postprocess_builtins(state, accumulator)?;
    let mut outputs = state
        .compiled
        .postprocess
        .run(&state.compiled.postprocess_inputs, builtins)?;
    outputs
        .remove(&state.compiled.postprocess_output)
        .context("postprocess effect did not produce the stroke result")
}

struct BrushEffectState {
    compiled: Arc<CompiledBrushPreset>,
    target_layer: LayerBinding,
    selection_layer: LayerBinding,
    has_selection: Buffer,
    canvas_resources: Buffer,
    accumulator: Option<GraphShaderLiteral>,
    initial_sample: Option<ComputedPenInput>,
    device: Device,
    queue: Queue,
    target_layer_format: TexelType,
    selection_layer_format: TexelType,
}

fn main_builtins(
    state: &BrushEffectState,
    sample: ComputedPenInput,
    accumulator: GraphShaderLiteral,
) -> Result<HashMap<String, GraphShaderLiteral>> {
    let mut values = base_builtins(state);
    values.insert(
        BRUSH_SAMPLE_BUILTIN.into(),
        prepared_literal(
            Arc::new(ComputedPenInputValueType),
            &sample,
            &state.device,
            &state.queue,
        )?,
    );
    values.insert(
        INITIAL_PEN_INPUT_BUILTIN.into(),
        prepared_literal(
            Arc::new(ComputedPenInputValueType),
            state.initial_sample.as_ref().unwrap_or(&sample),
            &state.device,
            &state.queue,
        )?,
    );
    values.insert(MAIN_ACCUMULATE_BUFFER.into(), accumulator);
    Ok(values)
}

fn postprocess_builtins(
    state: &BrushEffectState,
    accumulator: GraphShaderLiteral,
) -> Result<HashMap<String, GraphShaderLiteral>> {
    let mut values = base_builtins(state);
    values.insert(MAIN_ACCUMULATE_BUFFER.into(), accumulator);
    Ok(values)
}

fn base_builtins(state: &BrushEffectState) -> HashMap<String, GraphShaderLiteral> {
    let types = graph::brush_builtin_types(state.target_layer_format, state.selection_layer_format);
    let mut values = HashMap::new();
    for (name, ty) in types {
        let value: Box<dyn lapiz_shader_graph::graph::variable::GraphShaderLiteralValue> =
            match name.as_str() {
                CANVAS_RESOURCES_BUILTIN => Box::new(state.canvas_resources.clone()),
                HAS_SELECTION_BUILTIN => Box::new(state.has_selection.clone()),
                SELECTION_BUILTIN => Box::new(state.selection_layer.clone()),
                TARGET_LAYER_BUILTIN => Box::new(state.target_layer.clone()),
                _ => unreachable!(),
            };
        values.insert(name, GraphShaderLiteral::new_boxed(value, ty));
    }
    values
}

fn prepared_literal<T: GraphValueType>(
    ty: Arc<T>,
    value: &T::AssociatedLiteralType,
    device: &Device,
    queue: &Queue,
) -> Result<GraphShaderLiteral> {
    let prepared = ty.prepare_to_shader(value, device, queue)?;
    Ok(GraphShaderLiteral::new_boxed(Box::new(prepared), ty))
}

fn prepare_empty_layer(
    texel_type: TexelType,
    device: &Device,
    queue: &Queue,
) -> Result<GraphShaderLiteral> {
    let ty = Arc::new(LayerType { texel_type });
    prepared_literal(ty, &LayerReference, device, queue)
}

#[derive(ShaderType, Debug, Clone)]
pub struct OutputSamples {
    pub n_samples: u32,
    pub is_overflow: u32,
    #[shader(size(runtime))]
    pub samples: Vec<ComputedPenInput>,
}

impl OutputSamples {
    pub fn new(max_samples: u32) -> Self {
        Self {
            n_samples: 0,
            is_overflow: 0,
            samples: vec![ComputedPenInput::default(); max_samples as usize],
        }
    }
}

#[derive(ShaderType, Debug, Default, Clone, Copy)]
pub struct InputSampler {
    pub last_input: PenInput,
    pub last_sample: ComputedPenInput,
    pub has_last_sample: u32,
    pub has_initial_input: u32,
    pub next_dab_index: u32,
    pub distance_to_next_dab: f32,
    pub stroke_distance: f32,
}

#[derive(ShaderType, Debug, Default, Clone, Copy)]
pub struct PenInput {
    pub position: Vec2,
    pub tilt: Vec2,
    pub angle: Vec2,
    pub pressure: f32,
    pub time: Time,
    pub bezier_control_prev: Vec2,
    pub bezier_control_next: Vec2,
}

#[derive(ShaderType, Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct ComputedPenInput {
    pub position: Vec2,
    pub draw_direction_vec: Vec2,
    pub tilt: Vec2,
    pub angle: Vec2,
    pub draw_direction_angle: f32,
    pub pressure: f32,
    pub dab_index: u32,
    pub stroke_distance: f32,
    pub time: Time,
}

#[derive(ShaderType, Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Time {
    pub now: f32,
    pub stroke_begin: f32,
}

pub struct BuiltinHostValues<'a> {
    pub canvas_resources: &'a Buffer,
    pub has_selection: &'a Buffer,
    pub selection: &'a LayerBinding,
    pub target_layer: &'a LayerBinding,
}

pub struct StrokeResources {
    resource_layout: wgpu::BindGroupLayout,
    builtin_types: std::collections::BTreeMap<String, Arc<dyn ErasedGraphValueType>>,
    parameters: Arc<[lapiz_shader_graph::graph::variable::GraphLiteral]>,
    prepared_parameters: Vec<Box<dyn lapiz_shader_graph::graph::variable::GraphShaderLiteralValue>>,
    canvas_resources: Buffer,
    target_layer_format: TexelType,
    selection_layer_format: TexelType,
}

impl StrokeResources {
    fn new(
        device: &Device,
        queue: &Queue,
        brush: &CompiledBrushPreset,
        target_layer_format: TexelType,
        selection_layer_format: TexelType,
        canvas_resources: &DynamicBuffer<CanvasResources>,
    ) -> Self {
        let prepared_parameters = brush
            .spacing_parameters
            .iter()
            .map(|parameter| {
                parameter
                    .ty()
                    .prepare_to_shader(parameter.value(), device, queue)
                    .expect("failed to prepare spacing parameter")
            })
            .collect();
        Self {
            resource_layout: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("brush spacing resources"),
                entries: &brush.spacing_resource_layouts,
            }),
            builtin_types: brush.spacing_builtin_types.clone(),
            parameters: brush.spacing_parameters.clone(),
            prepared_parameters,
            canvas_resources: canvas_resources.inner_buffer().unwrap().clone(),
            target_layer_format,
            selection_layer_format,
        }
    }

    fn resource_bind_group(
        &self,
        device: &Device,
        builtins: &BuiltinHostValues<'_>,
    ) -> wgpu::BindGroup {
        let mut binding = 0;
        let mut entries = Vec::new();
        for (name, ty) in &self.builtin_types {
            let value: &dyn lapiz_shader_graph::graph::variable::GraphShaderLiteralValue =
                match name.as_str() {
                    CANVAS_RESOURCES_BUILTIN => builtins.canvas_resources,
                    HAS_SELECTION_BUILTIN => builtins.has_selection,
                    SELECTION_BUILTIN => builtins.selection,
                    TARGET_LAYER_BUILTIN => builtins.target_layer,
                    _ => unreachable!(),
                };
            let (next, bound) = ty
                .push_shader_binding(
                    lapiz_shader_graph::graph::slot::GraphShaderStage::Input,
                    value,
                    binding,
                    lapiz_render::bind_group_entries::DynamicBindGroupEntries::new(),
                )
                .expect("failed to bind spacing builtin");
            binding = next;
            entries.extend(bound.to_vec());
        }
        for (parameter, prepared) in self.parameters.iter().zip(&self.prepared_parameters) {
            let (next, bound) = parameter
                .ty()
                .push_shader_binding(
                    lapiz_shader_graph::graph::slot::GraphShaderStage::Input,
                    prepared.as_ref(),
                    binding,
                    lapiz_render::bind_group_entries::DynamicBindGroupEntries::new(),
                )
                .expect("failed to bind spacing parameter");
            binding = next;
            entries.extend(bound.to_vec());
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("brush spacing resources"),
            layout: &self.resource_layout,
            entries: &entries,
        })
    }
}
