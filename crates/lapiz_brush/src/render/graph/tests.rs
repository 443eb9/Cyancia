use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context as _, Result, anyhow};
use iced_core::Point;
use lapiz_assets::store::AssetRegistry;
use lapiz_effect::nodes::{PassInputNode, PassOutputNode};
use lapiz_image::{blend_modes::BlendMode, image, texel::TexelType};
use lapiz_render::{bind_group_layout_entries::DynamicBindGroupLayoutEntries, render, wesl_jit};
use lapiz_shader_graph::{
    graph::{
        Graph, GraphResources, GraphVarIdentGenerator,
        node::{ErasedGraphNode, GraphNode, GraphNodeCreateSlotsContext, GraphNodeRegistry},
        slot::{ErasedGraphValueType, GraphShaderStage},
    },
    wgsl_std::nodes::{function::GraphFunctionNode, graph_io::GraphInputNode, repeat::RepeatNode},
};
use wesl::syntax::Ident;
use wgpu::{
    ShaderStages,
    naga::{
        front::wgsl,
        valid::{Capabilities, ValidationFlags, Validator},
    },
};

use super::{
    BRUSH_GRAPH_TYPES, BRUSH_SAMPLE_BUILTIN, BlendColorNode, BlendWithInputNode,
    BlendWithLayerNode, ComputedPenInputValueType, EllipticalMaskNode, FilterWithinBoundsNode,
    INITIAL_PEN_INPUT_BUILTIN, main_builtin_types, main_graph_nodes, postprocess_builtin_types,
    postprocess_graph_nodes, spacing_graph_nodes,
};
use crate::brush;

fn resources(nodes: GraphNodeRegistry) -> GraphResources {
    GraphResources {
        type_registry: BRUSH_GRAPH_TYPES.clone(),
        node_registry: Arc::new(nodes),
        assets: AssetRegistry::new_in_memory(Arc::new(Default::default())),
    }
}

fn compile_graph(
    graph: &Graph,
    builtins: &BTreeMap<String, Arc<dyn ErasedGraphValueType>>,
    is_eval: bool,
) -> Result<String> {
    let mut declarations = String::new();
    let mut binding = 0;
    for (name, ty) in builtins {
        let (next, _, shader) = ty.push_shader_layout(
            name,
            GraphShaderStage::Input,
            0,
            binding,
            DynamicBindGroupLayoutEntries::new(ShaderStages::COMPUTE),
            declarations,
        )?;
        binding = next;
        declarations = shader;
        if let Some(body) = ty.generate_extra_shader_body(GraphShaderStage::Input, name) {
            declarations.push_str(&body);
        }
    }

    let mut inputs = Vec::new();
    for (index, variable) in graph.signature().inputs.values().enumerate() {
        let name = format!("input_{index}");
        let (next, _, shader) = variable.ty().push_shader_layout(
            &name,
            GraphShaderStage::Input,
            0,
            binding,
            DynamicBindGroupLayoutEntries::new(ShaderStages::COMPUTE),
            declarations,
        )?;
        binding = next;
        declarations = shader;
        inputs.push(Ident::new(name).into());
    }

    let (_, _, code, bodies) = graph.compile(inputs, GraphVarIdentGenerator::default())?;
    let mut shader = format!(
        "{declarations}\nfn graph(dispatch_index: vec2i) {{\n{code}\n}}\n\
         @compute @workgroup_size(1) fn main() {{ graph(vec2i(0)); }}\n"
    );
    for body in bodies.into_iter().collect::<BTreeMap<_, _>>().into_values() {
        body.inject_body(&mut shader);
    }
    let shader = wesl_jit::compile_wesl_with_config(
        shader,
        &[&image::PACKAGE, &render::PACKAGE, &brush::PACKAGE],
        |compiler| {
            compiler.set_feature("EVAL", is_eval);
        },
    )?;
    let module =
        wgsl::parse_str(&shader).map_err(|error| anyhow!(error.emit_to_string(&shader)))?;
    Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module)?;
    Ok(shader)
}

fn add_node(graph: &mut Graph, node: Box<dyn ErasedGraphNode>) -> Result<()> {
    let node = graph.add_boxed_node(Point::ORIGIN, node);
    let inputs = graph
        .get_node(&node)
        .unwrap()
        .data
        .create_inputs(GraphNodeCreateSlotsContext {
            resources: graph.resources(),
        });
    for (index, input) in inputs.into_iter().enumerate() {
        if input
            .ty
            .literal_to_code(input.ty.default_literal().as_ref())
            .is_some()
        {
            continue;
        }

        let source = graph.add_node(Point::ORIGIN, GraphInputNode);
        graph.update_node_state::<GraphInputNode>(source, |state| {
            state.name = format!("input_{index}");
            state.ty = Some(input.ty.id().id);
        });
        graph.connect_slots_by_index(source, 0, node, index);
    }
    Ok(())
}

