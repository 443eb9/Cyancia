//! End-to-end brush test: builds a hard round brush from three effects
//! (spacing / main / post process), round trips it through the `.lapiz`
//! preset asset, then renders a stroke preview through the real brush
//! pipeline and checks that the dabbed stroke landed on the image.

use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use futures::executor::block_on;
use iced_core::Point;
use indexmap::IndexMap;
use lapiz_assets::loader::{AssetRegistryBuilder, AssetSerializer};
use lapiz_brush::{
    asset::{BrushPreset, BrushPresetMetadata, BrushPresetSerializer, SerializableBrushParameter},
    instance::BrushPresetInstance,
    render::graph::{
        DAB_BOUNDS_OUTPUT, DAB_COLOR_OUTPUT, SPACING_OUTPUT, STROKE_BOUNDS_OUTPUT,
        STROKE_COLOR_OUTPUT,
    },
    render::stroke_preview::{
        create_stroke_preview_with, predefined_curve_samples,
    },
};
use lapiz_effect::{
    asset::{
        EffectAssetSerializer, EffectInputSlotId, EffectOutputSlotId, EffectPassDispatchStrategy,
        EffectPassId,
    },
    instance::{EffectInputSlot, EffectOutputSlot, EffectInstance, EffectPass},
    nodes::{PassInput, PassInputNode, PassOutput, PassOutputNode},
};
use lapiz_render::util::DevicePollExt as _;
use lapiz_runtime::renderer::RenderContext;
use lapiz_shader_graph::{
    graph::{Graph, variable::GraphLiteral},
    save::SerializableGraphLiteral,
    wgsl_std::nodes::ScalarMathNodeMode,
};
use uuid::Uuid;

const SPACING_PARAMETER: f32 = 10.0;
const RADIUS_PARAMETER: f32 = 14.0;

fn spacing_effect(assets: lapiz_assets::store::AssetRegistry) -> Result<(EffectInstance, EffectInputSlotId)> {
    let resources = lapiz_brush::instance::spacing_effect_resources(assets.clone());
    let spacing_parameter = EffectInputSlotId::new(Uuid::new_v4());
    let spacing_output = EffectOutputSlotId::new(Uuid::new_v4());

    let mut graph = Graph::new(resources.clone());
    let input = graph.add_node(Point::ORIGIN, PassInputNode);
    graph.update_node_state::<PassInputNode>(input, |state| {
        state.input = Some(PassInput::Effect(spacing_parameter));
        state.cached_ty = Some(Arc::new(lapiz_shader_graph::wgsl_std::types::F32Type));
    });
    let output = graph.add_node(Point::ORIGIN, PassOutputNode);
    graph.update_node_state::<PassOutputNode>(output, |state| {
        state.output = Some(PassOutput::Effect(spacing_output));
        state.cached_ty = Some(Arc::new(lapiz_shader_graph::wgsl_std::types::F32Type));
    });
    graph.connect_slots_by_index(input, 0, output, 0);

    let mut instance = EffectInstance {
        name: "Brush Spacing".into(),
        passes: IndexMap::from([(
            EffectPassId::new(Uuid::new_v4()),
            EffectPass {
                name: "Spacing".into(),
                graph,
                dispatch_strategy: EffectPassDispatchStrategy::Once,
            },
        )]),
        inputs: IndexMap::from([(
            spacing_parameter,
            EffectInputSlot {
                name: "spacing".into(),
                id: spacing_parameter,
                ty: Arc::new(lapiz_shader_graph::wgsl_std::types::F32Type),
            },
        )]),
        outputs: IndexMap::from([(
            spacing_output,
            EffectOutputSlot {
                name: SPACING_OUTPUT.into(),
                id: spacing_output,
                ty: Arc::new(lapiz_shader_graph::wgsl_std::types::F32Type),
            },
        )]),
    };
    instance.sync_pass_graph_effect_properties()?;
    Ok((instance, spacing_parameter))
}

