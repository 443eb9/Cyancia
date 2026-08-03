use std::sync::LazyLock;

use cyancia_shader_graph::{
    editor::{GraphEditor, GraphEditorMessage},
    graph::{
        Graph, GraphData, GraphResources, node::GraphNodeRegistry, variable::GraphTypeRegistry,
    },
    wgsl_std::{
        builtin_nodes, builtin_types,
        nodes::{GraphInputNode, GraphOutputNode},
    },
};

#[derive(Default, Clone)]
struct DemoData;

impl GraphData for DemoData {
    fn type_registry() -> &'static GraphTypeRegistry {
        LazyLock::force(&TYPE_REGISTRY)
    }

    fn node_registry() -> &'static GraphNodeRegistry<Self> {
        LazyLock::force(&NODE_REGISTRY)
    }
}

static NODE_REGISTRY: LazyLock<GraphNodeRegistry<DemoData>> = LazyLock::new(|| {
    let mut nodes = builtin_nodes();
    nodes.register::<GraphInputNode>();
    nodes.register::<GraphOutputNode>();
    nodes
});

static TYPE_REGISTRY: LazyLock<GraphTypeRegistry> = LazyLock::new(builtin_types);

struct DemoEditor {
    graph: Graph<DemoData>,
}

impl DemoEditor {
    fn new() -> Self {
        Self {
            graph: Graph::new(GraphResources::default()),
        }
    }

    fn view(&self) -> iced_core::Element<'_, GraphEditorMessage, iced::Theme, iced_wgpu::Renderer> {
        GraphEditor::new(&self.graph, false).into()
    }

    fn update(&mut self, message: GraphEditorMessage) {
        match message {
            GraphEditorMessage::NodeCreateRequest(position, name) => {
                let node = DemoData::node_registry()
                    .get(name)
                    .expect("graph editor requested an unregistered node");
                self.graph.add_boxed_node(position, node);
            }
            GraphEditorMessage::NodeMoveRequest(position, id) => {
                self.graph
                    .get_node_mut(&id)
                    .expect("graph editor moved a missing node")
                    .position = position;
            }
            GraphEditorMessage::NodeDeleteRequest(id) => self.graph.delete_node(&id),
            GraphEditorMessage::EdgeCreateRequest(from, to) => {
                self.graph.connect_slots(from, to);
            }
            GraphEditorMessage::EdgeRemoveRequest(to) => self.graph.disconnect_slot(to),
            GraphEditorMessage::NodeUpdate(message) => self.graph.update_node(message),
        }
    }
}

fn main() -> iced::Result {
    iced::application(DemoEditor::new, DemoEditor::update, DemoEditor::view)
        .window_size((1280.0, 800.0))
        .run()
}
