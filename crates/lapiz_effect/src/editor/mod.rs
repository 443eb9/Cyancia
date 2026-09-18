use std::collections::{HashMap, HashSet};

use iced_core::{Length, alignment::Vertical};
use iced_widget::{Column, column, component::component, row};
use lapiz_i18n::t;
use lapiz_shader_graph::{
    GraphElement,
    editor::{GraphEditor, GraphEditorMessage, GraphEditorState},
    graph::{Graph, GraphResources},
};
use lapiz_widgets::{
    button::{self, Button},
    icon,
    label::Label,
    panel::Panel,
    scrollable::Scrollable,
    text_input::TextInput,
};
use uuid::Uuid;

use crate::{
    asset::{EffectPassDispatchStrategy, EffectPassId, EffectPassOutputSlotId},
    instance::{EffectInstance, EffectPass},
    nodes::{PassInput, PassInputNode, PassOutput, PassOutputNode},
};

mod io;

pub use io::*;

pub struct EffectEditorState {
    pub open_pass: Option<EffectPassId>,
    pub graph_editor_states: HashMap<EffectPassId, GraphEditorState>,
    pub resources: GraphResources,
    pub renaming_pass: Option<EffectPassId>,
}

impl EffectEditorState {
    pub fn new(resources: GraphResources) -> Self {
        Self {
            open_pass: None,
            graph_editor_states: HashMap::new(),
            resources,
            renaming_pass: None,
        }
    }

    pub fn update(&mut self, instance: &mut EffectInstance, message: EffectEditorMessage) {
        match message {
            EffectEditorMessage::OpenPass(pass_id) => {
                self.graph_editor_states.entry(pass_id).or_default();
                self.open_pass = Some(pass_id);
            }
            EffectEditorMessage::BackToPassList => self.open_pass = None,
            EffectEditorMessage::Graph(message) => {
                let pass_id = self
                    .open_pass
                    .expect("graph messages only arrive while a pass is open");
                let pass = instance
                    .passes
                    .get_mut(&pass_id)
                    .expect("the open pass exists");
                let editor_state = self
                    .graph_editor_states
                    .get_mut(&pass_id)
                    .expect("graph editor state is created when a pass is opened");
                editor_state.update(&mut pass.graph, message);
                resync(instance);
            }
            EffectEditorMessage::Io(message) => {
                io::apply(instance, message);
                resync(instance);
            }
            EffectEditorMessage::PassRenamed(pass_id, name) => {
                if let Some(pass) = instance.passes.get_mut(&pass_id) {
                    pass.name = name;
                }
                resync(instance);
            }
            EffectEditorMessage::PassRenameToggled(pass_id) => {
                self.renaming_pass = if self.renaming_pass == Some(pass_id) {
                    None
                } else {
                    Some(pass_id)
                };
            }
            EffectEditorMessage::PassMoveRequested { index, up } => {
                io::move_slot(&mut instance.passes, index, up);
            }
            EffectEditorMessage::PassRemoveRequested(pass_id) => {
                instance.passes.shift_remove(&pass_id);
                self.graph_editor_states.remove(&pass_id);
                if self.open_pass == Some(pass_id) {
                    self.open_pass = None;
                }
                resync(instance);
            }
            EffectEditorMessage::PassAddRequested => {
                let pass_id = EffectPassId::new(Uuid::new_v4());
                let name = format!("Pass {}", instance.passes.len() + 1);
                instance.passes.insert(
                    pass_id,
                    EffectPass {
                        name,
                        graph: Graph::new(self.resources.clone()),
                        dispatch_strategy: EffectPassDispatchStrategy::Once,
                    },
                );
                self.graph_editor_states.entry(pass_id).or_default();
                self.open_pass = Some(pass_id);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum EffectEditorMessage {
    OpenPass(EffectPassId),
    BackToPassList,
    Graph(GraphEditorMessage),
    Io(EffectIoEditorMessage),
    PassRenamed(EffectPassId, String),
    PassRenameToggled(EffectPassId),
    PassMoveRequested { index: usize, up: bool },
    PassRemoveRequested(EffectPassId),
    PassAddRequested,
}

pub struct EffectEditorView<'a> {
    instance: &'a EffectInstance,
    state: &'a EffectEditorState,
}

impl<'a> EffectEditorView<'a> {
    pub fn new(instance: &'a EffectInstance, state: &'a EffectEditorState) -> Self {
        Self { instance, state }
    }
}

impl<'a> From<EffectEditorView<'a>> for GraphElement<'a, EffectEditorMessage> {
    fn from(value: EffectEditorView<'a>) -> Self {
        let EffectEditorView { instance, state } = value;
        match state.open_pass {
            Some(pass_id) => view_pass_graph(instance, state, pass_id),
            None => view_pass_list(instance, state),
        }
    }
}

fn view_pass_list<'a>(
    instance: &'a EffectInstance,
    state: &'a EffectEditorState,
) -> GraphElement<'a, EffectEditorMessage> {
    let pass_count = instance.passes.len();
    let mut pass_list = Column::new().spacing(8);
    for (index, pass_id) in instance.passes.keys().enumerate() {
        pass_list = pass_list.push(pass_card(instance, state, *pass_id, index, pass_count));
    }
    pass_list = pass_list.push(
        Button::new(
            row![icon::plus().size(13), Label::new(t!("add_pass"))]
                .spacing(4)
                .align_y(Vertical::Center),
        )
        .outline()
        .on_press(EffectEditorMessage::PassAddRequested),
    );

