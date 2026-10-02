use std::{
    array::from_fn,
    f32::consts::TAU,
    fs::{self, File},
};

use anyhow::{Result, anyhow, ensure};
use glam::{IVec4, Vec2, Vec4};
use iced_runtime::Task;
use image::{ImageFormat, RgbaImage};
use lapiz_assets::{asset::AssetHandle, store::AssetRegistry};
use lapiz_dirs::cache_dir;
use lapiz_image::{
    layer_bounds::LayerBoundsPipeline,
    texel::TexelType,
    tile::{DynamicLayerStorage, GpuLayerInfo, GpuTileInfo, LayerBinding},
};
use lapiz_render::{
    bind_group_entries::BindGroupEntries,
    bind_group_layout_entries::{BindGroupLayoutEntries, binding_types},
    buffer::DynamicBuffer,
    readback::{
        create_readback_buffer_and_schedule_copy_buffer,
        create_readback_buffer_and_schedule_copy_texture, readback_buffer_on_submit_async,
        readback_buffer_raw_on_submit_async,
    },
    render_context::RenderContextAppExt as _,
    util::DevicePollExt as _,
};
use lapiz_runtime::global::Globals;
use lapiz_utils::log_err::LogErr as _;
use tracing::info;
use wesl::include_wesl;
use wgpu::{
    BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, Buffer, BufferUsages,
    ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor, Device, Extent3d,
    PipelineLayoutDescriptor, Queue, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureViewDescriptor,
};

use crate::{
    asset::BrushPreset,
    input_processing::{BasicStabilizer, InputProcessor, RawPenInput},
    instance::BrushPresetInstance,
    render::{BrushPresetRenderer, Time, graph::CanvasResources},
};

pub const CACHED_STROKE_PREVIEW_SIZE: (u32, u32) = (512, 256);

pub fn load_cached_stroke_preview_or_generate(
    brush: &AssetHandle<BrushPreset>,
    assets: &AssetRegistry,
    globals: &Globals,
) -> Result<Task<Result<RgbaImage>>> {
    let cache_path = cache_dir()
        .join("brush_stroke_preview")
        .join(format!("preview-{}.png", brush.id()));

    if cache_path.exists()
        && let Ok(img) = image::open(&cache_path).logged_err()
    {
        info!("Loaded cached stroke preview from {}", cache_path.display());
        return Ok(Task::done(Ok(img.into_rgba8())));
    }

    info!("Generating stroke preview for brush {}", brush.id());

    let instance = BrushPresetInstance::from_asset(brush, assets.clone())
        .map_err(|error| anyhow::anyhow!("Failed to create brush preset instance: {error:#}"))?;

    let texture = create_stroke_preview(
        &instance,
        &predefined_curve_samples(CACHED_STROKE_PREVIEW_SIZE.0, CACHED_STROKE_PREVIEW_SIZE.1),
        CACHED_STROKE_PREVIEW_SIZE.0,
        CACHED_STROKE_PREVIEW_SIZE.1,
        globals,
        &CanvasResources {
            foreground_color: Vec4::ONE,
            background_color: Vec4::ZERO,
        },
    )?;

    let device = globals.render_device().clone();
    let queue = globals.render_queue().clone();

    Ok(texture
        .then(move |texture| readback_preview(device.clone(), queue.clone(), texture))
        .map(move |img| {
            let img = img.logged_err().unwrap_or_else(|_| {
                RgbaImage::new(CACHED_STROKE_PREVIEW_SIZE.0, CACHED_STROKE_PREVIEW_SIZE.1)
            });
            fs::create_dir_all(
                cache_path
                    .parent()
                    .expect("preview cache path has a parent"),
            )?;
            let mut file = File::create(&cache_path)?;
            img.write_to(&mut file, ImageFormat::Png)?;
            info!("Stroke preview saved to {}", cache_path.display());
            Ok(img)
        }))
}

fn readback_preview(device: Device, queue: Queue, texture: Texture) -> Task<Result<RgbaImage>> {
    let mut ec = device.create_command_encoder(&Default::default());
    let staging = create_readback_buffer_and_schedule_copy_texture(&device, &mut ec, &texture);
    let mut readback = Some(readback_buffer_raw_on_submit_async(&mut ec, &staging, ..));
    let si = queue.submit([ec.finish()]);

    let width = texture.width();
    let height = texture.height();

    Task::future(async move {
        let _ = device.poll_indefinitely_for(si);
    })
    .then(move |_| {
        let readback = readback
            .take()
            .expect("stroke preview readback task must only run once");
        Task::future(async move {
            let rgba_bytes = readback.into_inner().await??;
            RgbaImage::from_raw(width, height, rgba_bytes)
                .ok_or_else(|| anyhow!("Unable to create preview image."))
        })
    })
}