fn main_effect(assets: lapiz_assets::store::AssetRegistry) -> Result<(EffectInstance, EffectInputSlotId)> {
    use lapiz_brush::render::graph::{
        BlendWithInputNode, EllipticalMaskNode, ForegroundColorNode, PenPositionNode,
        PixelPositionNode,
    };
    use lapiz_shader_graph::wgsl_std::nodes::{CombineComponentsNode, ScalarMathNode};

    let resources = lapiz_brush::instance::main_effect_resources(assets.clone());
    let radius_parameter = EffectInputSlotId::new(Uuid::new_v4());
    let dab_color = EffectOutputSlotId::new(Uuid::new_v4());
    let dab_bounds = EffectOutputSlotId::new(Uuid::new_v4());
    let f32_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(lapiz_shader_graph::wgsl_std::types::F32Type);
    let color_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(lapiz_shader_graph::wgsl_std::types::ColorType);
    let rect_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(lapiz_shader_graph::wgsl_std::types::RectType);

    let mut graph = Graph::new(resources.clone());

    let radius = graph.add_node(Point::ORIGIN, PassInputNode);
    graph.update_node_state::<PassInputNode>(radius, |state| {
        state.input = Some(PassInput::Effect(radius_parameter));
        state.cached_ty = Some(f32_ty.clone());
    });

    // radii = vec2f(radius, radius)
    let half = graph.add_node(Point::ORIGIN, ScalarMathNode);
    graph.update_node_state::<ScalarMathNode>(half, |mode| {
        *mode = ScalarMathNodeMode::Multiply;
    });
    let radii = graph.add_node(Point::ORIGIN, CombineComponentsNode);

    let position = graph.add_node(Point::ORIGIN, PenPositionNode);
    let pixel = graph.add_node(Point::ORIGIN, PixelPositionNode);
    let mask = graph.add_node(Point::ORIGIN, EllipticalMaskNode);
    let color = graph.add_node(Point::ORIGIN, ForegroundColorNode);
    let blend = graph.add_node(Point::ORIGIN, BlendWithInputNode);

    let output_color = graph.add_node(Point::ORIGIN, PassOutputNode);
    graph.update_node_state::<PassOutputNode>(output_color, |state| {
        state.output = Some(PassOutput::Effect(dab_color));
        state.cached_ty = Some(color_ty.clone());
    });
    let output_bounds = graph.add_node(Point::ORIGIN, PassOutputNode);
    graph.update_node_state::<PassOutputNode>(output_bounds, |state| {
        state.output = Some(PassOutput::Effect(dab_bounds));
        state.cached_ty = Some(rect_ty.clone());
    });

    graph.connect_slots_by_index(radius, 0, half, 0);
    graph.connect_slots_by_index(half, 0, radii, 0);
    graph.connect_slots_by_index(radius, 0, radii, 1);
    graph.connect_slots_by_index(pixel, 0, mask, 0);
    graph.connect_slots_by_index(position, 0, mask, 1);
    graph.connect_slots_by_index(radii, 0, mask, 2);
    graph.connect_slots_by_index(color, 0, blend, 0);
    graph.connect_slots_by_index(mask, 0, blend, 1);
    graph.connect_slots_by_index(blend, 0, output_color, 0);
    graph.connect_slots_by_index(mask, 1, output_bounds, 0);

    let mut instance = EffectInstance {
        name: "Brush Main".into(),
        passes: IndexMap::from([(
            EffectPassId::new(Uuid::new_v4()),
            EffectPass {
                name: "Dab".into(),
                graph,
                dispatch_strategy: EffectPassDispatchStrategy::Once,
            },
        )]),
        inputs: IndexMap::from([(
            radius_parameter,
            EffectInputSlot {
                name: "radius".into(),
                id: radius_parameter,
                ty: f32_ty,
            },
        )]),
        outputs: IndexMap::from([
            (
                dab_color,
                EffectOutputSlot {
                    name: DAB_COLOR_OUTPUT.into(),
                    id: dab_color,
                    ty: color_ty,
                },
            ),
            (
                dab_bounds,
                EffectOutputSlot {
                    name: DAB_BOUNDS_OUTPUT.into(),
                    id: dab_bounds,
                    ty: rect_ty,
                },
            ),
        ]),
    };
    instance.sync_pass_graph_effect_properties()?;
    Ok((instance, radius_parameter))
}

