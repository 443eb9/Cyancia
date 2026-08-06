use bevy_math::Rect;
use cyancia_color::shader::IccTransformShader;
use cyancia_image::{texel::TexelType, tile::GpuTileStorage};
use glam::Vec2;
use iced_core::{
    Clipboard, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Widget,
    layout::{self, Limits},
    mouse, renderer,
    widget::Tree,
    window,
};
use iced_wgpu::primitive::Renderer;
use iced_widget::{renderer::wgpu::primitive, shader::Program};
use moxcms::ColorProfile;

use crate::{
    CCanvas,
    render::{CanvasPrimitive, ICC_TRANSFORM_SHADER_IDENT},
};

pub struct CanvasWidget<'a, Message> {
    pub is_focusing: bool,
    pub canvas: &'a CCanvas,
    pub tile_storage: GpuTileStorage,
    pub on_focus: Box<dyn Fn(Point) -> Message + 'a>,
    pub on_mouse_event: Box<dyn Fn(mouse::Event) -> Message + 'a>,
    pub on_widget_rect_change: Box<dyn Fn(Rect) -> Message + 'a>,
    pub color_profile: ColorProfile,
    pub window_id: u64,
    pub monitor_name: String,
}

impl<'a, Message> CanvasWidget<'a, Message> {
    pub fn unmanaged(
        canvas: &'a CCanvas,
        tile_storage: GpuTileStorage,
        on_focus: impl Fn(Point) -> Message + 'a,
        on_mouse_event: impl Fn(mouse::Event) -> Message + 'a,
        on_widget_rect_change: impl Fn(Rect) -> Message + 'a,
        color_profile: ColorProfile,
        window_id: u64,
        monitor_name: String,
    ) -> Self {
        Self {
            is_focusing: false,
            canvas,
            tile_storage,
            on_focus: Box::new(on_focus),
            on_mouse_event: Box::new(on_mouse_event),
            on_widget_rect_change: Box::new(on_widget_rect_change),
            color_profile,
            window_id,
            monitor_name,
        }
    }

    pub fn focusing(mut self, is_focusing: bool) -> Self {
        self.is_focusing = is_focusing;
        self
    }
}

impl<Message, Theme> Widget<Message, Theme, iced_wgpu::Renderer> for CanvasWidget<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _: &mut Tree, _: &iced_wgpu::Renderer, limits: &Limits) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn update(
        &mut self,
        _: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &iced_wgpu::Renderer,
        _: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let widget_rect = Rect {
            min: Vec2::new(bounds.x, bounds.y),
            max: Vec2::new(bounds.x + bounds.width, bounds.y + bounds.height),
        };
        if widget_rect != self.canvas.transform.widget_bounds {
            shell.publish((self.on_widget_rect_change)(widget_rect));
        }

        if let Event::Mouse(event) = event {
            if self.is_focusing {
                shell.publish((self.on_mouse_event)(event.clone()));
                shell.capture_event();
            } else if let mouse::Event::ButtonPressed(mouse::Button::Left) = event
                && let Some(cursor_pos) = cursor.position_over(bounds)
            {
                shell.publish((self.on_focus)(cursor_pos));
                shell.publish((self.on_mouse_event)(event.clone()));
                shell.capture_event();
            }
        }
    }

    fn draw(
        &self,
        _: &Tree,
        renderer: &mut iced_wgpu::Renderer,
        _: &Theme,
        _: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        _: &Rectangle,
    ) {
        renderer.draw_primitive(
            layout.bounds(),
            CanvasPrimitive {
                image_size: self.canvas.image.size(),
                root_layer: *self.canvas.image.layer_stack().root_id(),
                selection_layer: self.canvas.image.selection_layer(),
                root_texel_type: self.canvas.image.texel_type(),
                selection_texel_type: TexelType::A8,
                transform: self.canvas.transform.clone(),
                tile_storage: self.tile_storage.clone(),
                color_profile: self.color_profile.clone(),
                window_id: self.window_id,
                monitor_name: self.monitor_name.clone(),
            },
        );
    }
}

impl<'a, Message, Theme> From<CanvasWidget<'a, Message>>
    for Element<'a, Message, Theme, iced_wgpu::Renderer>
where
    Message: 'a,
{
    fn from(canvas: CanvasWidget<'a, Message>) -> Self {
        Element::new(canvas)
    }
}