pub fn predefined_curve_samples(width: u32, height: u32) -> [RawPenInput; 32] {
    from_fn(|i| {
        let t = i as f32 / 31.0;
        let azimuth = t * TAU;
        let altitude = (30.0 + 30.0 * t).to_radians();
        let tan_altitude = altitude.tan();

        RawPenInput {
            position: Vec2::new(
                width as f32 * t,
                height as f32 * (0.5 + 0.25 * (t * TAU).sin()),
            ),
            pressure: t,
            tilt: Vec2::new(
                (azimuth.cos() / tan_altitude).atan(),
                (azimuth.sin() / tan_altitude).atan(),
            ),
            angle: Vec2::new(altitude, azimuth),
            time: Time {
                now: t,
                stroke_begin: 0.0,
            },
        }
    })
}

// TODO remove this method
pub fn create_stroke_preview(
    brush: &BrushPresetInstance,
    samples: &[RawPenInput],
    width: u32,
    height: u32,
    globals: &Globals,
    canvas_resources: &CanvasResources,
) -> Result<Task<Texture>> {
    create_stroke_preview_with(
        brush,
        samples,
        width,
        height,
        globals.render_device(),
        globals.render_queue(),
        canvas_resources,
    )
}

pub fn create_stroke_preview_with(
    brush: &BrushPresetInstance,
    samples: &[RawPenInput],
    width: u32,
    height: u32,
    device: &Device,
    queue: &Queue,
    canvas_resources: &CanvasResources,
) -> Result<Task<Texture>> {
    let target_layer = DynamicLayerStorage::new(
        device.clone(),
        queue.clone(),
        GpuLayerInfo {
            texel_type: TexelType::RGBA8,
        },
    );
    create_stroke_preview_on_target_with(
        brush,
        samples,
        width,
        height,
        device,
        queue,
        canvas_resources,
        target_layer,
    )
}

pub fn create_stroke_preview_on_target(
    brush: &BrushPresetInstance,
    samples: &[RawPenInput],
    width: u32,
    height: u32,
    globals: &Globals,
    canvas_resources: &CanvasResources,
    target_layer: DynamicLayerStorage,
) -> Result<Task<Texture>> {
    create_stroke_preview_on_target_with(
        brush,
        samples,
        width,
        height,
        globals.render_device(),
        globals.render_queue(),
        canvas_resources,
        target_layer,
    )
}

pub fn create_stroke_preview_on_target_with(
    brush: &BrushPresetInstance,
    samples: &[RawPenInput],
    width: u32,
    height: u32,
    device: &Device,
    queue: &Queue,
    canvas_resources: &CanvasResources,
    target_layer: DynamicLayerStorage,
) -> Result<Task<Texture>> {
    ensure!(
        width > 0 && height > 0,
        "stroke preview dimensions must be non-zero"
    );
    let stroke = render_stroke_on_target_with(
        brush,
        samples,
        device,
        queue,
        canvas_resources,
        target_layer,
    )?;
    let device = device.clone();
    let queue = queue.clone();
    Ok(stroke.map(move |result| {
        map_result_texture(device.clone(), queue.clone(), width, height, result)
    }))
}

/// Renders at the original resolution and crops to the non-transparent pixel bounds.
/// An empty result is represented by a transparent 1×1 texture.
pub fn create_stroke_image_on_target_with(
    brush: &BrushPresetInstance,
    samples: &[RawPenInput],
    device: &Device,
    queue: &Queue,
    canvas_resources: &CanvasResources,
    target_layer: DynamicLayerStorage,
) -> Result<Task<Result<Texture>>> {
    let stroke = render_stroke_on_target_with(
        brush,
        samples,
        device,
        queue,
        canvas_resources,
        target_layer,
    )?;
    let device = device.clone();
    let queue = queue.clone();
    Ok(stroke.then(move |result| {
        let device = device.clone();
        let queue = queue.clone();
        Task::future(async move { map_tight_result_texture(&device, &queue, result).await })
    }))
}

