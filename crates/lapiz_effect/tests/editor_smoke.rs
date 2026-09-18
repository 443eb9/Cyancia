//! Editor construction and io editing smoke test. Views are built without a
//! renderer: element construction alone exercises every node and panel builder.

use std::{io::Cursor, sync::Arc, sync::LazyLock};

use iced_core::{Element, Point};
use indexmap::IndexMap;
use lapiz_assets::loader::AssetSerializer;
use lapiz_effect::{
    asset::{
        EffectAssetSerializer, EffectInputSlotId, EffectOutputSlotId, EffectPassDispatchStrategy,
        EffectPassId, EffectPassInputSlotId, EffectPassOutputSlotId,
    },
    editor::{EffectEditorMessage, EffectEditorState, EffectEditorView, EffectIoEditorMessage},
    instance::{EffectInputSlot, EffectInstance, EffectOutputSlot, EffectPass},
    nodes::{
        PassInput, PassInputNode, PassOutput, PassOutputDef, PassOutputNode, TypeChoice,
        effect_nodes,
    },
};
use lapiz_shader_graph::{
    GraphRenderer, GraphTheme,
    graph::{
        Graph, GraphResources, function::ASSET_GRAPH_FUNCTION_STORAGE, node::GraphNodeId,
        node::GraphNodeRegistry, slot::ErasedGraphValueType, variable::GraphTypeRegistry,
    },
    wgsl_std::{builtin_types, types::F32Type},
};
use uuid::Uuid;

static TYPE_REGISTRY: LazyLock<Arc<GraphTypeRegistry>> =
    LazyLock::new(|| Arc::new(builtin_types()));

static NODE_REGISTRY: LazyLock<Arc<GraphNodeRegistry>> = LazyLock::new(|| Arc::new(effect_nodes()));

fn graph_resources() -> GraphResources {
    GraphResources {
        type_registry: TYPE_REGISTRY.clone(),
        node_registry: NODE_REGISTRY.clone(),
        functions: ASSET_GRAPH_FUNCTION_STORAGE.clone(),
    }
}

