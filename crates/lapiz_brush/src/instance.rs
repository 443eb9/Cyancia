use std::sync::{Arc, LazyLock};

use anyhow::{Context, Result, bail, ensure};
use indexmap::IndexMap;
use lapiz_assets::asset::{AssetHandle, AssetId};
use lapiz_effect::{
    asset::{EffectInputSlotId, EffectPassOutputSlotId},
    instance::EffectInstance,
    nodes::{PassInput, PassInputNode, PassOutput, PassOutputNode},
    render::{pass_input_ident, pass_output_ident},
};
use lapiz_render::wesl_jit;
use lapiz_shader_graph::{
    graph::{
        Graph, GraphResources, GraphVarIdentGenerator,
        node::GraphNodeRegistry,
        slot::{ErasedGraphValueType, GraphShaderStage},
        variable::GraphLiteral,
    },
    save::SerializableGraphLiteral,
};
use wgpu::BindGroupLayoutEntry;

use crate::{
    asset::{BrushPreset, BrushPresetMetadata, SerializableBrushParameter},
    render::graph::{
        BRUSH_GRAPH_TYPES, DAB_BOUNDS_OUTPUT, DAB_COLOR_OUTPUT,
        SPACING_OUTPUT, STROKE_BOUNDS_OUTPUT, STROKE_COLOR_OUTPUT, main_graph_nodes,
        postprocess_graph_nodes, spacing_graph_nodes,
    },
};

#[derive(Clone)]
pub struct BrushParameter {
    pub name: String,
    pub value: GraphLiteral,
}

pub struct CompiledGraph {
    pub main: String,
    pub bounds_eval: String,
}

pub struct CompiledBrushPreset {
    pub input_sampling: String,
    pub main_graph: CompiledGraph,
    pub stroke_postprocess_graphs: Vec<CompiledGraph>,
    pub parameter_declarations: String,
    pub parameter_layouts: Vec<BindGroupLayoutEntry>,
    pub parameters: Arc<[GraphLiteral]>,
}

pub struct BrushPresetInstance {
    brush_id: Option<AssetId<BrushPreset>>,
    metadata: BrushPresetMetadata,

    spacing_effect: EffectInstance,
    main_effect: EffectInstance,
    postprocess_effect: EffectInstance,
    parameters: IndexMap<EffectInputSlotId, BrushParameter>,
}

impl BrushPresetInstance {
    pub fn from_asset(
        handle: &AssetHandle<BrushPreset>,
    ) -> Result<Self> {
        let preset = handle
            .get()
            .map_err(|e| anyhow::anyhow!("Brush preset asset is not loaded yet: {e}"))?;
        let mut instance = Self::new(&preset)?;
        instance.brush_id = Some(handle.id());
        Ok(instance)
    }

    pub fn new(preset: &BrushPreset) -> Result<Self> {
        let spacing_effect = EffectInstance::from_asset(&preset.spacing_effect, spacing_effect_resources())?;
        let main_effect = EffectInstance::from_asset(&preset.main_effect, main_effect_resources())?;
        let postprocess_effect =
            EffectInstance::from_asset(&preset.postprocess_effect, postprocess_effect_resources())?;

        // Every effect input is a brush parameter. Persisted values win; new
        // or missing inputs fall back to the type's default literal.
        let mut parameters = IndexMap::new();
        for effect in [&spacing_effect, &main_effect, &postprocess_effect] {
            for (id, slot) in &effect.inputs {
                let value = preset
                    .parameters
                    .get(id)
                    .and_then(|persisted| {
                        persisted
                            .value
                            .deserialize(BRUSH_GRAPH_TYPES.as_ref())
                            .map_err(|error| {
                                log::warn!(
                                    "Brush parameter '{}' failed to deserialize, using default: {error}",
                                    slot.name
                                );
                                error
                            })
                            .ok()
                    })
                    .unwrap_or_else(|| {
                        GraphLiteral::new_boxed(slot.ty.default_literal(), slot.ty.clone())
                    });
                parameters.insert(
                    *id,
                    BrushParameter {
                        name: slot.name.clone(),
                        value,
                    },
                );
            }
        }

        Ok(Self {
            brush_id: None,
            metadata: preset.metadata.clone(),
            spacing_effect,
            main_effect,
            postprocess_effect,
            parameters,
        })
    }

