//! A widget that lays out and draws the logical windows of the application.

use crate::core::layout;
use crate::core::overlay;
use crate::core::pointer::mouse;
use crate::core::renderer;
use crate::core::widget::{self, Widget};
use crate::core::window::Id;
use crate::core::{Color, Element, Point, Rectangle, Size, Vector};

/// The content of a logical window: a view drawn at a fixed position and
/// size inside the native window.
pub struct Window<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    pub id: Id,
    pub position: Point,
    pub size: Size,
    pub background: Color,
    pub content: Element<'a, Message, Theme, Renderer>,
}

/// Lays out its [`Window`] children at their absolute positions, from
/// bottom to top.
///
/// This is the root widget of the Android runtime: every logical window of
/// the application is a child of this container, and the whole tree is a
/// single [`UserInterface`](crate::runtime::user_interface::UserInterface).
pub struct Windows<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    windows: Vec<Window<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Windows<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Creates an empty [`Windows`] container.
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
        }
    }

    /// Adds a window on top of the existing ones.
    pub fn push(&mut self, window: Window<'a, Message, Theme, Renderer>) -> &mut Self {
        self.windows.push(window);

        self
    }

    fn bounds(&self, window: &Window<'a, Message, Theme, Renderer>) -> Rectangle {
        Rectangle::new(window.position, window.size)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Windows<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn size(&self) -> Size<crate::core::Length> {
        use crate::core::Length;

        Size::new(Length::Fill, Length::Fill)
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        tree.diff_children_custom(
            &mut self.windows,
            |tree, window| tree.diff(&mut window.content),
            |window| widget::Tree::new(&window.content),
        );
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let bounds = limits.max();

        let children: Vec<layout::Node> = self
            .windows
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(window, state)| {
                let limits = layout::Limits::new(window.size, window.size);

                window
                    .content
                    .as_widget_mut()
                    .layout(state, renderer, &limits)
                    .move_to(window.position)
            })
            .collect();

        layout::Node::with_children(bounds, children)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &crate::core::Event,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut crate::core::Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for ((window, state), layout) in self
            .windows
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            window
                .content
                .as_widget_mut()
                .update(state, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.windows
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((window, state), layout)| {
                window
                    .content
                    .as_widget()
                    .mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for ((window, state), layout) in self
            .windows
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .filter(|(_, layout)| layout.bounds().intersects(viewport))
        {
            let bounds = self.bounds(window);

            renderer.with_layer(bounds, |renderer| {
                // Logical windows are composited on top of each other, so
                // each one paints its own background.
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..Default::default()
                    },
                    window.background,
                );

                window
                    .content
                    .as_widget()
                    .draw(state, renderer, theme, style, layout, cursor, viewport);
            });
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: layout::Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let children = self
            .windows
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .filter_map(|((window, state), layout)| {
                window.content.as_widget_mut().overlay(
                    state,
                    layout,
                    renderer,
                    viewport,
                    translation,
                )
            })
            .collect::<Vec<_>>();

        (!children.is_empty()).then(|| overlay::Group::with_children(children).overlay())
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: layout::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        for ((window, state), layout) in self
            .windows
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            window
                .content
                .as_widget_mut()
                .operate(state, layout, renderer, operation);
        }
    }
}

impl<Message, Theme, Renderer> Default for Windows<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer> From<Windows<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(windows: Windows<'a, Message, Theme, Renderer>) -> Self {
        Element::new(windows)
    }
}
