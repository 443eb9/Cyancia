use std::sync::Arc;

use anyhow::Result;
use iced_core::{Element, Length, Size, Theme, alignment::Vertical, window};
use iced_futures::Subscription;
use iced_runtime::Task;
use iced_widget::{Column, column, component::component, row};
use lapiz_assets::{AssetAppExt, asset::AssetHandle};
use lapiz_effect::{
    asset::EffectInputSlotId,
    editor::{EffectEditorMessage, EffectEditorState, EffectEditorView, EffectIoEditor},
    instance::{EffectInputSlot, EffectInstance, EffectOutputSlot},
};
use lapiz_i18n::t;
use lapiz_image::texel::TexelType;
use lapiz_runtime::{
    Services,
    windows::{WindowView, WindowViewId},
};
use lapiz_shader_graph::{
    GraphElement,
    graph::slot::{ErasedGraphLiteralUpdateMessage, ErasedGraphValueType, GraphInputSlotId},
    wgsl_std::types::LayerType,
};
use lapiz_widgets::{
    button::Button, label::Label, panel::Panel, scrollable::Scrollable, text_input::TextInput,
};
use uuid::Uuid;

use crate::{
    asset::{FilterPreset, FilterPresetMetadata},
    instance::FilterInstance,
    render::graph::filter_graph_resources,
};

pub struct FilterEditor {
    windows: Arc<[window::Id]>,
    main_window: window::Id,
    filters: Vec<AssetHandle<FilterPreset>>,
    selected_index: Option<usize>,
    selected: Option<SelectedFilter>,
    effect_editor_state: EffectEditorState,
    filter_name_buffer: String,
    dirty: bool,
    validation_error: Option<String>,
}

pub struct SelectedFilter {
    pub handle: AssetHandle<FilterPreset>,
    pub instance: FilterInstance,
}

#[derive(Clone)]
pub enum FilterEditorMessage {
    SelectFilter(usize),
    NewFilter,
    FilterNameChanged(String),
    Save,
    Effect(EffectEditorMessage),
    UpdateParameter(EffectInputSlotId, ErasedGraphLiteralUpdateMessage),
}

impl WindowView for FilterEditor {
    type Message = FilterEditorMessage;

    type BootParams = ();

    fn id() -> WindowViewId {
        WindowViewId::new("filter_editor")
    }

    fn boot(
        _params: Option<Self::BootParams>,
        services: &mut Services,
    ) -> Result<(Self, Task<Self::Message>)> {
        let filters = services
            .assets()
            .all_handles_of::<FilterPreset>()
            .expect("Failed to list filter presets");
        let (main_window, open) = iced_runtime::window::open(window::Settings {
            size: Size {
                width: 1280.0,
                height: 800.0,
            },
            ..Default::default()
        });
        Ok((
            Self {
                windows: [main_window].into(),
                main_window,
                filters,
                selected_index: None,
                selected: None,
                effect_editor_state: EffectEditorState::new(filter_graph_resources()),
                filter_name_buffer: String::new(),
                dirty: false,
                validation_error: None,
            },
            open.discard(),
        ))
    }

