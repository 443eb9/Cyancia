use std::sync::Arc;

use cyancia_assets::store::AssetRegistry;
use cyancia_input::action::{ActionCollection, ActionManifest, matching::ActionMatcher};
use iced_core::{
    Clipboard, Element, Event, Layout, Length, Rectangle, Shell, Size, Widget,
    keyboard::{self, key},
    layout::{self, Limits},
    mouse, renderer,
    widget::{Tree, tree},
};

pub struct CanvasWidget {}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for CanvasWidget
where
    Renderer: iced_core::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
    }
}

impl<Message, Theme, Renderer> From<CanvasWidget> for Element<'_, Message, Theme, Renderer>
where
    Renderer: iced_core::Renderer,
{
    fn from(canvas: CanvasWidget) -> Self {
        Element::new(canvas)
    }
}
