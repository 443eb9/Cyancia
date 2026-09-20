use std::collections::HashMap;

use anyhow::{Context as _, Result, bail, ensure};
use lapiz_abr::descriptor::{
    UnitFloat,
    class::{self, BrushTip, Descriptor, DynamicsControl, PropertyDynamics, ToolOptions},
};
use lapiz_assets::asset::AssetId;
use lapiz_brush::asset::{BrushPreset, BrushPresetMetadata};
use lapiz_image::blend_modes::BlendMode;
use lapiz_render::texture::Image;
use uuid::Uuid;

use crate::desc::{
    graph::{ComputedMainTip, MainGraphOptions, SampledMainTip},
    wgsl::{BrushPose, BrushTexture, ColorAdjustment, DualBrush, DualBrushTip, Dynamics, Scatter},
};

pub mod graph;
pub mod wgsl;

fn parse_blend_mode(mode: class::BlendMode) -> BlendMode {
    match mode {
        class::BlendMode::Normal => BlendMode::Normal,
        class::BlendMode::Dissolve => BlendMode::Dissolve,
        class::BlendMode::Behind => BlendMode::Behind,
        class::BlendMode::Clear => BlendMode::Clear,
        class::BlendMode::Darken => BlendMode::Darken,
        class::BlendMode::Multiply => BlendMode::Multiply,
        class::BlendMode::ColorBurn => BlendMode::ColorBurn,
        class::BlendMode::LinearBurn => BlendMode::LinearBurn,
        class::BlendMode::DarkerColor => BlendMode::DarkerColor,
        class::BlendMode::Lighten => BlendMode::Lighten,
        class::BlendMode::Screen => BlendMode::Screen,
        class::BlendMode::ColorDodge => BlendMode::ColorDodge,
        class::BlendMode::LinearDodge => BlendMode::LinearDodge,
        class::BlendMode::LighterColor => BlendMode::LighterColor,
        class::BlendMode::Overlay => BlendMode::Overlay,
        class::BlendMode::SoftLight => BlendMode::SoftLight,
        class::BlendMode::HardLight => BlendMode::HardLight,
        class::BlendMode::VividLight => BlendMode::VividLight,
        class::BlendMode::LinearLight => BlendMode::LinearLight,
        class::BlendMode::PinLight => BlendMode::PinLight,
        class::BlendMode::HardMix => BlendMode::HardMix,
        class::BlendMode::Difference => BlendMode::Difference,
        class::BlendMode::Exclusion => BlendMode::Exclusion,
        class::BlendMode::Subtract => BlendMode::Subtract,
        class::BlendMode::SubtractTexture => BlendMode::Subtractive,
        class::BlendMode::Divide => BlendMode::Divide,
        class::BlendMode::Hue => BlendMode::Hue,
        class::BlendMode::Saturation => BlendMode::Saturation,
        class::BlendMode::Color => BlendMode::Color,
        class::BlendMode::Luminosity => BlendMode::Luminosity,
        class::BlendMode::Height => BlendMode::Height,
        class::BlendMode::LinearHeight => BlendMode::LinearHeight,
    }
}