    let passes_panel = Panel::new(
        column![
            Label::new(t!("passes")).strong(),
            Scrollable::new(pass_list).height(Length::Fill),
        ]
        .spacing(6),
    )
    .padding(8)
    .width(Length::Fill);

    let io_panel = Panel::new(
        component(EffectIoEditor::new(
            instance,
            &state.resources.type_registry,
        ))
        .map(EffectEditorMessage::Io),
    )
    .padding(8)
    .width(320);

    row![passes_panel, io_panel]
        .spacing(8)
        .height(Length::Fill)
        .padding(8)
        .into()
}

fn pass_card<'a>(
    instance: &'a EffectInstance,
    state: &'a EffectEditorState,
    pass_id: EffectPassId,
    index: usize,
    pass_count: usize,
) -> GraphElement<'a, EffectEditorMessage> {
    let pass = &instance.passes[&pass_id];
    let io = instance.pass_io_labels(&pass_id);
    let body = row![
        io_column(t!("inputs"), &io.inputs),
        io_column(t!("outputs"), &io.outputs),
    ]
    .spacing(8);

    let renaming = state.renaming_pass == Some(pass_id);
    let content: GraphElement<'a, EffectEditorMessage> = if renaming {
        row![
            TextInput::new("", &pass.name)
                .on_input(move |name| EffectEditorMessage::PassRenamed(pass_id, name))
                .width(Length::Fill),
            button::icon_button(icon::check().size(13))
                .on_press(EffectEditorMessage::PassRenameToggled(pass_id)),
        ]
        .spacing(4)
        .width(Length::Fill)
        .into()
    } else {
        Button::new(column![Label::new(pass.name.clone()).strong(), body].spacing(4))
            .width(Length::Fill)
            .height(Length::Shrink)
            .on_press(EffectEditorMessage::OpenPass(pass_id))
            .into()
    };

    let mut controls = Column::new().spacing(2).push(
        Button::new(icon::trash().size(13))
            .width(24)
            .height(24)
            .padding(5)
            .danger()
            .on_press(EffectEditorMessage::PassRemoveRequested(pass_id)),
    );
    if index > 0 {
        controls = controls.push(
            button::icon_button(icon::chevron_up().size(13))
                .on_press(EffectEditorMessage::PassMoveRequested { index, up: true }),
        );
    }
    if index + 1 < pass_count {
        controls = controls.push(
            button::icon_button(icon::chevron_down().size(13))
                .on_press(EffectEditorMessage::PassMoveRequested { index, up: false }),
        );
    }
    controls = controls.push(
        button::icon_button(icon::pencil().size(13))
            .on_press(EffectEditorMessage::PassRenameToggled(pass_id)),
    );

    row![content, controls].spacing(4).into()
}

fn io_column<'a>(title: String, labels: &[String]) -> GraphElement<'a, EffectEditorMessage> {
    let rows = labels
        .iter()
        .map(|label| Label::new(format!("• {label}")).faint().into())
        .collect::<Vec<_>>();
    column![Label::new(title).muted(), Column::with_children(rows)]
        .spacing(2)
        .width(Length::Fill)
        .into()
}

fn view_pass_graph<'a>(
    instance: &'a EffectInstance,
    state: &'a EffectEditorState,
    pass_id: EffectPassId,
) -> GraphElement<'a, EffectEditorMessage> {
    let pass = &instance.passes[&pass_id];
    let graph_state = state
        .graph_editor_states
        .get(&pass_id)
        .expect("graph editor state is created when a pass is opened");
    let back = Button::new(
        row![
            icon::chevron_left().size(13),
            Label::new(t!("back_to_passes"))
        ]
        .spacing(4)
        .align_y(Vertical::Center),
    )
    .transparent()
    .on_press(EffectEditorMessage::BackToPassList);

    column![
        row![back, Label::new(pass.name.clone()).strong()]
            .spacing(8)
            .align_y(Vertical::Center),
        GraphElement::from(GraphEditor::new(&pass.graph, graph_state))
            .map(EffectEditorMessage::Graph),
    ]
    .spacing(4)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn resync(instance: &mut EffectInstance) {
    sanitize_bindings(instance);
    instance
        .sync_pass_graph_effect_properties()
        .expect("sanitized instances always sync");
}

// Graph edits can delete port nodes, io edits can delete ports; drop bindings
// that reference anything missing before sync recomputes the cached types.
fn sanitize_bindings(instance: &mut EffectInstance) {
    let effect_inputs = instance.inputs.keys().copied().collect::<HashSet<_>>();
    let effect_outputs = instance.outputs.keys().copied().collect::<HashSet<_>>();
    let pass_outputs = instance
        .passes
        .values()
        .flat_map(|pass| pass.graph.iter_nodes())
        .filter_map(|node| node.data.state::<PassOutputNode>())
        .map(|state| state.id)
        .collect::<HashSet<EffectPassOutputSlotId>>();

    for pass in instance.passes.values_mut() {
        for node in pass.graph.iter_nodes_mut() {
            if let Some(state) = node.data.state_mut::<PassInputNode>()
                && let Some(input) = state.input
                && match input {
                    PassInput::Effect(id) => !effect_inputs.contains(&id),
                    PassInput::Pass(id) => !pass_outputs.contains(&id),
                }
            {
                state.input = None;
            }
            if let Some(state) = node.data.state_mut::<PassOutputNode>()
                && let Some(PassOutput::Effect(id)) = &state.output
                && !effect_outputs.contains(id)
            {
                state.output = None;
            }
        }
    }
}
