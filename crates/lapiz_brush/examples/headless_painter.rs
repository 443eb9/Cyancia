//! Headless brush rendering with unscaled, tightly cropped image output.
//!
//! Run from the workspace root, for example:
//! `ASSETS_DIR=assets cargo run --release -p lapiz_brush --example headless_painter --
//! --preset "My Brush" --output stroke.png --perf perf.json`
//! `--input-curve` accepts a JSON array of RawPenInput objects or a JSON file path.
//! Vector fields are [x, y]; time is {"now": 0.0, "stroke_begin": 0.0}.
//! Asset lookup uses the application's asset directory (overridden by ASSETS_DIR).

use std::{
    fs::{self, File},
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use anyhow::{Context as _, Result, bail, ensure};
use clap::Parser;
use futures::{StreamExt as _, executor::block_on};
use glam::{IVec2, Vec4};
use iced_runtime::{Action, Task, task::into_stream};
use image::{DynamicImage, RgbaImage, imageops};
use lapiz_assets::{
    asset::{AssetHandle, AssetId},
    bundle::{directory::BuiltinAssetsDirectorySource, standard::StandardAssetBundleSource},
    embedded,
    loader::AssetSerializerRegistry,
    source::AssetSourceRegistry,
    store::AssetRegistry,
};
use lapiz_brush::{
    asset::{BrushPreset, BrushPresetSerializer},
    input_processing::RawPenInput,
    instance::BrushPresetInstance,
    render::{
        graph::CanvasResources,
        stroke_preview::{
            CACHED_STROKE_PREVIEW_SIZE, create_stroke_preview_on, predefined_curve_samples,
        },
    },
};
use lapiz_dirs::assets_dir;
use lapiz_effect::asset::EffectAssetSerializer;
use lapiz_image::{
    texel::TexelType,
    tile::{DynamicLayerStorage, GpuLayerInfo, GpuTileStorage},
};
use lapiz_render::{
    readback::readback_buffer_raw_on_submit_async, texture::ImageSerializer,
    util::DevicePollExt as _,
};
use lapiz_runtime::renderer::RenderContext;
use lapiz_shader_graph::save::SerializableGraphFunctionSerializer;
use serde::Serialize;
use uuid::Uuid;
use wgpu::{
    BufferDescriptor, BufferUsages, COPY_BYTES_PER_ROW_ALIGNMENT, Device, Extent3d, Queue,
    TexelCopyBufferInfo, TexelCopyBufferLayout, Texture,
};

#[derive(Debug, Parser)]
#[command(about = "Render a brush without a window; output is tightly cropped and never resized")]
struct Args {
    /// Image to upload to the target layer before painting; otherwise transparent.
    #[arg(long, value_name = "PATH")]
    base_image: Option<PathBuf>,
    /// JSON array of RawPenInput objects or a JSON file path; otherwise a predefined curve.
    #[arg(long, value_name = "JSON_OR_PATH")]
    input_curve: Option<String>,
    /// Save the result; the image format is inferred from the extension.
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Report single cold-run wall-clock timings as JSON; no path means stdout.
    #[arg(long, num_args = 0..=1, value_name = "PATH")]
    perf: Option<Option<PathBuf>>,
    /// Exact brush metadata name or asset UUID. Duplicate names require a UUID.
    #[arg(long, value_name = "NAME_OR_ID")]
    preset: String,
}

#[derive(Serialize)]
struct Performance {
    measurement: &'static str,
    preset_id: AssetId<BrushPreset>,
    preset_name: String,
    adapter_name: String,
    raw_inputs: usize,
    output_width: u32,
    output_height: u32,
    asset_setup_ms: f64,
    input_load_ms: f64,
    gpu_setup_ms: f64,
    base_upload_ms: f64,
    render_ms: f64,
    image_readback_ms: f64,
    image_save_ms: f64,
    total_ms: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    block_on(paint(args))
}

async fn paint(args: Args) -> Result<()> {
    let total = Instant::now();
    let start = Instant::now();
    let assets = load_assets()?;
    let handle = find_preset(&assets, &args.preset)?;
    let brush = BrushPresetInstance::from_asset(&handle, assets)?;
    let asset_setup_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let start = Instant::now();
    let base = args
        .base_image
        .as_ref()
        .map(|path| {
            image::open(path)
                .with_context(|| format!("Unable to open base image {}", path.display()))
                .map(DynamicImage::into_rgba8)
        })
        .transpose()?;
    let (width, height) = base
        .as_ref()
        .map(RgbaImage::dimensions)
        .unwrap_or(CACHED_STROKE_PREVIEW_SIZE.into());
    let samples = load_curve(args.input_curve.as_deref(), width, height)?;
    let input_load_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let start = Instant::now();
    let context = RenderContext::request().await;
    let device = &context.device;
    let queue = &context.queue;
    let gpu_setup_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let start = Instant::now();
    let target = upload_base(device, queue, base.as_ref());
    wait_for_queue(device, queue)?;
    let base_upload_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let start = Instant::now();
    let task = create_stroke_preview_on(
        &brush,
        &samples,
        None,
        device,
        queue,
        &CanvasResources {
            foreground_color: Vec4::ONE,
            background_color: Vec4::ZERO,
        },
        target,
    )?;
    let texture = run_task(task).await??;
    wait_for_queue(device, queue)?;
    let render_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let start = Instant::now();
    let image = read_image(device, queue, &texture).await?;
    let image_readback_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let start = Instant::now();
    if let Some(path) = &args.output {
        image
            .save(path)
            .with_context(|| format!("Unable to save image {}", path.display()))?;
    }
    let image_save_ms = start.elapsed().as_secs_f64() * 1_000.0;

    if let Some(path) = args.perf {
        let perf = Performance {
            measurement: "single_cold_run_wall_clock; render includes compilation, GPU completion and bounds cropping; not GPU timestamp timing",
            preset_id: handle.id(),
            preset_name: brush.metadata().name.clone(),
            adapter_name: context.adapter.get_info().name,
            raw_inputs: samples.len(),
            output_width: image.width(),
            output_height: image.height(),
            asset_setup_ms,
            input_load_ms,
            gpu_setup_ms,
            base_upload_ms,
            render_ms,
            image_readback_ms,
            image_save_ms,
            total_ms: total.elapsed().as_secs_f64() * 1_000.0,
        };
        match path {
            Some(path) => {
                let file = File::create(&path).with_context(|| {
                    format!("Unable to create performance file {}", path.display())
                })?;
                serde_json::to_writer_pretty(file, &perf)?;
            }
            None => {
                let mut stdout = io::stdout().lock();
                serde_json::to_writer_pretty(&mut stdout, &perf)?;
                writeln!(stdout)?;
            }
        }
    }
    Ok(())
}

fn load_assets() -> Result<AssetRegistry> {
    let root = assets_dir();
    embedded::extract_if_empty(root)?;
    let mut serializers = AssetSerializerRegistry::default();
    serializers.register::<BrushPresetSerializer>();
    serializers.register::<EffectAssetSerializer>();
    serializers.register::<SerializableGraphFunctionSerializer>();
    serializers.register::<ImageSerializer>();
    let mut sources = AssetSourceRegistry::default();
    sources.register::<BuiltinAssetsDirectorySource>();
    sources.register::<StandardAssetBundleSource>();
    let assets = AssetRegistry::new(root, Arc::new(serializers))?;
    assets.add_erased_bundles(sources.scan_all(root))?;
    Ok(assets)
}

fn find_preset(assets: &AssetRegistry, name_or_id: &str) -> Result<AssetHandle<BrushPreset>> {
    if let Ok(id) = Uuid::parse_str(name_or_id) {
        return assets
            .handle(AssetId::new(id))
            .with_context(|| format!("Brush preset ID not found: {id}"));
    }
    let mut matches = Vec::new();
    for handle in assets.all_handles_of::<BrushPreset>()? {
        match handle.get() {
            Ok(preset) if preset.metadata.name == name_or_id => matches.push(handle),
            Ok(_) => {}
            Err(error) => eprintln!("Unable to read brush preset {}: {error:#}", handle.id()),
        }
    }
    ensure!(
        !matches.is_empty(),
        "Brush preset name not found: {name_or_id}"
    );
    ensure!(
        matches.len() == 1,
        "Ambiguous brush preset name: {name_or_id}; use an asset UUID"
    );
    Ok(matches.pop().unwrap())
}

fn load_curve(input: Option<&str>, width: u32, height: u32) -> Result<Vec<RawPenInput>> {
    let Some(input) = input else {
        return Ok(predefined_curve_samples(width, height).to_vec());
    };
    let json = if input.trim_start().starts_with('[') {
        input.to_owned()
    } else {
        fs::read_to_string(Path::new(input))
            .with_context(|| format!("Unable to read input curve {input}"))?
    };
    let samples = serde_json::from_str::<Vec<RawPenInput>>(&json)
        .context("Input curve must be a JSON array of RawPenInput objects")?;
    ensure!(
        samples.len() >= 2,
        "Input curve requires at least two samples"
    );
    for (index, sample) in samples.iter().enumerate() {
        ensure!(
            sample.position.is_finite()
                && sample.pressure.is_finite()
                && sample.tilt.is_finite()
                && sample.angle.is_finite()
                && sample.time.now.is_finite()
                && sample.time.stroke_begin.is_finite(),
            "Input curve sample {index} contains non-finite values"
        );
    }
    Ok(samples)
}

fn upload_base(device: &Device, queue: &Queue, image: Option<&RgbaImage>) -> DynamicLayerStorage {
    let mut target = DynamicLayerStorage::new(
        device.clone(),
        queue.clone(),
        GpuLayerInfo {
            texel_type: TexelType::RGBA8,
        },
    );
    if let Some(image) = image {
        let size = GpuTileStorage::TILE_SIZE;
        let mut tile = RgbaImage::new(size, size);
        for y in 0..image.height().div_ceil(size) {
            for x in 0..image.width().div_ceil(size) {
                tile.fill(0);
                let region = imageops::crop_imm(
                    image,
                    x * size,
                    y * size,
                    size.min(image.width() - x * size),
                    size.min(image.height() - y * size),
                );
                imageops::replace(&mut tile, &region.to_image(), 0, 0);
                let data =
                    TexelType::RGBA8.convert_image_to_wgpu(DynamicImage::ImageRgba8(tile.clone()));
                target.write_raw(queue, IVec2::new(x as i32, y as i32), &data);
            }
        }
    }
    target
}

fn wait_for_queue(device: &Device, queue: &Queue) -> Result<()> {
    device.poll_indefinitely_for(queue.submit([]))?;
    Ok(())
}

async fn run_task<T: Send + 'static>(task: Task<T>) -> Result<T> {
    let mut stream = into_stream(task).context("Brush task produced no stream")?;
    let mut output = None;
    while let Some(action) = stream.next().await {
        match action {
            Action::Output(value) => {
                ensure!(output.is_none(), "Brush task produced multiple outputs");
                output = Some(value);
            }
            _ => bail!("Brush task requires a GUI action"),
        }
    }
    output.context("Brush task produced no output")
}