fn render_stroke_on_target_with(
    brush: &BrushPresetInstance,
    samples: &[RawPenInput],
    device: &Device,
    queue: &Queue,
    canvas_resources: &CanvasResources,
    mut target_layer: DynamicLayerStorage,
) -> Result<Task<DynamicLayerStorage>> {
    ensure!(
        samples.len() >= 2,
        "stroke preview requires at least two samples"
    );

    let mut selection_layer = DynamicLayerStorage::new(
        device.clone(),
        queue.clone(),
        GpuLayerInfo {
            texel_type: TexelType::A8,
        },
    );

    let foreground_color = {
        let mut buffer = DynamicBuffer::new(
            Some("brush preview foreground color".into()),
            BufferUsages::STORAGE,
        );
        buffer.push(&canvas_resources.foreground_color);
        buffer.write_buffer(device, queue);
        buffer
    };
    let background_color = {
        let mut buffer = DynamicBuffer::new(
            Some("brush preview background color".into()),
            BufferUsages::STORAGE,
        );
        buffer.push(&canvas_resources.background_color);
        buffer.write_buffer(device, queue);
        buffer
    };

    let compiled = brush.compile(
        target_layer.layer_info().texel_type,
        selection_layer.layer_info().texel_type,
        device,
        queue,
    )?;
    let mut renderer = BrushPresetRenderer::new(
        compiled,
        target_layer.layer_info().texel_type,
        selection_layer.layer_info().texel_type,
        device,
        queue,
        &foreground_color,
        &background_color,
    );

    let mut input_processor = InputProcessor::new(256, Box::new(BasicStabilizer));

    // Ensure both layers have at least a null tile so binding() works without
    // the app-global empty-layer bindings.
    target_layer.get_tile_or_allocate(GpuTileInfo::NULL.index);
    selection_layer.get_tile_or_allocate(GpuTileInfo::NULL.index);

    let worker = renderer.begin(
        device,
        queue,
        target_layer.binding_or_empty(),
        selection_layer.binding_or_empty(),
    );
    renderer.record_raw_inputs(samples.len());

    for sample in samples.iter().take(samples.len() - 1) {
        if let Some(pen_input) = input_processor.push(*sample) {
            renderer.update(pen_input);
        }
    }

    for pen_input in input_processor.flush(*samples.last().unwrap()) {
        renderer.update(pen_input);
    }

    let mut target_layer = Some(target_layer);
    let final_result = renderer.end().map(move |result| {
        let mut target_layer = target_layer.take().expect("stroke task must only run once");
        // The brush result replaces modified tiles, not the entire target layer.
        target_layer.copy_tiles_from(&result, result.iter_tile_indices());
        Some(target_layer)
    });
    Ok(Task::batch([worker.map(|()| None), final_result])
        .collect()
        .map(|layers| {
            layers
                .into_iter()
                .flatten()
                .next()
                .expect("stroke worker produced no layer")
        }))
}

async fn map_tight_result_texture(
    device: &Device,
    queue: &Queue,
    result: DynamicLayerStorage,
) -> Result<Texture> {
    let Some(binding) = result.binding() else {
        return Ok(create_output_texture(device, 1, 1));
    };
    let mut encoder = device.create_command_encoder(&Default::default());
    let bounds_buffer = LayerBoundsPipeline::new(device, TexelType::RGBA8, false).dispatch(
        device,
        queue,
        &mut encoder,
        &binding,
        None,
    );
    let staging =
        create_readback_buffer_and_schedule_copy_buffer(device, &mut encoder, &bounds_buffer);
    let readback = readback_buffer_on_submit_async::<IVec4, _>(&mut encoder, &staging, ..);
    let submission = queue.submit([encoder.finish()]);
    device.poll_indefinitely_for(submission)?;
    let bounds = readback.into_inner().await??;
    if bounds.z <= bounds.x || bounds.w <= bounds.y {
        return Ok(create_output_texture(device, 1, 1));
    }
    let width = (i64::from(bounds.z) - i64::from(bounds.x)) as u64;
    let height = (i64::from(bounds.w) - i64::from(bounds.y)) as u64;
    let limit = u64::from(device.limits().max_texture_dimension_2d);
    ensure!(
        width <= limit && height <= limit,
        "stroke bounds exceed the texture size limit: {width}×{height}"
    );
    let texture = create_output_texture(device, width as u32, height as u32);
    ComposeStrokePreviewPipeline::new(device, "unscaled").dispatch(
        device,
        queue,
        &binding,
        &bounds_buffer,
        &texture,
    );
    Ok(texture)
}

