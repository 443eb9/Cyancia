use lapiz_runtime::{global::Globals, renderer::RenderContext};
use wgpu::{Device, Queue};

pub trait RenderContextAppExt {
    fn render_context(&self) -> &RenderContext;
    fn render_device(&self) -> &Device;
    fn render_queue(&self) -> &Queue;
}

impl RenderContextAppExt for Globals {
    fn render_context(&self) -> &RenderContext {
        self.global::<RenderContext>()
    }

    fn render_device(&self) -> &Device {
        &self.render_context().device
    }

    fn render_queue(&self) -> &Queue {
        &self.render_context().queue
    }
}
