use std::{borrow::Cow, sync::Arc};

use iced_core::Length;
use iced_widget::{Column, Component, column, component::component, row};
use lapiz_assets::store::AssetRegistry;
use lapiz_i18n::t;
use lapiz_widgets::{
    button::Button, icon, label::Label, scrollable::Scrollable, text_input::TextInput,
};
use uuid::Uuid;

use crate::{
    GraphElement, GraphRenderer, GraphTheme,
    editor::types::GraphTypeSelector,
    graph::{
        slot::{ErasedGraphLiteralUpdateMessage, ErasedGraphValueType, GraphInputSlotId},
        variable::{GraphLiteral, GraphTypeRegistry},
    },
};

pub struct ParametersEditor<'a, Message> {
    parameters: Vec<(Cow<'a, str>, GraphLiteral)>,
    type_registry: &'a GraphTypeRegistry,
    assets: &'a AssetRegistry,
    on_change: Box<dyn Fn(ParametersEditorMessage) -> Message + 'a>,
}

impl<'a, Message> ParametersEditor<'a, Message> {
    pub fn new(
        parameters: impl IntoIterator<Item = (&'a str, &'a GraphLiteral)>,
        type_registry: &'a GraphTypeRegistry,
        assets: &'a AssetRegistry,
        on_change: impl Fn(ParametersEditorMessage) -> Message + 'a,
    ) -> Self {
        Self {
            parameters: parameters
                .into_iter()
                .map(|(name, value)| (Cow::Borrowed(name), value.clone()))
                .collect(),
            type_registry,
            assets,
            on_change: Box::new(on_change),
        }
    }
}

impl<'a, Message> Component<'a, Message, GraphTheme, GraphRenderer>
    for ParametersEditor<'a, Message>
{
    type State = ParametersEditorState;
    type Event = ParametersEditorEvent;

    fn update(
        &mut self,
        state: &mut Self::State,
        event: Self::Event,
        _renderer: &GraphRenderer,
    ) -> Option<Message> {
        let message = match event {
            ParametersEditorEvent::NewNameChanged(name) => {
                state.new_name = name;
                return None;
            }
            ParametersEditorEvent::NewTypeSelected(ty) => {
                state.new_type = ty;
                return None;
            }
            ParametersEditorEvent::AddPressed => {
                let ty = state.new_type.clone()?;
                let name = std::mem::take(&mut state.new_name);
                ParametersEditorMessage::Add {
                    name,
                    value: GraphLiteral::new_boxed_default(ty),
                }
            }
            ParametersEditorEvent::Apply(message) => message,
        };
        Some((self.on_change)(message))
    }

    fn view(&self, state: &Self::State) -> GraphElement<'a, Self::Event> {
        let parameter_rows = self
            .parameters
            .iter()
            .enumerate()
            .map(|(index, (name, value))| {
                let literal = value
                    .ty()
                    .view_literal(
                        GraphInputSlotId::new(Uuid::nil()),
                        value.value(),
                        self.assets,
                    )
                    .map(move |message| {
                        ParametersEditorEvent::Apply(ParametersEditorMessage::UpdateLiteral {
                            index,
                            message,
                        })
                    });
                column![
                    row![
                        TextInput::new(&t!("name"), name)
                            .on_input(move |name| {
                                ParametersEditorEvent::Apply(ParametersEditorMessage::Rename {
                                    index,
                                    name,
                                })
                            })
                            .width(Length::Fill),
                        Label::new(value.ty().id().id).size(10).faint(),
                        Button::new(icon::trash().size(13))
                            .width(24)
                            .height(24)
                            .padding(5)
                            .danger()
                            .on_press(ParametersEditorEvent::Apply(
                                ParametersEditorMessage::Remove { index },
                            )),
                    ]
                    .spacing(4),
                    literal,
                ]
                .spacing(4)
                .into()
            })
            .collect::<Vec<_>>();

        let parameters: GraphElement<'a, ParametersEditorEvent> = if parameter_rows.is_empty() {
            Label::new(t!("no_parameters")).muted().into()
        } else {
            Scrollable::new(Column::with_children(parameter_rows).spacing(8))
                .height(Length::Fill)
                .into()
        };

        let add_row = column![
            TextInput::new(&t!("name"), &state.new_name)
                .on_input(ParametersEditorEvent::NewNameChanged)
                .width(Length::Fill),
            component(GraphTypeSelector::new(
                self.type_registry,
                ParametersEditorEvent::NewTypeSelected,
            )),
            Button::new(row![icon::plus().size(13), Label::new(t!("add_parameter"))].spacing(4))
                .on_press_maybe(
                    (!state.new_name.trim().is_empty() && state.new_type.is_some())
                        .then_some(ParametersEditorEvent::AddPressed),
                ),
        ]
        .spacing(4);

        column![parameters, add_row].spacing(8).into()
    }
}

#[derive(Clone)]
pub enum ParametersEditorMessage {
    Add {
        name: String,
        value: GraphLiteral,
    },
    Remove {
        index: usize,
    },
    Rename {
        index: usize,
        name: String,
    },
    UpdateLiteral {
        index: usize,
        message: ErasedGraphLiteralUpdateMessage,
    },
}

#[derive(Default)]
pub struct ParametersEditorState {
    new_name: String,
    new_type: Option<Arc<dyn ErasedGraphValueType>>,
}

#[derive(Clone)]
pub enum ParametersEditorEvent {
    NewNameChanged(String),
    NewTypeSelected(Option<Arc<dyn ErasedGraphValueType>>),
    AddPressed,
    Apply(ParametersEditorMessage),
}