fn map_result_texture(
    device: Device,
    queue: Queue,
    width: u32,
    height: u32,
    result: DynamicLayerStorage,
) -> Texture {
    let output_texture = create_output_texture(&device, width, height);

    let Some(result_binding) = result.binding() else {
        return output_texture;
    };
    let mut ec = device.create_command_encoder(&Default::default());
    let result_bounds = LayerBoundsPipeline::new(&device, TexelType::RGBA8, false).dispatch(
        &device,
        &queue,
        &mut ec,
        &result_binding,
        None,
    );
    queue.submit([ec.finish()]);

    ComposeStrokePreviewPipeline::new(&device, "main").dispatch(
        &device,
        &queue,
        &result_binding,
        &result_bounds,
        &output_texture,
    );

    output_texture
}

fn create_output_texture(device: &Device, width: u32, height: u32) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("stroke preview texture"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::STORAGE_BINDING
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

struct ComposeStrokePreviewPipeline {
    layout: BindGroupLayout,
    pipeline: ComputePipeline,
}

impl ComposeStrokePreviewPipeline {
    pub fn new(device: &Device, entry_point: &str) -> Self {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("compose stroke preview bind group layout"),
            entries: BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    binding_types::texture_storage_2d_array(
                        TexelType::RGBA8.wgpu_format(),
                        StorageTextureAccess::ReadOnly,
                    ),
                    binding_types::storage_buffer_read_only::<GpuTileInfo>(false),
                    binding_types::storage_buffer_read_only::<IVec4>(false),
                    binding_types::texture_storage_2d(
                        TextureFormat::Rgba8Unorm,
                        StorageTextureAccess::WriteOnly,
                    ),
                ),
            )
            .as_ref(),
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("compose stroke preview pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("compose stroke preview shader"),
            source: ShaderSource::Wgsl(include_wesl!("compose_stroke_preview").into()),
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("compose stroke preview pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache: None,
        });

        Self { layout, pipeline }
    }

    fn dispatch(
        &self,
        device: &Device,
        queue: &Queue,
        result: &LayerBinding,
        result_bounds: &Buffer,
        output: &Texture,
    ) {
        let output_view = output.create_view(&TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("compose stroke preview bind group"),
            layout: &self.layout,
            entries: BindGroupEntries::sequential((
                &result.texture,
                result.tile_info_buffer.as_entire_binding(),
                result_bounds.as_entire_binding(),
                &output_view,
            ))
            .as_ref(),
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("compose stroke preview pass"),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(output.width().div_ceil(16), output.height().div_ceil(16), 1);
        }
        queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use futures::executor::block_on;
    use glam::IVec2;
    use lapiz_render::util::DevicePollExt as _;
    use lapiz_runtime::renderer::RenderContext;

    use super::{
        DynamicLayerStorage, GpuLayerInfo, TexelType,
        create_readback_buffer_and_schedule_copy_texture, map_tight_result_texture,
        readback_buffer_raw_on_submit_async,
    };

    #[test]
    fn tight_output_preserves_negative_coordinates_and_pixel_values() -> Result<()> {
        block_on(async {
            let context = RenderContext::request().await;
            let device = &context.device;
            let queue = &context.queue;
            let new_layer = || {
                DynamicLayerStorage::new(
                    device.clone(),
                    queue.clone(),
                    GpuLayerInfo {
                        texel_type: TexelType::RGBA8,
                    },
                )
            };
            let empty = map_tight_result_texture(device, queue, new_layer()).await?;
            assert_eq!((empty.width(), empty.height()), (1, 1));

            let mut layer = new_layer();
            let mut bytes = vec![0; 256 * 256 * 4];
            let offset = (254 * 256 + 253) * 4;
            bytes[offset..offset + 4].copy_from_slice(&[9, 10, 11, 127]);
            layer.write_raw(queue, IVec2::new(-1, -1), &bytes);
            bytes.fill(0);
            let offset = (254 * 256 + 60) * 4;
            bytes[offset..offset + 4].copy_from_slice(&[12, 13, 14, 255]);
            layer.write_raw(queue, IVec2::new(0, -1), &bytes);
            let texture = map_tight_result_texture(device, queue, layer).await?;
            assert_eq!((texture.width(), texture.height()), (64, 1));
            let mut encoder = device.create_command_encoder(&Default::default());
            let staging =
                create_readback_buffer_and_schedule_copy_texture(device, &mut encoder, &texture);
            let readback = readback_buffer_raw_on_submit_async(&mut encoder, &staging, ..);
            device.poll_indefinitely_for(queue.submit([encoder.finish()]))?;
            let bytes = readback.into_inner().await??;
            assert_eq!(&bytes[..4], &[9, 10, 11, 127]);
            assert!(bytes[4..63 * 4].iter().all(|byte| *byte == 0));
            assert_eq!(&bytes[63 * 4..], &[12, 13, 14, 255]);
            Ok(())
        })
    }
}
