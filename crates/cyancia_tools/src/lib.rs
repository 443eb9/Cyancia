use cyancia_canvas::{CCanvas, action::CanvasAction};

pub mod brush;
pub mod canvas_pan;
pub mod canvas_rotate;
pub mod canvas_zoom;

pub trait CanvasTool: CanvasAction {}