#[test]
fn registered_brush_nodes_compile_in_both_stages() -> Result<()> {
    let formats = (TexelType::RGBA8, TexelType::A8);
    let main_builtins = main_builtin_types(formats.0, formats.1);
    let postprocess_builtins = postprocess_builtin_types(formats.0, formats.1);
    let mut spacing_builtins = super::brush_builtin_types(formats.0, formats.1);
    spacing_builtins.insert(
        BRUSH_SAMPLE_BUILTIN.into(),
        Arc::new(ComputedPenInputValueType),
    );
    spacing_builtins.insert(
        INITIAL_PEN_INPUT_BUILTIN.into(),
        Arc::new(ComputedPenInputValueType),
    );
    for (nodes, builtins) in [
        (main_graph_nodes(), main_builtins),
        (postprocess_graph_nodes(), postprocess_builtins),
        (spacing_graph_nodes(), spacing_builtins),
    ] {
        let resources = resources(nodes);
        for (name, node) in resources.node_registry.all() {
            if *name == GraphNode::id(&PassInputNode) || *name == GraphNode::id(&PassOutputNode) {
                continue;
            }
            let mut graph = Graph::new(resources.clone());
            add_node(&mut graph, node.clone())?;
            for is_eval in [false, true] {
                compile_graph(&graph, &builtins, is_eval)
                    .with_context(|| format!("node {name}, EVAL={is_eval}"))?;
            }
        }
    }
    Ok(())
}

#[test]
fn blend_mode_function_references_compile() -> Result<()> {
    let resources = resources(main_graph_nodes());
    let builtins = main_builtin_types(TexelType::RGBA8, TexelType::A8);
    for mode in BlendMode::ALL {
        let mut graph = Graph::new(resources.clone());
        let color = graph.add_node(Point::ORIGIN, BlendColorNode);
        graph.update_node_state::<BlendColorNode>(color, |state| state.blend_mode = mode);
        let input = graph.add_node(Point::ORIGIN, BlendWithInputNode);
        graph.update_node_state::<BlendWithInputNode>(input, |state| state.blend_mode = mode);
        let layer = graph.add_node(Point::ORIGIN, BlendWithLayerNode);
        graph.update_node_state::<BlendWithLayerNode>(layer, |state| state.blend_mode = mode);
        for is_eval in [false, true] {
            compile_graph(&graph, &builtins, is_eval)
                .with_context(|| format!("blend mode {mode:?}, EVAL={is_eval}"))?;
        }
    }
    Ok(())
}

#[test]
fn helper_bodies_are_deduplicated_and_available_without_pen_inputs() -> Result<()> {
    let mut graph = Graph::new(resources(postprocess_graph_nodes()));
    for _ in 0..2 {
        graph.add_node(Point::ORIGIN, FilterWithinBoundsNode);
        graph.add_node(Point::ORIGIN, EllipticalMaskNode);
    }
    let (_, _, _, bodies) = graph.compile(Vec::new(), GraphVarIdentGenerator::default())?;
    assert_eq!(bodies.len(), 2);
    for is_eval in [false, true] {
        compile_graph(
            &graph,
            &postprocess_builtin_types(TexelType::RGBA8, TexelType::A8),
            is_eval,
        )?;
    }
    Ok(())
}

#[test]
fn function_and_repeat_subgraphs_collect_node_helpers() -> Result<()> {
    let builtins = postprocess_builtin_types(TexelType::RGBA8, TexelType::A8);
    let resources = resources(postprocess_graph_nodes());
    let mut body = Graph::new(resources.clone());
    body.add_node(Point::ORIGIN, FilterWithinBoundsNode);
    body.add_node(Point::ORIGIN, EllipticalMaskNode);
    let mut function_graph = Graph::new(resources.clone());
    let function = function_graph.add_node(Point::ORIGIN, GraphFunctionNode);
    function_graph
        .update_node_state::<GraphFunctionNode>(function, |state| state.cached_graph = Some(body));

    let mut graph = Graph::new(resources);
    let repeat = graph.add_node(Point::ORIGIN, RepeatNode);
    graph.update_node_state::<RepeatNode>(repeat, |state| {
        *GraphNode::subgraphs_mut(&mut RepeatNode, state).remove(0) = function_graph;
    });
    let (_, _, _, bodies) = graph.compile(Vec::new(), GraphVarIdentGenerator::default())?;
    assert_eq!(bodies.len(), 2);
    for is_eval in [false, true] {
        compile_graph(&graph, &builtins, is_eval)?;
    }
    Ok(())
}