    fn view<'a>(
        &'a self,
        _: window::Id,
        _: &'a Services,
    ) -> impl Into<Element<'a, Self::Message, Theme, lapiz_runtime::Renderer>> {
        let filter_list = self
            .filters
            .iter()
            .enumerate()
            .map(|(index, handle)| {
                let name = handle
                    .get()
                    .map(|preset| preset.metadata.name.clone())
                    .unwrap_or_else(|_| "<loading>".to_string());
                Button::new(Label::new(name))
                    .width(Length::Fill)
                    .activated(self.selected_index == Some(index))
                    .on_press(FilterEditorMessage::SelectFilter(index))
                    .into()
            })
            .collect::<Vec<_>>();

        // Region 1: pick or create a filter.
        let sidebar = Panel::new(
            column![
                Label::new(t!("filters")).strong(),
                Scrollable::new(Column::with_children(filter_list).spacing(2))
                    .width(Length::Fill)
                    .height(Length::Fill),
                Button::new(Label::new(t!("new_filter")))
                    .on_press(FilterEditorMessage::NewFilter),
            ]
            .spacing(6),
        )
        .padding(8)
        .width(220);

        let empty = || -> EditorElement<'_> { Label::new("").into() };
        let (naming, effect_editor, io_panel, parameters) = match self.selected.as_ref() {
            Some(selected) => self.view_selected(selected),
            None => (
                Label::new(t!("select_a_filter_to_adjust")).muted().into(),
                empty(),
                empty(),
                empty(),
            ),
        };

        // Region 2: naming on top, effect editor filling the rest.
        let center = column![naming, effect_editor].spacing(6).height(Length::Fill);

        row![sidebar, center, io_panel, parameters]
            .spacing(8)
            .height(Length::Fill)
            .padding(8)
    }

    fn update(
        &mut self,
        message: Self::Message,
        services: &mut Services,
    ) -> impl Into<Task<Self::Message>> {
        match message {
            FilterEditorMessage::SelectFilter(index) => self.select_filter(index, services),
            FilterEditorMessage::NewFilter => self.new_filter(services),
            FilterEditorMessage::FilterNameChanged(name) => {
                self.filter_name_buffer = name.clone();
                if let Some(selected) = self.selected.as_mut() {
                    selected.instance.metadata_mut().name = name;
                    self.dirty = true;
                }
                Task::none()
            }
            FilterEditorMessage::Save => self.save(services),
            FilterEditorMessage::Effect(message) => {
                if let Some(selected) = self.selected.as_mut() {
                    self.effect_editor_state
                        .update(selected.instance.effect_mut(), message);
                    // Io edits change the parameter set; realign values.
                    selected.instance.resync_parameters();
                    self.dirty = true;
                    self.revalidate();
                }
                Task::none()
            }
            FilterEditorMessage::UpdateParameter(id, message) => {
                if let Some(selected) = self.selected.as_mut() {
                    selected.instance.update_parameter(&id, message);
                    self.dirty = true;
                }
                Task::none()
            }
        }
    }

    fn subscription(&self, _services: &Services) -> Subscription<Self::Message> {
        Subscription::none()
    }

    fn close(self, _: &mut Services) -> Task<()> {
        iced_runtime::window::close(self.main_window)
    }

    fn windows(&self) -> Arc<[window::Id]> {
        self.windows.clone()
    }

    fn root_window(&self) -> Option<window::Id> {
        Some(self.main_window)
    }
}

type EditorElement<'a> = GraphElement<'a, FilterEditorMessage>;