async fn read_image(device: &Device, queue: &Queue, texture: &Texture) -> Result<RgbaImage> {
    let row_bytes = texture.width() * 4;
    let padded_row_bytes =
        row_bytes.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("headless painter image readback"),
        size: u64::from(padded_row_bytes) * u64::from(texture.height()),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row_bytes),
                rows_per_image: Some(texture.height()),
            },
        },
        Extent3d {
            width: texture.width(),
            height: texture.height(),
            depth_or_array_layers: 1,
        },
    );
    let readback = readback_buffer_raw_on_submit_async(&mut encoder, &buffer, ..);
    device.poll_indefinitely_for(queue.submit([encoder.finish()]))?;
    let bytes = readback.into_inner().await??;
    let pixels = bytes
        .chunks_exact(padded_row_bytes as usize)
        .flat_map(|row| row[..row_bytes as usize].iter().copied())
        .collect();
    RgbaImage::from_raw(texture.width(), texture.height(), pixels)
        .context("Unable to construct output image")
}

#[cfg(test)]
mod tests {
    use std::{env, fs, sync::Arc};

    use anyhow::Result;
    use clap::{Parser as _, error::ErrorKind};
    use futures::executor::block_on;
    use glam::{IVec2, Vec2};
    use iced_runtime::Task;
    use image::{Rgba, RgbaImage};
    use lapiz_assets::{
        bundle::{AssetBundle as _, directory::AssetDirectory},
        loader::AssetSerializerRegistry,
        store::AssetRegistry,
    };
    use lapiz_brush::{
        asset::{BrushPreset, BrushPresetMetadata, BrushPresetSerializer},
        render::stroke_preview::predefined_curve_samples,
    };
    use lapiz_effect::asset::EffectAsset;
    use lapiz_runtime::renderer::RenderContext;
    use uuid::Uuid;
    use wgpu::{
        Extent3d, TexelCopyBufferLayout, TextureDescriptor, TextureDimension, TextureFormat,
        TextureUsages,
    };