fn parse_dual_brush(
    brush: &Descriptor,
    sample_assets: &HashMap<Uuid, AssetId<Image>>,
) -> Result<(Option<DualBrush>, Option<AssetId<Image>>)> {
    let dual = &brush.dual_brush;
    if !dual.enabled {
        return Ok((None, None));
    }
    let tip = dual.brush.as_ref().context("missing dual brush tip")?;
    let spacing = match tip {
        BrushTip::Computed(tip) => tip.spacing.value as f32 / 100.0,
        BrushTip::Sampled(tip) => tip.spacing.value as f32 / 100.0,
        BrushTip::DBrush(tip) => tip.spacing.value as f32 / 100.0,
    }
    .max(0.001);
    let scatter = if dual.use_scatter {
        let scatter_dynamics = dual
            .scatter_dynamics
            .as_ref()
            .map(|dynamics| parse_dynamics(dynamics, false, 0.0).context("dual scatter dynamics"))
            .transpose()?;
        let count_dynamics = dual
            .count_dynamics
            .as_ref()
            .map(|dynamics| parse_dynamics(dynamics, false, 0.0).context("dual count dynamics"))
            .transpose()?;
        let count = dual.scatter_count.round();
        ensure!(
            dual.scatter_count.is_finite()
                && (dual.scatter_count - count).abs() < f64::EPSILON
                && (1.0..=16.0).contains(&count),
            "unsupported dual scatter count {}",
            dual.scatter_count
        );
        Some(Scatter {
            both_axes: dual.scatter_both_axes,
            scatter_dynamics,
            count: count as u32,
            count_dynamics,
        })
    } else {
        None
    };
    let (tip, sample_asset) = match tip {
        BrushTip::Computed(tip) => {
            ensure!(tip.interpolation, "unsupported dual computed interpolation");
            (
                DualBrushTip::Computed {
                    diameter: tip.diameter.value as f32,
                    hardness: (tip.hardness.value as f32 / 100.0).clamp(0.0, 1.0),
                    angle: tip.angle.value.to_radians() as f32,
                    roundness: (tip.roundness.value as f32 / 100.0).clamp(0.001, 1.0),
                    flip_x: tip.flip_x,
                    flip_y: tip.flip_y,
                },
                None,
            )
        }
        BrushTip::Sampled(tip) => {
            ensure!(tip.interpolation, "unsupported dual sampled interpolation");
            let sample_asset = sample_assets
                .get(&tip.id)
                .copied()
                .with_context(|| format!("dual brush sample not found {}", tip.id))?;
            (
                DualBrushTip::Sampled {
                    diameter: tip.diameter.value as f32,
                    angle: tip.angle.value.to_radians() as f32,
                    roundness: (tip.roundness.value as f32 / 100.0).clamp(0.001, 1.0),
                    flip_x: tip.flip_x,
                    flip_y: tip.flip_y,
                },
                Some(sample_asset),
            )
        }
        BrushTip::DBrush(_) => bail!("unsupported dual dbrush tip"),
    };
    Ok((
        Some(DualBrush {
            tip,
            flip: dual.flip,
            blend_mode: parse_blend_mode(dual.blend_mode.unwrap_or(class::BlendMode::Multiply)),
            spacing,
            scatter,
        }),
        sample_asset,
    ))
}

fn parse_dynamics(
    dynamics: &PropertyDynamics,
    pressure_override: bool,
    minimum: f32,
) -> Result<Dynamics> {
    let control = if pressure_override {
        DynamicsControl::PenPressure
    } else {
        dynamics.control
    };
    ensure!(
        control != DynamicsControl::StylusWheel,
        "unsupported stylus wheel"
    );
    let dynamics_minimum = dynamics
        .minimum
        .as_ref()
        .and_then(UnitFloat::as_percentage_01)
        .unwrap_or(0.0);
    Ok(Dynamics {
        control,
        fade_steps: dynamics.fade_steps,
        jitter: (dynamics.jitter.value as f32 / 100.0).max(0.0),
        minimum: minimum.max(dynamics_minimum).clamp(0.0, 1.0),
    })
}