    pub fn as_asset(&self) -> Result<BrushPreset> {
        let mut parameters = IndexMap::new();
        for (id, parameter) in &self.parameters {
            parameters.insert(
                *id,
                SerializableBrushParameter {
                    name: parameter.name.clone(),
                    value: SerializableGraphLiteral::serialize(&parameter.value)?,
                },
            );
        }

        Ok(BrushPreset {
            metadata: self.metadata.clone(),
            spacing_effect: self.spacing_effect.as_asset()?,
            main_effect: self.main_effect.as_asset()?,
            postprocess_effect: self.postprocess_effect.as_asset()?,
            parameters,
        })
    }

    pub fn asset_id(&self) -> Option<AssetId<BrushPreset>> {
        self.brush_id
    }

    pub fn metadata(&self) -> &BrushPresetMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut BrushPresetMetadata {
        &mut self.metadata
    }

    pub fn spacing_effect(&self) -> &EffectInstance {
        &self.spacing_effect
    }

    pub fn spacing_effect_mut(&mut self) -> &mut EffectInstance {
        &mut self.spacing_effect
    }

    pub fn main_effect(&self) -> &EffectInstance {
        &self.main_effect
    }

    pub fn main_effect_mut(&mut self) -> &mut EffectInstance {
        &mut self.main_effect
    }

    pub fn postprocess_effect(&self) -> &EffectInstance {
        &self.postprocess_effect
    }

    pub fn postprocess_effect_mut(&mut self) -> &mut EffectInstance {
        &mut self.postprocess_effect
    }

    pub fn parameters(&self) -> &IndexMap<EffectInputSlotId, BrushParameter> {
        &self.parameters
    }

    pub fn parameters_mut(&mut self) -> &mut IndexMap<EffectInputSlotId, BrushParameter> {
        &mut self.parameters
    }

    pub fn update_parameter(
        &mut self,
        id: &EffectInputSlotId,
        message: lapiz_shader_graph::graph::slot::ErasedGraphLiteralUpdateMessage,
    ) {
        if let Some(parameter) = self.parameters.get_mut(id) {
            parameter.value.update(message);
        }
    }

    #[tracing::instrument(skip_all, name = "compile_brush_preset")]
    pub fn compile(&self) -> Result<CompiledBrushPreset> {
        let mut parameters = Vec::new();
        let mut declarations = String::new();
        let mut layouts = Vec::new();
        let mut binding = 0u32;

        for effect in [&self.spacing_effect, &self.main_effect, &self.postprocess_effect] {
            for (id, slot) in &effect.inputs {
                let parameter = self
                    .parameters
                    .get(id)
                    .with_context(|| format!("Brush effect input '{:?}' has no parameter", id))?;
                let port = pass_input_port_of(effect, *id)?;
                let (next, layout, extended) = slot.ty.push_shader_layout(
                    &pass_input_ident(port),
                    GraphShaderStage::Input,
                    0,
                    crate::render::PARAMETER_BASE_BINDING + binding,
                    lapiz_render::bind_group_layout_entries::DynamicBindGroupLayoutEntries::new(
                        wgpu::ShaderStages::COMPUTE,
                    ),
                    declarations,
                )?;
                declarations = extended;
                ensure!(
                    next == crate::render::PARAMETER_BASE_BINDING + binding + 1,
                    "brush parameters must occupy exactly one binding slot each"
                );
                layouts.extend(layout.to_vec());
                parameters.push(parameter.value.clone());
                binding += 1;
            }
        }

        let input_sampling = self.compile_spacing_pass(&declarations)?;
        let main_graph = self.compile_main_pass(&declarations)?;
        let stroke_postprocess_graphs = self.compile_postprocess_passes(&declarations)?;

        Ok(CompiledBrushPreset {
            input_sampling,
            main_graph,
            stroke_postprocess_graphs,
            parameter_declarations: declarations,
            parameter_layouts: layouts,
            parameters: parameters.into(),
        })
    }