    use super::{Args, find_preset, load_curve, read_image, run_task, upload_base};

    #[test]
    fn preset_is_required_and_perf_path_is_optional() {
        assert_eq!(
            Args::try_parse_from(["painter"]).unwrap_err().kind(),
            ErrorKind::MissingRequiredArgument
        );
        let args = Args::try_parse_from(["painter", "--preset", "brush", "--perf"]).unwrap();
        assert_eq!(args.perf, Some(None));
        let args =
            Args::try_parse_from(["painter", "--perf", "perf.json", "--preset", "brush"]).unwrap();
        assert_eq!(args.perf, Some(Some("perf.json".into())));
        assert!(
            Args::try_parse_from(["painter", "--preset", "brush"])
                .unwrap()
                .perf
                .is_none()
        );
    }

    #[test]
    fn predefined_and_inline_curves_use_raw_input_fields() {
        let default = load_curve(None, 100, 50).unwrap();
        assert_eq!(default.len(), 32);
        assert_eq!(default.last().unwrap().position.x, 100.0);
        let json = serde_json::to_string(&predefined_curve_samples(100, 50)).unwrap();
        let samples = load_curve(Some(&json), 100, 50).unwrap();
        assert_eq!(samples.len(), default.len());
        assert_eq!(samples[10].position, default[10].position);
        assert_eq!(samples[10].time.now, default[10].time.now);
        assert_eq!(samples[10].tilt, default[10].tilt);
        assert_eq!(samples[10].angle, default[10].angle);
    }

