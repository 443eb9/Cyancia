//! Regression test for internal (non-bubbling) io editor events coming from
//! overlays (the add-form type combo). The event loop below mirrors iced's
//! AboutToWait: the user interface persists across events and is only rebuilt
//! from a fresh view when messages arrived or the interface reports itself
//! outdated. Selecting a type must refresh the view (outdated) without
//! publishing any application message.

use std::sync::{Arc, LazyLock};

use futures::executor::block_on;
use iced_core::{
    Element, Event, Point, Size,
    keyboard,
    pointer::{self, button, mouse},
    renderer::Headless,
    shell::{Bus, Waker},
    window,
};
use iced_runtime::user_interface::{self, Cache, UserInterface};
use indexmap::IndexMap;
use lapiz_effect::{
    asset::{EffectPassDispatchStrategy, EffectPassId},
    editor::{EffectEditorMessage, EffectEditorState, EffectEditorView},
    instance::{EffectInstance, EffectPass},
    nodes::effect_nodes,
};
use lapiz_runtime::Renderer;
use lapiz_shader_graph::{
    GraphTheme,
    graph::{
        Graph, GraphResources,
        function::ASSET_GRAPH_FUNCTION_STORAGE, node::GraphNodeRegistry,
        variable::GraphTypeRegistry,
    },
    wgsl_std::builtin_types,
};
use uuid::Uuid;

static TYPE_REGISTRY: LazyLock<Arc<GraphTypeRegistry>> =
    LazyLock::new(|| Arc::new(builtin_types()));

static NODE_REGISTRY: LazyLock<Arc<GraphNodeRegistry>> =
    LazyLock::new(|| Arc::new(effect_nodes()));

fn graph_resources() -> GraphResources {
    GraphResources {
        type_registry: TYPE_REGISTRY.clone(),
        node_registry: NODE_REGISTRY.clone(),
        functions: ASSET_GRAPH_FUNCTION_STORAGE.clone(),
    }
}

// Empty io: the panel then contains nothing but the two add forms, so any
// combo found while scanning is an add-form combo and cannot publish row
// edits.
fn empty_io_instance() -> EffectInstance {
    EffectInstance {
        name: "Smoke".into(),
        passes: IndexMap::from([(
            EffectPassId::new(Uuid::new_v4()),
            EffectPass {
                name: "Only".into(),
                graph: Graph::new(graph_resources()),
                dispatch_strategy: EffectPassDispatchStrategy::Once,
            },
        )]),
        inputs: IndexMap::new(),
        outputs: IndexMap::new(),
    }
}

struct Loop<'a> {
    instance: &'a EffectInstance,
    state: &'a EffectEditorState,
    bounds: Size,
    renderer: Renderer,
    ui: Option<UserInterface<'a, EffectEditorMessage, GraphTheme, Renderer>>,
    cache: Cache,
}

impl<'a> Loop<'a> {
    fn new(instance: &'a EffectInstance, state: &'a EffectEditorState, renderer: Renderer) -> Self {
        Self {
            instance,
            state,
            bounds: Size::new(1280.0, 800.0),
            renderer,
            ui: None,
            cache: Cache::new(),
        }
    }

    fn step(
        &mut self,
        events: &[Event],
        cursor: Point,
        messages: &mut Bus<EffectEditorMessage>,
    ) -> (bool, bool) {
        if self.ui.is_none() {
            let element: Element<'_, EffectEditorMessage, GraphTheme, Renderer> =
                EffectEditorView::new(self.instance, self.state).into();
            self.ui = Some(UserInterface::build(
                element,
                self.bounds,
                std::mem::take(&mut self.cache),
                &mut self.renderer,
            ));
        }
        let (ui_state, _) = self
            .ui
            .as_mut()
            .unwrap()
            .update(
                &window::Headless,
                &Waker::noop(),
                events,
                pointer::mouse::Cursor::Available(cursor),
                &mut self.renderer,
                messages,
            );
        let rebuild = !messages.is_empty() || matches!(ui_state, user_interface::State::Outdated);
        if rebuild {
            self.cache = self.ui.take().unwrap().into_cache();
        }
        (rebuild, ui_state.has_layout_changed())
    }

    fn click(&mut self, at: Point, messages: &mut Bus<EffectEditorMessage>) -> (bool, bool) {
        let events = [
            Event::Pointer(pointer::Event::PointerMoved {
                position: at,
                source: pointer::Source::Mouse,
            }),
            Event::Pointer(pointer::Event::PointerPressed {
                position: at,
                button: button::Source::Mouse(mouse::Button::Left),
            }),
            Event::Pointer(pointer::Event::PointerReleased {
                position: at,
                button: button::Source::Mouse(mouse::Button::Left),
            }),
        ];
        // The selection may already fire on the press; a later step of the
        // same click then runs against the freshly rebuilt interface, so the
        // rebuild decision has to be accumulated across all steps.
        let mut rebuilt = false;
        let mut layout_changed = false;
        for event in events {
            let (step_rebuilt, step_layout_changed) = self.step(&[event], at, messages);
            rebuilt |= step_rebuilt;
            layout_changed |= step_layout_changed;
        }
        (rebuilt, layout_changed)
    }
}

#[test]
fn overlay_internal_events_refresh_without_spilling() {
    let Some(renderer) = block_on(<Renderer as Headless>::new(Default::default(), None)) else {
        eprintln!("skipping: no headless renderer");
        return;
    };

    let instance = empty_io_instance();
    let state = EffectEditorState::new(graph_resources());
    let mut messages = Bus::new();
    let mut app = Loop::new(&instance, &state, renderer);

    // Coordinates match the fixed 1280x800 layout of the empty-io demo;
    // verified via screenshots.
    let type_combo = Point::new(1131.0, 384.0);
    let name_field = Point::new(1016.0, 384.0);
    let add_button = Point::new(1227.0, 384.0);

    // Open the add form's type combo and pick the first menu item. The menu
    // is an overlay and the selection is an internal component event: it must
    // refresh the interface (rebuild) without publishing any message.
    app.click(type_combo, &mut messages);
    let (rebuilt, _) = app.click(Point::new(type_combo.x, type_combo.y + 30.0), &mut messages);
    assert!(
        messages.is_empty(),
        "the type selection is internal and must not spill to the application"
    );
    assert!(
        rebuilt,
        "an overlay-driven internal event must invalidate the user interface"
    );

    // Name the new input: still internal, still no application messages.
    app.click(name_field, &mut messages);
    app.step(
        &[Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Character("E".to_string().into()),
            modified_key: keyboard::Key::Character("E".to_string().into()),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyE),
            location: keyboard::Location::Standard,
            modifiers: Default::default(),
            text: Some("E".into()),
            repeat: false,
        })],
        name_field,
        &mut messages,
    );
    assert!(
        messages.is_empty(),
        "buffer edits are internal and must not spill to the application"
    );

    // The add button now carries both buffers and bubbles a single message.
    app.click(add_button, &mut messages);
    let EffectEditorMessage::Io(lapiz_effect::editor::EffectIoEditorMessage::AddInput {
        name,
        ty,
    }) = messages
        .drain()
        .map(|(message, _)| message)
        .find(|message| {
            matches!(
                message,
                EffectEditorMessage::Io(lapiz_effect::editor::EffectIoEditorMessage::AddInput {
                    ..
                })
            )
        })
        .expect("add input message")
    else {
        unreachable!()
    };
    assert_eq!(name, "E");
    assert!(!ty.ty.id().id.is_empty());
    eprintln!("added input '{name}' of type {}", ty.ty.id().id);
}