    fn compile_spacing_pass(&self, parameter_declarations: &str) -> Result<String> {
        let graph = single_pass_graph(&self.spacing_effect, "spacing")?;
        let spacing_port = output_port_of(&self.spacing_effect, SPACING_OUTPUT)?;
        let spacing_ty = self
            .spacing_effect
            .outputs
            .iter()
            .find(|(_, slot)| slot.name == SPACING_OUTPUT)
            .map(|(_, slot)| slot.ty.clone())
            .with_context(|| format!("Brush spacing effect has no '{SPACING_OUTPUT}' output"))?;

        let (_, _, code) = graph.compile(Vec::new(), GraphVarIdentGenerator::default())?;
        let spacing_var = pass_output_ident(spacing_port);
        let body = format!(
            "var {spacing_var}: {};\n{code}return {spacing_var};\n",
            conventional_type_name(&spacing_ty)?,
        );

        let shader = include_str!("render/brush_sample.wesl")
            .replace("//CODEGENFLAG_COMPUTED_GRAPH_REQUIRED_SPACING", &body)
            .replace(
                "//CODEGENFLAG_EXTERNAL_VARIABLE_BINDINGS",
                parameter_declarations,
            );

        compile_brush_wesl(shader, false, false)
    }

    fn compile_main_pass(&self, parameter_declarations: &str) -> Result<CompiledGraph> {
        let graph = single_pass_graph(&self.main_effect, "main")?;
        let color_port = output_port_of(&self.main_effect, DAB_COLOR_OUTPUT)?;
        let bounds_port = output_port_of(&self.main_effect, DAB_BOUNDS_OUTPUT)?;

        compile_two_stage_pass(
            graph,
            color_port,
            bounds_port,
            parameter_declarations,
            false,
        )
    }

    fn compile_postprocess_passes(&self, parameter_declarations: &str) -> Result<Vec<CompiledGraph>> {
        self.postprocess_effect
            .passes
            .values()
            .map(|pass| {
                let color_port = output_port_in_pass(&self.postprocess_effect, pass, STROKE_COLOR_OUTPUT)
                    .with_context(|| format!("Brush postprocess pass '{}' has no '{STROKE_COLOR_OUTPUT}' output", pass.name))?;
                let bounds_port = output_port_in_pass(&self.postprocess_effect, pass, STROKE_BOUNDS_OUTPUT)
                    .with_context(|| format!("Brush postprocess pass '{}' has no '{STROKE_BOUNDS_OUTPUT}' output", pass.name))?;
                compile_two_stage_pass(
                    &pass.graph,
                    color_port,
                    bounds_port,
                    parameter_declarations,
                    true,
                )
            })
            .collect()
    }
}

fn compile_two_stage_pass(
    graph: &Graph,
    color_port: EffectPassOutputSlotId,
    bounds_port: EffectPassOutputSlotId,
    parameter_declarations: &str,
    postprocess: bool,
) -> Result<CompiledGraph> {
    let (_, _, code) = graph.compile(Vec::new(), GraphVarIdentGenerator::default())?;
    let color_var = pass_output_ident(color_port);
    let bounds_var = pass_output_ident(bounds_port);
    let body = format!(
        "var {color_var}: vec4f;\nvar {bounds_var}: Rect;\n{code}\
         @if(BOUNDS_EVAL) {{ set_output_pixel_bounds({bounds_var}); }}\n\
         @if(!BOUNDS_EVAL) {{ set_output_color(pixel_pos, {color_var}); }}\n",
    );

    let mut compiled = CompiledGraph {
        main: String::new(),
        bounds_eval: String::new(),
    };
    for bounds_eval in [false, true] {
        let shader = include_str!("render/brush_template.wesl")
            .replace("//CODEGENFLAG_COMPILED_GRAPH", &body)
            .replace(
                "//CODEGENFLAG_EXTERNAL_VARIABLE_BINDINGS",
                parameter_declarations,
            );
        let wesl = compile_brush_wesl(shader, postprocess, bounds_eval)?;
        if bounds_eval {
            compiled.bounds_eval = wesl;
        } else {
            compiled.main = wesl;
        }
    }
    Ok(compiled)
}

fn compile_brush_wesl(
    shader: String,
    postprocess: bool,
    bounds_eval: bool,
) -> Result<String> {
    wesl_jit::compile_wesl_with_config_and_include(
        shader,
        &[&lapiz_image::image::PACKAGE, &lapiz_render::render::PACKAGE],
        |resolver| {
            resolver.add_module(
                "package::brush_types".parse().unwrap(),
                include_str!("render/brush_types.wesl").into(),
            );
        },
        |compiler| {
            compiler.set_feature("POSTPROCESS", postprocess);
            compiler.set_feature("BOUNDS_EVAL", bounds_eval);
            compiler.set_feature("EVAL", false);
        },
    )
    .context("Brush WESL compilation failed")
}