fn postprocess_effect(assets: lapiz_assets::store::AssetRegistry) -> Result<(EffectInstance, ())> {
    use lapiz_brush::render::graph::{CurrentPixelColorNode, PixelPositionNode, StrokeBoundsNode};

    let resources = lapiz_brush::instance::postprocess_effect_resources(assets.clone());
    let stroke_color = EffectOutputSlotId::new(Uuid::new_v4());
    let stroke_bounds = EffectOutputSlotId::new(Uuid::new_v4());
    let color_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(lapiz_shader_graph::wgsl_std::types::ColorType);
    let rect_ty: Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType> =
        Arc::new(lapiz_shader_graph::wgsl_std::types::RectType);

    let mut graph = Graph::new(resources.clone());
    let pixel = graph.add_node(Point::ORIGIN, PixelPositionNode);
    let color = graph.add_node(Point::ORIGIN, CurrentPixelColorNode);
    let bounds = graph.add_node(Point::ORIGIN, StrokeBoundsNode);

    let output_color = graph.add_node(Point::ORIGIN, PassOutputNode);
    graph.update_node_state::<PassOutputNode>(output_color, |state| {
        state.output = Some(PassOutput::Effect(stroke_color));
        state.cached_ty = Some(color_ty.clone());
    });
    let output_bounds = graph.add_node(Point::ORIGIN, PassOutputNode);
    graph.update_node_state::<PassOutputNode>(output_bounds, |state| {
        state.output = Some(PassOutput::Effect(stroke_bounds));
        state.cached_ty = Some(rect_ty.clone());
    });

    graph.connect_slots_by_index(pixel, 0, color, 0);
    graph.connect_slots_by_index(color, 0, output_color, 0);
    graph.connect_slots_by_index(bounds, 0, output_bounds, 0);

    let mut instance = EffectInstance {
        name: "Brush Post Process".into(),
        passes: IndexMap::from([(
            EffectPassId::new(Uuid::new_v4()),
            EffectPass {
                name: "Passthrough".into(),
                graph,
                dispatch_strategy: EffectPassDispatchStrategy::Once,
            },
        )]),
        inputs: IndexMap::new(),
        outputs: IndexMap::from([
            (
                stroke_color,
                EffectOutputSlot {
                    name: STROKE_COLOR_OUTPUT.into(),
                    id: stroke_color,
                    ty: color_ty,
                },
            ),
            (
                stroke_bounds,
                EffectOutputSlot {
                    name: STROKE_BOUNDS_OUTPUT.into(),
                    id: stroke_bounds,
                    ty: rect_ty,
                },
            ),
        ]),
    };
    instance.sync_pass_graph_effect_properties()?;
    Ok((instance, ()))
}