impl FilterEditor {
    fn view_selected<'a>(
        &'a self,
        selected: &'a SelectedFilter,
    ) -> (EditorElement<'a>, EditorElement<'a>, EditorElement<'a>, EditorElement<'a>) {
        let status = if self.dirty {
            Label::new("*").muted()
        } else {
            Label::new("")
        };
        let naming_row = row![
            Label::new(t!("name")),
            TextInput::new("", &self.filter_name_buffer)
                .on_input(FilterEditorMessage::FilterNameChanged)
                .width(Length::Fill),
            Button::new(Label::new(t!("save")))
                .primary()
                .on_press_maybe(
                    (self.dirty && self.validation_error.is_none())
                        .then_some(FilterEditorMessage::Save),
                ),
            status,
        ]
        .spacing(6)
        .align_y(Vertical::Center)
        .height(Length::Shrink);

        let naming: EditorElement<'a> = match self.validation_error.as_ref() {
            Some(error) => {
                let error_text: EditorElement<'a> = iced_widget::Text::new(error.clone())
                    .color(iced_core::Color::from_rgb(1.0, 0.3, 0.3))
                    .into();
                column![naming_row, error_text].spacing(2).into()
            }
            None => naming_row.into(),
        };

        // Region 2 bottom: the effect editor (passes and their graphs).
        let effect_editor: EditorElement<'a> =
            GraphElement::from(EffectEditorView::new(
                selected.instance.effect(),
                &self.effect_editor_state,
            ))
            .map(FilterEditorMessage::Effect);

        // Region 3: effect io editor. Adding or retyping an input here also
        // grows the filter's parameter set.
        let io_panel = Panel::new(
            column![
                Label::new(t!("effect_io")).strong(),
                component(EffectIoEditor::new(
                    selected.instance.effect(),
                    &self.effect_editor_state.resources.type_registry,
                ))
                .map(|message| FilterEditorMessage::Effect(EffectEditorMessage::Io(message))),
            ]
            .spacing(6),
        )
        .padding(8)
        .width(320);

        // Region 4: filter parameter values. Names and types come from the
        // effect inputs managed in region 3.
        let parameter_rows = selected
            .instance
            .parameters()
            .iter()
            .map(|(id, parameter)| {
                row![
                    Label::new(parameter.name.clone()).width(Length::Fill),
                    parameter
                        .value
                        .ty()
                        .view_literal(GraphInputSlotId::new(id.0), parameter.value.value())
                        .map(move |message| FilterEditorMessage::UpdateParameter(*id, message)),
                ]
                .spacing(6)
                .into()
            })
            .collect::<Vec<_>>();
        let parameters_content = if parameter_rows.is_empty() {
            column![Label::new(t!("no_external_variables")).muted()].spacing(6)
        } else {
            column![Label::new(t!("parameters")).strong()]
                .spacing(6)
                .push(
                    Scrollable::new(Column::with_children(parameter_rows).spacing(6))
                        .height(Length::Fill),
                )
        };
        let parameters = Panel::new(parameters_content).padding(8).width(260);

        (
            naming,
            effect_editor,
            io_panel.into(),
            parameters.into(),
        )
    }

    fn select_filter(&mut self, index: usize, _services: &Services) -> Task<FilterEditorMessage> {
        let Some(handle) = self.filters.get(index).cloned() else {
            return Task::none();
        };
        let instance = match FilterInstance::from_asset(&handle) {
            Ok(instance) => instance,
            Err(e) => {
                log::error!("Failed to load filter preset: {e}");
                return Task::none();
            }
        };
        self.selected_index = Some(index);
        self.filter_name_buffer = instance.metadata().name.clone();
        self.selected = Some(SelectedFilter { handle, instance });
        self.effect_editor_state = EffectEditorState::new(filter_graph_resources());
        self.dirty = false;
        self.validation_error = None;
        Task::none()
    }

    fn new_filter(&mut self, services: &mut Services) -> Task<FilterEditorMessage> {
        let layer_ty: Arc<dyn ErasedGraphValueType> = Arc::new(LayerType {
            texel_type: TexelType::RGBA8,
        });
        let target = EffectInputSlotId::new(Uuid::new_v4());
        let output = lapiz_effect::asset::EffectOutputSlotId::new(Uuid::new_v4());
        let effect = EffectInstance {
            name: "Filter".into(),
            passes: Default::default(),
            inputs: indexmap::IndexMap::from([(
                target,
                EffectInputSlot {
                    name: "Target".into(),
                    id: target,
                    ty: layer_ty.clone(),
                },
            )]),
            outputs: indexmap::IndexMap::from([(
                output,
                EffectOutputSlot {
                    name: "Layer".into(),
                    id: output,
                    ty: layer_ty,
                },
            )]),
        };
        let preset = FilterPreset {
            metadata: FilterPresetMetadata {
                name: "[Unnamed Filter]".into(),
            },
            effect: effect
                .as_asset()
                .expect("freshly built effects always serialize"),
            parameters: Default::default(),
        };
        let Some(bundle) = services
            .assets()
            .bundles()
            .find(|bundle| !bundle.is_readonly())
            .map(|bundle| bundle.metadata().bundle_id)
        else {
            log::error!("No writable asset bundle available for a new filter preset");
            return Task::none();
        };
        let path = format!("unnamed_filter_{}.lfp", Uuid::new_v4());
        let id = match services.assets().add_asset(bundle, path, Arc::new(preset)) {
            Ok(id) => id,
            Err(err) => {
                log::error!("Failed to add new filter preset asset: {err}");
                return Task::none();
            }
        };
        let Some(handle) = services.assets().handle(id).ok() else {
            log::error!("Failed to obtain handle for new filter preset");
            return Task::none();
        };
        let index = self.filters.len();
        self.filters.push(handle);
        let task = self.select_filter(index, services);
        self.dirty = true;
        task
    }

    fn save(&mut self, _services: &mut Services) -> Task<FilterEditorMessage> {
        let Some(selected) = self.selected.as_mut() else {
            return Task::none();
        };
        if self.validation_error.is_some() {
            return Task::none();
        }
        let preset = match selected.instance.as_asset() {
            Ok(preset) => preset,
            Err(err) => {
                self.validation_error = Some(format!("Failed to serialize filter: {err}"));
                return Task::none();
            }
        };
        if let Err(err) = selected.handle.update(preset) {
            self.validation_error = Some(format!("Failed to update filter preset: {err}"));
            return Task::none();
        }
        if let Err(err) = selected.handle.write() {
            self.validation_error = Some(format!("Failed to write filter preset: {err}"));
            return Task::none();
        }
        self.dirty = false;
        self.validation_error = None;
        Task::none()
    }

    fn revalidate(&mut self) {
        let Some(selected) = self.selected.as_ref() else {
            self.validation_error = None;
            return;
        };
        let effect = selected.instance.effect();
        let layer_inputs = effect
            .inputs
            .values()
            .filter(|slot| slot.ty.is::<LayerType>())
            .count();
        let layer_outputs = effect
            .outputs
            .values()
            .filter(|slot| slot.ty.is::<LayerType>())
            .count();
        self.validation_error = match (layer_inputs, layer_outputs) {
            (1, 1) => None,
            (inputs, outputs) => Some(format!(
                "A filter needs exactly one layer input and one layer output (currently {inputs} input(s), {outputs} output(s))."
            )),
        };
    }
}