// The conventional outputs live in shader function variables, so their types
// must be nameable in the brush templates.
fn conventional_type_name(ty: &Arc<dyn ErasedGraphValueType>) -> Result<&'static str> {
    ty.wgsl_type_name().with_context(|| {
        format!(
            "Brush conventional outputs must be nameable value types, got '{}'",
            ty.id().id
        )
    })
}

fn single_pass_graph<'a>(effect: &'a EffectInstance, label: &str) -> Result<&'a Graph> {
    if effect.passes.len() != 1 {
        bail!("Brush {label} effect must have exactly one pass");
    }
    Ok(&effect.passes.first().unwrap().1.graph)
}

// The pass input port bound to an effect input.
fn pass_input_port_of(
    effect: &EffectInstance,
    effect_input: EffectInputSlotId,
) -> Result<lapiz_effect::asset::EffectPassInputSlotId> {
    for (_id, pass) in &effect.passes {
        for node in pass.graph.iter_nodes() {
            let Some(state) = node.data.state::<PassInputNode>() else {
                continue;
            };
            if state.input == Some(PassInput::Effect(effect_input)) {
                return Ok(state.id);
            }
        }
    }
    bail!(
        "Brush effect input {:?} is not bound by any pass input node",
        effect_input
    );
}

// The output port of the pass output node bound to a named effect output.
fn output_port_of(effect: &EffectInstance, output_name: &str) -> Result<EffectPassOutputSlotId> {
    if effect.passes.len() != 1 {
        bail!("Brush effect must have exactly one pass to resolve '{output_name}'");
    }
    let pass = effect.passes.first().unwrap().1;
    output_port_in_pass(effect, pass, output_name)
}

fn output_port_in_pass(
    effect: &EffectInstance,
    pass: &lapiz_effect::instance::EffectPass,
    output_name: &str,
) -> Result<EffectPassOutputSlotId> {
    let output_id = effect
        .outputs
        .iter()
        .find(|(_, slot)| slot.name == output_name)
        .map(|(id, _)| *id)
        .with_context(|| format!("Effect has no output named '{output_name}'"))?;
    for node in pass.graph.iter_nodes() {
        let Some(state) = node.data.state::<PassOutputNode>() else {
            continue;
        };
        if matches!(&state.output, Some(PassOutput::Effect(id)) if *id == output_id) {
            return Ok(state.id);
        }
    }
    bail!("No pass output node is bound to effect output '{output_name}'")
}

pub static SPACING_GRAPH_NODES: LazyLock<Arc<GraphNodeRegistry>> =
    LazyLock::new(|| Arc::new(spacing_graph_nodes()));
pub static MAIN_GRAPH_NODES: LazyLock<Arc<GraphNodeRegistry>> =
    LazyLock::new(|| Arc::new(main_graph_nodes()));
pub static POSTPROCESS_GRAPH_NODES: LazyLock<Arc<GraphNodeRegistry>> =
    LazyLock::new(|| Arc::new(postprocess_graph_nodes()));

pub fn spacing_effect_resources() -> GraphResources {
    crate::render::graph::brush_graph_resources(SPACING_GRAPH_NODES.clone())
}

pub fn main_effect_resources() -> GraphResources {
    crate::render::graph::brush_graph_resources(MAIN_GRAPH_NODES.clone())
}

pub fn postprocess_effect_resources() -> GraphResources {
    crate::render::graph::brush_graph_resources(POSTPROCESS_GRAPH_NODES.clone())
}

pub struct GraphFunctionInstance {
    graph_function: lapiz_shader_graph::graph::function::GraphFunction,
}

impl GraphFunctionInstance {
    pub fn new(graph_function: lapiz_shader_graph::graph::function::GraphFunction) -> Self {
        Self { graph_function }
    }

    pub fn graph_function(&self) -> &lapiz_shader_graph::graph::function::GraphFunction {
        &self.graph_function
    }

    pub fn graph_function_mut(&mut self) -> &mut lapiz_shader_graph::graph::function::GraphFunction {
        &mut self.graph_function
    }
}