pub fn parse_desc(
    brush: &Descriptor,
    sample_assets: &HashMap<Uuid, AssetId<Image>>,
    pattern_assets: &HashMap<Uuid, AssetId<Image>>,
) -> Result<BrushPreset> {
    ensure!(!brush.wtdg, "unsupported wet edges");
    ensure!(!brush.repeat, "unsupported repeat");
    match &brush.tool_options {
        Some(ToolOptions::Smudge(_)) => bail!("unsupported smudge tool"),
        Some(ToolOptions::Sh(_)) => bail!("unsupported sh tool"),
        _ => {}
    }
    let paint_options = match &brush.tool_options {
        Some(ToolOptions::Paint(options)) => Some(options),
        _ => None,
    };
    let (opacity, flow, pressure_overrides_size, pressure_overrides_opacity, paint_blend_mode) =
        match &brush.tool_options {
            Some(ToolOptions::Paint(options)) => (
                (options.opacity as f32 / 100.0).clamp(0.0, 1.0),
                (options.flow as f32 / 100.0).clamp(0.0, 1.0),
                options.use_pressure_overrides_size,
                options.use_pressure_overrides_opacity,
                parse_blend_mode(options.blend_mode),
            ),
            Some(ToolOptions::Eraser(options)) => {
                ensure!(!options.magic_eraser, "unsupported magic eraser");
                (
                    (options.opacity as f32 / 100.0).clamp(0.0, 1.0),
                    (options.flow as f32 / 100.0).clamp(0.0, 1.0),
                    options.use_pressure_overrides_size,
                    options.use_pressure_overrides_opacity,
                    BlendMode::Clear,
                )
            }
            _ => (1.0, 1.0, false, false, BlendMode::Normal),
        };
    let minimum_diameter = brush
        .minimum_diameter
        .as_ref()
        .and_then(UnitFloat::as_percentage_01)
        .unwrap_or(0.0);
    let size_dynamics = brush
        .size_dynamics
        .as_ref()
        .or_else(|| paint_options.and_then(|options| options.size_dynamics.as_ref()))
        .map(|dynamics| {
            parse_dynamics(dynamics, pressure_overrides_size, minimum_diameter)
                .context("size dynamics")
        })
        .transpose()?;
    let size_dynamics = if pressure_overrides_size && size_dynamics.is_none() {
        Some(Dynamics {
            control: DynamicsControl::PenPressure,
            fade_steps: 0,
            jitter: 0.0,
            minimum: minimum_diameter.clamp(0.0, 1.0),
        })
    } else {
        size_dynamics
    };
    let opacity_dynamics = brush
        .opacity_dynamics
        .as_ref()
        .or_else(|| paint_options.and_then(|options| options.opacity_dynamics.as_ref()))
        .map(|dynamics| {
            parse_dynamics(dynamics, pressure_overrides_opacity, 0.0).context("opacity dynamics")
        })
        .transpose()?;
    let opacity_dynamics = if pressure_overrides_opacity && opacity_dynamics.is_none() {
        Some(Dynamics {
            control: DynamicsControl::PenPressure,
            fade_steps: 0,
            jitter: 0.0,
            minimum: 0.0,
        })
    } else {
        opacity_dynamics
    };
    let flow_dynamics = brush
        .flow_dynamics
        .as_ref()
        .or_else(|| paint_options.and_then(|options| options.flow_dynamics.as_ref()))
        .map(|dynamics| parse_dynamics(dynamics, false, 0.0).context("flow dynamics"))
        .transpose()?;
    let angle_dynamics = brush
        .angle_dynamics
        .as_ref()
        .map(|dynamics| {
            ensure!(
                dynamics.control != DynamicsControl::Fade,
                "unsupported angle fade"
            );
            parse_dynamics(dynamics, false, 0.0).context("angle dynamics")
        })
        .transpose()?;
    let minimum_roundness = brush
        .minimum_roundness
        .as_ref()
        .and_then(UnitFloat::as_percentage_01)
        .unwrap_or(0.0);
    let roundness_dynamics = brush
        .roundness_dynamics
        .as_ref()
        .map(|dynamics| {
            parse_dynamics(dynamics, false, minimum_roundness).context("roundness dynamics")
        })
        .transpose()?;
    let tilt_scale = brush
        .tilt_scale
        .as_ref()
        .and_then(UnitFloat::as_percentage_01)
        .unwrap_or(2.0)
        .clamp(0.0, 2.0);
    let pose = if brush.use_brush_pose {
        BrushPose {
            pressure: brush
                .override_pose_pressure
                .then_some(brush.brush_pose_pressure.as_ref())
                .flatten()
                .map(|value| (value.value as f32 / 100.0).clamp(0.0, 1.0)),
            azimuth: brush
                .override_pose_angle
                .then(|| (brush.brush_pose_angle as f32).to_radians()),
            tilt_x: brush
                .override_pose_tilt_x
                .then(|| (brush.brush_pose_tilt_x as f32).to_radians()),
            tilt_y: brush
                .override_pose_tilt_y
                .then(|| (brush.brush_pose_tilt_y as f32).to_radians()),
        }
    } else {
        BrushPose::default()
    };
    let color_enabled = brush.use_color_dynamics;
    let hue_jitter = if color_enabled {
        brush
            .hue_jitter
            .as_ref()
            .and_then(UnitFloat::as_percentage_01)
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let saturation_jitter = if color_enabled {
        brush
            .saturation_jitter
            .as_ref()
            .and_then(UnitFloat::as_percentage_01)
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let value_jitter = if color_enabled {
        brush
            .value_jitter
            .as_ref()
            .and_then(UnitFloat::as_percentage_01)
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let purity = if color_enabled {
        brush
            .purity_jitter
            .as_ref()
            .map(|value| value.value as f32 / 100.0)
            .unwrap_or(0.0)
            .clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let color_dynamics = if color_enabled {
        brush
            .color_dynamics
            .as_ref()
            .or_else(|| paint_options.and_then(|options| options.color_dynamics.as_ref()))
            .map(|dynamics| parse_dynamics(dynamics, false, 0.0).context("color dynamics"))
            .transpose()?
    } else {
        None
    };
    let foreground_color = paint_options
        .and_then(|options| options.foreground_color.as_ref())
        .map(|color| {
            [
                (color.red as f32 / 255.0).clamp(0.0, 1.0),
                (color.green as f32 / 255.0).clamp(0.0, 1.0),
                (color.blue as f32 / 255.0).clamp(0.0, 1.0),
            ]
        });
    let has_hsv_jitter = hue_jitter > 0.0 || saturation_jitter > 0.0 || value_jitter > 0.0;
    let has_color_adjustment =
        has_hsv_jitter || purity != 0.0 || color_dynamics.is_some() || foreground_color.is_some();
    let color_adjustment = has_color_adjustment.then_some(ColorAdjustment {
        hue_jitter,
        saturation_jitter,
        value_jitter,
        purity,
        dynamics: color_dynamics,
        per_tip: brush.color_dynamics_per_tip,
        foreground_color,
    });
    let scatter = if brush.use_scatter {
        let scatter_dynamics = brush
            .scatter_dynamics
            .as_ref()
            .map(|dynamics| parse_dynamics(dynamics, false, 0.0).context("scatter dynamics"))
            .transpose()?;
        let count_dynamics = brush
            .count_dynamics
            .as_ref()
            .map(|dynamics| parse_dynamics(dynamics, false, 0.0).context("count dynamics"))
            .transpose()?;
        let count = brush.scatter_count.round();
        ensure!(
            brush.scatter_count.is_finite()
                && (brush.scatter_count - count).abs() < f64::EPSILON
                && (1.0..=16.0).contains(&count),
            "unsupported scatter count {}",
            brush.scatter_count
        );
        Some(Scatter {
            both_axes: brush.scatter_both_axes,
            scatter_dynamics,
            count: count as u32,
            count_dynamics,
        })
    } else {
        None
    };

    ensure!(!brush.protect_texture, "unsupported protect texture");
    ensure!(
        brush.interpretation != Some(false),
        "unsupported texture interpretation"
    );
    let (brush_texture, pattern_asset) = if brush.use_texture {
        let texture = brush
            .texture
            .as_ref()
            .context("missing texture pattern reference")?;
        let pattern_asset = pattern_assets
            .get(&texture.id)
            .copied()
            .with_context(|| format!("missing texture pattern {}", texture.id))?;
        let scale = brush
            .texture_scale
            .as_ref()
            .and_then(UnitFloat::as_percentage_01)
            .unwrap_or(1.0);
        ensure!(scale.is_finite() && scale > 0.0, "invalid texture scale");
        let depth = brush
            .texture_depth
            .as_ref()
            .and_then(UnitFloat::as_percentage_01)
            .unwrap_or(1.0);
        let minimum_depth = brush
            .texture_minimum_depth
            .as_ref()
            .and_then(UnitFloat::as_percentage_01)
            .unwrap_or(0.0);
        let depth_dynamics = brush
            .texture_depth_dynamics
            .as_ref()
            .map(|dynamics| {
                parse_dynamics(dynamics, false, minimum_depth).context("texture depth dynamics")
            })
            .transpose()?;
        let brightness = (brush.texture_brightness as f32 / 150.0).clamp(-1.0, 1.0);
        let contrast = if brush.texture_contrast < 0 {
            brush.texture_contrast as f32 / 50.0
        } else {
            brush.texture_contrast as f32 / 100.0
        }
        .clamp(-1.0, 1.0);
        let use_legacy = match &brush.tool_options {
            Some(ToolOptions::Paint(o)) => o.use_legacy,
            Some(ToolOptions::Smudge(o)) => o.use_legacy,
            Some(ToolOptions::Sh(o)) => o.use_legacy,
            Some(ToolOptions::Eraser(o)) => o.use_legacy,
            None => false,
        };

        (
            Some(BrushTexture {
                scale,
                inverted: brush.texture_inverted,
                depth,
                depth_dynamics,
                each_tip: brush.txt_c,
                blend_mode: parse_blend_mode(
                    brush
                        .texture_blend_mode
                        .unwrap_or(class::BlendMode::Multiply),
                ),
                brightness,
                contrast,
                use_legacy,
            }),
            Some(pattern_asset),
        )
    } else {
        (None, None)
    };
    let (dual_brush, dual_sample_asset) = parse_dual_brush(brush, sample_assets)?;
    let main_graph_options = MainGraphOptions {
        flow,
        size_dynamics,
        opacity_dynamics,
        flow_dynamics,
        angle_dynamics,
        roundness_dynamics,
        tilt_scale,
        flip_x_jitter: brush.flip_x_jitter,
        flip_y_jitter: brush.flip_y_jitter,
        pose,
        color_adjustment,
        scatter,
        brush_texture,
        pattern_asset,
        noise: brush.noise,
        dual_brush,
        dual_sample_asset,
    };

    let base_size = match &brush.brush {
        BrushTip::Computed(tip) => tip.diameter.value as f32,
        BrushTip::Sampled(tip) => tip.diameter.value as f32,
        BrushTip::DBrush(_) => bail!("unsupported dual brush tip in {}", brush.name),
    };
    let (spacing_effect, main_effect, inputs) = match &brush.brush {
        BrushTip::Computed(tip) => {
            ensure!(tip.interpolation, "unsupported computed interpolation");
            let inputs = graph::BrushInputs::new(
                base_size,
                None,
                main_graph_options.pattern_asset,
                main_graph_options.dual_sample_asset,
            )?;
            let (spacing, main) = graph::computed_graphs(
                ComputedMainTip {
                    diameter: tip.diameter.value as f32,
                    hardness: (tip.hardness.value / 100.0).clamp(0.0, 1.0) as f32,
                    angle: tip.angle.value.to_radians() as f32,
                    roundness: (tip.roundness.value / 100.0).clamp(0.001, 1.0) as f32,
                    flip_x: tip.flip_x,
                    flip_y: tip.flip_y,
                    spacing: (tip.spacing.value / 100.0).max(0.001) as f32,
                },
                main_graph_options,
                &inputs,
            )?;
            (spacing, main, inputs)
        }
        BrushTip::Sampled(tip) => {
            ensure!(tip.interpolation, "unsupported sampled interpolation");
            let sample_asset = sample_assets
                .get(&tip.id)
                .copied()
                .with_context(|| format!("sample not found {}", tip.id))?;
            let inputs = graph::BrushInputs::new(
                base_size,
                Some(sample_asset),
                main_graph_options.pattern_asset,
                main_graph_options.dual_sample_asset,
            )?;
            let (spacing, main) = graph::sampled_graphs(
                SampledMainTip {
                    sample_asset,
                    diameter: tip.diameter.value as f32,
                    angle: tip.angle.value.to_radians() as f32,
                    roundness: (tip.roundness.value / 100.0).clamp(0.001, 1.0) as f32,
                    flip_x: tip.flip_x,
                    flip_y: tip.flip_y,
                    spacing: (tip.spacing.value / 100.0).max(0.001) as f32,
                },
                main_graph_options,
                &inputs,
            )?;
            (spacing, main, inputs)
        }
        BrushTip::DBrush(_) => bail!("unsupported dual brush tip in {}", brush.name),
    };

    let postprocess_effect = graph::opacity_postprocess_effect(opacity, paint_blend_mode, &inputs)?;

    Ok(BrushPreset {
        metadata: BrushPresetMetadata {
            name: brush.name.clone(),
        },
        spacing_effect,
        main_effect,
        postprocess_effect,
        parameters: inputs.serialized_parameters().collect(),
    })
}