fn input_node(
    graph: &mut Graph,
    source: PassInput,
    ty: Arc<dyn ErasedGraphValueType>,
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
    ty: Arc<dyn ErasedGraphValueType>,
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

fn demo_instance() -> (EffectInstance, EffectPassId, EffectInputSlotId) {
    let f32_ty: Arc<dyn ErasedGraphValueType> = Arc::new(F32Type);
    let input_id = EffectInputSlotId::new(Uuid::new_v4());
    let output_id = EffectOutputSlotId::new(Uuid::new_v4());

    let mut producer = Graph::new(graph_resources());
    let (source, _) = input_node(&mut producer, PassInput::Effect(input_id), f32_ty.clone());
    let (buffer, buffer_port) = output_node(
        &mut producer,
        PassOutput::Pass(PassOutputDef {
            name: "doubled".into(),
            ty: f32_ty.clone(),
        }),
        f32_ty.clone(),
    );
    producer.connect_slots_by_index(source, 0, buffer, 0);

    let mut consumer = Graph::new(graph_resources());
    let (cross, _) = input_node(&mut consumer, PassInput::Pass(buffer_port), f32_ty.clone());
    let (export, _) = output_node(&mut consumer, PassOutput::Effect(output_id), f32_ty.clone());
    consumer.connect_slots_by_index(cross, 0, export, 0);

    let producer_pass = EffectPassId::new(Uuid::new_v4());
    let mut instance = EffectInstance {
        name: "Smoke".into(),
        passes: IndexMap::from([
            (
                producer_pass,
                EffectPass {
                    name: "Producer".into(),
                    graph: producer,
                    dispatch_strategy: EffectPassDispatchStrategy::Once,
                },
            ),
            (
                EffectPassId::new(Uuid::new_v4()),
                EffectPass {
                    name: "Consumer".into(),
                    graph: consumer,
                    dispatch_strategy: EffectPassDispatchStrategy::Once,
                },
            ),
        ]),
        inputs: IndexMap::from([(
            input_id,
            EffectInputSlot {
                name: "Strength".into(),
                id: input_id,
                ty: f32_ty.clone(),
            },
        )]),
        outputs: IndexMap::from([(
            output_id,
            EffectOutputSlot {
                name: "Result".into(),
                id: output_id,
                ty: f32_ty,
            },
        )]),
    };
    instance.sync_pass_graph_effect_properties().unwrap();
    (instance, producer_pass, input_id)
}

type EditorElement<'a> = Element<'a, EffectEditorMessage, GraphTheme, GraphRenderer>;

fn view<'a>(instance: &'a EffectInstance, state: &'a EffectEditorState) -> EditorElement<'a> {
    EffectEditorView::new(instance, state).into()
}

#[test]
fn editor_views_construct_and_edits_round_trip() {
    let (mut instance, producer_pass, input_id) = demo_instance();
    let mut state = EffectEditorState::new(graph_resources());

    let _ = view(&instance, &state);

    state.update(&mut instance, EffectEditorMessage::OpenPass(producer_pass));
    let _ = view(&instance, &state);

    state.update(&mut instance, EffectEditorMessage::BackToPassList);
    let _ = view(&instance, &state);

    let f32_choice = TypeChoice {
        label: "f32".into(),
        ty: Arc::new(F32Type),
    };
    state.update(
        &mut instance,
        EffectEditorMessage::Io(EffectIoEditorMessage::RenameInput(
            input_id,
            "Amount".into(),
        )),
    );
    // The io editor component resolves add-form buffers internally and only
    // bubbles a single AddInput message.
    state.update(
        &mut instance,
        EffectEditorMessage::Io(EffectIoEditorMessage::AddInput {
            name: "Extra".into(),
            ty: f32_choice.clone(),
        }),
    );
    assert_eq!(instance.inputs.len(), 2);
    assert_eq!(instance.inputs[&input_id].name, "Amount");

    let extra_id = *instance.inputs.keys().find(|id| **id != input_id).unwrap();
    state.update(
        &mut instance,
        EffectEditorMessage::Io(EffectIoEditorMessage::MoveInput { index: 1, up: true }),
    );
    assert_eq!(*instance.inputs.get_index(0).unwrap().0, extra_id);
    state.update(
        &mut instance,
        EffectEditorMessage::Io(EffectIoEditorMessage::RetypeInput(input_id, f32_choice)),
    );
    state.update(
        &mut instance,
        EffectEditorMessage::Io(EffectIoEditorMessage::RemoveInput(extra_id)),
    );
    assert_eq!(instance.inputs.len(), 1);
    let _ = view(&instance, &state);

    // Pass ops: rename, move, add, remove.
    state.update(
        &mut instance,
        EffectEditorMessage::PassRenameToggled(producer_pass),
    );
    state.update(
        &mut instance,
        EffectEditorMessage::PassRenamed(producer_pass, "Generator".into()),
    );
    assert_eq!(instance.passes[&producer_pass].name, "Generator");
    state.update(
        &mut instance,
        EffectEditorMessage::PassMoveRequested { index: 0, up: true },
    );
    state.update(&mut instance, EffectEditorMessage::PassAddRequested);
    assert_eq!(instance.passes.len(), 3);
    assert!(state.open_pass.is_some());
    let _ = view(&instance, &state);
    state.update(&mut instance, EffectEditorMessage::BackToPassList);
    let new_pass_id = *instance.passes.keys().last().unwrap();
    state.update(
        &mut instance,
        EffectEditorMessage::PassRemoveRequested(new_pass_id),
    );
    assert_eq!(instance.passes.len(), 2);
    state.update(
        &mut instance,
        EffectEditorMessage::PassRemoveRequested(producer_pass),
    );
    assert_eq!(instance.passes.len(), 1);
    let _ = view(&instance, &state);

    let serializer = EffectAssetSerializer;
    let mut encoded = Vec::new();
    serializer
        .write(&instance.as_asset().unwrap(), &mut encoded)
        .unwrap();
    let reloaded = EffectInstance::from_asset(
        &serializer.read(&mut Cursor::new(encoded)).unwrap(),
        graph_resources(),
    )
    .unwrap();
    assert_eq!(reloaded.inputs.len(), 1);
    let _ = view(&reloaded, &EffectEditorState::new(graph_resources()));
}