    #[test]
    fn invalid_curves_fail_before_rendering() {
        assert!(load_curve(Some("[]"), 100, 50).is_err());
        assert!(load_curve(Some("[{}]"), 100, 50).is_err());
        let mut samples = predefined_curve_samples(100, 50);
        samples[0].position = Vec2::new(f32::INFINITY, 0.0);
        let json = serde_json::to_string(&samples).unwrap();
        assert!(load_curve(Some(&json), 100, 50).is_err());
    }

    #[test]
    fn presets_resolve_by_name_or_id_and_reject_duplicate_names() -> Result<()> {
        let root = env::temp_dir().join(format!("lapiz-painter-assets-{}", Uuid::new_v4()));
        fs::create_dir(&root)?;
        let result = (|| -> Result<()> {
            let directory = AssetDirectory::new(&root)?;
            let bundle_id = directory.metadata()?.bundle_id;
            let mut serializers = AssetSerializerRegistry::default();
            serializers.register::<BrushPresetSerializer>();
            let assets = AssetRegistry::new_in_memory(Arc::new(serializers));
            assets.add_bundle(directory)?;
            let empty_effect = EffectAsset {
                name: String::new(),
                passes: Vec::new(),
                inputs: Vec::new(),
                outputs: Vec::new(),
            };
            let preset = Arc::new(BrushPreset {
                metadata: BrushPresetMetadata {
                    name: "Test Brush".into(),
                },
                spacing_effect: empty_effect.clone(),
                main_effect: empty_effect.clone(),
                postprocess_effect: empty_effect,
                parameters: Default::default(),
            });
            let id = assets.add_asset(bundle_id, "first.lapiz", preset.clone())?;
            assert_eq!(find_preset(&assets, "Test Brush")?.id(), id);
            assert_eq!(find_preset(&assets, &id.to_string())?.id(), id);
            assert!(find_preset(&assets, "Missing Brush").is_err());
            assert!(find_preset(&assets, &Uuid::new_v4().to_string()).is_err());
            assets.add_asset(bundle_id, "second.lapiz", preset)?;
            assert!(find_preset(&assets, "Test Brush").is_err());
            assert_eq!(find_preset(&assets, &id.to_string())?.id(), id);
            Ok(())
        })();
        fs::remove_dir_all(root)?;
        result
    }

    #[test]
    fn curve_can_be_loaded_from_a_file() -> Result<()> {
        let path = env::temp_dir().join(format!("lapiz-curve-{}.json", Uuid::new_v4()));
        fs::write(
            &path,
            serde_json::to_vec(&predefined_curve_samples(100, 50))?,
        )?;
        let result = load_curve(Some(path.to_str().unwrap()), 100, 50);
        fs::remove_file(path)?;
        assert_eq!(result?.len(), 32);
        Ok(())
    }

    #[test]
    fn gpu_base_upload_and_unaligned_readback_preserve_pixels() -> Result<()> {
        block_on(async {
            let context = RenderContext::request().await;
            let device = &context.device;
            let queue = &context.queue;
            let image = RgbaImage::from_pixel(257, 3, Rgba([23, 45, 67, 89]));
            let layer = upload_base(device, queue, Some(&image));
            let tiles = layer
                .readback(device, queue, layer.iter_tile_indices())
                .await?;
            let first = &tiles[&IVec2::ZERO];
            assert_eq!(&first[..4], &[23, 45, 67, 89]);
            let second = &tiles[&IVec2::X];
            assert_eq!(&second[..4], &[23, 45, 67, 89]);
            assert_eq!(&second[4..8], &[0, 0, 0, 0]);
            assert_eq!(&second[3 * 256 * 4..3 * 256 * 4 + 4], &[0, 0, 0, 0]);

            let image = RgbaImage::from_pixel(3, 2, Rgba([12, 34, 56, 78]));
            let texture = device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: 3,
                    height: 2,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                texture.as_image_copy(),
                image.as_raw(),
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(12),
                    rows_per_image: Some(2),
                },
                texture.size(),
            );
            assert_eq!(read_image(device, queue, &texture).await?, image);
            Ok(())
        })
    }

    #[test]
    fn headless_task_executes_all_work() {
        let task = Task::batch([Task::future(async { None }), Task::done(Some(42))])
            .collect()
            .map(|values| values.into_iter().flatten().next().unwrap());
        assert_eq!(block_on(run_task(task)).unwrap(), 42);
        assert!(block_on(run_task(Task::<()>::none())).is_err());
    }
}