fn generate_brush_preset(assets: lapiz_assets::store::AssetRegistry) -> Result<BrushPreset> {
    let (spacing, spacing_parameter) = spacing_effect(assets.clone())?;
    let (main, radius_parameter) = main_effect(assets.clone())?;
    let (postprocess, ()) = postprocess_effect(assets)?;

    let mut parameters = IndexMap::new();
    for (id, value) in [
        (spacing_parameter, SPACING_PARAMETER),
        (radius_parameter, RADIUS_PARAMETER),
    ] {
        parameters.insert(
            id,
            SerializableBrushParameter {
                name: "value".into(),
                value: SerializableGraphLiteral::serialize(&GraphLiteral::new::<
                    lapiz_shader_graph::wgsl_std::types::F32Type,
                >(value))?,
            },
        );
    }

    Ok(BrushPreset {
        metadata: BrushPresetMetadata {
            name: "E2E Round Brush".into(),
        },
        spacing_effect: spacing.as_asset()?,
        main_effect: main.as_asset()?,
        postprocess_effect: postprocess.as_asset()?,
        parameters,
    })
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
fn brush_round_trips_and_renders_preview() -> Result<()> {
    if !gpu_available() {
        eprintln!("skipping: no wgpu adapter available");
        return Ok(());
    }

    let assets = AssetRegistryBuilder::default().build();
    let preset = generate_brush_preset(assets.clone())?;

    // Serialize.
    let mut encoded = Vec::new();
    BrushPresetSerializer.write(&preset, &mut encoded)?;
    // Deserialize.
    let assets = AssetRegistryBuilder::default().build();
    let instance = BrushPresetInstance::new(&preset, assets.clone())
        .with_context(|| "deserialized brush preset must build an instance")?;

    // The persisted parameter values survive the round trip.
    let spacing_value = instance
        .parameters()
        .values()
        .filter_map(|parameter| parameter.value.try_as_ref::<f32>().copied())
        .find(|value| (*value - SPACING_PARAMETER).abs() < f32::EPSILON);
    bail_if_none(spacing_value.is_some(), "spacing parameter value missing")?;

    // Compile smoke check before touching the GPU pipeline.
    instance.compile().context("brush must compile")?;
    eprintln!("brush compiled");

    // Render a preview through the full brush pipeline.
    let context = RenderContext::default();
    let samples = predefined_curve_samples(256, 128);
    eprintln!("creating preview task");
    let task = create_stroke_preview_with(
        &instance,
        &samples,
        256,
        128,
        &context.device,
        &context.queue,
        &assets,
        &lapiz_brush::render::graph::CanvasResources {
            foreground_color: glam::Vec4::ONE,
            background_color: glam::Vec4::ZERO,
        },
    )?;
    eprintln!("preview task created");

    let texture = {
        // Headless runs have no frame loop to maintain the device, so keep a
        // poll thread alive while the preview tasks wait on buffer maps.
        let poll_device = context.device.clone();
        let poll_handle = std::thread::spawn(move || loop {
            let _ = poll_device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
        });
        use futures::StreamExt;

        let texture = block_on(async {
            let stream =
                iced_runtime::task::into_stream(task).expect("preview task is a stream");
            futures::pin_mut!(stream);
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(received) = action {
                    return Some(received);
                }
            }
            None
        });
        poll_handle.thread().unpark();
        texture.context("preview task produced no texture")?
    };
    eprintln!("preview texture received");

    // Read the composed preview texture back into host memory.
    let mut ec = context.device.create_command_encoder(&Default::default());
    let staging = lapiz_render::readback::create_readback_buffer_and_schedule_copy_texture(
        &context.device,
        &mut ec,
        &texture,
    );
    let readback = lapiz_render::readback::readback_buffer_raw_on_submit_async(
        &mut ec,
        &staging,
        ..,
    );
    let submission = context.queue.submit([ec.finish()]);
    context
        .device
        .poll_indefinitely_for(submission)
        .expect("preview readback submission must complete");
    let rgba_bytes = block_on(readback.into_inner())??;    let image = image::RgbaImage::from_raw(texture.width(), texture.height(), rgba_bytes)
        .context("preview image must match the texture size")?;

    let painted = image.pixels().filter(|p| p.0[3] > 0).count();
    eprintln!("painted pixels: {painted}");
    let path = std::env::temp_dir().join("lapiz_brush_e2e_preview.png");
    image.save(&path)?;
    eprintln!("preview saved to {}", path.display());
    assert!(painted > 0, "the stroke must paint at least one pixel");

    Ok(())
}

fn bail_if_none(ok: bool, message: &str) -> Result<()> {
    if !ok {
        bail!("{message}");
    }
    Ok(())
}

// Silence unused import when GPU tests are skipped early.
#[allow(unused)]
fn _touch() {
    let _ = EffectAssetSerializer;
}
