use cyancia_actions::ActionFunction;
use cyancia_canvas::CCanvas;

pub mod brush;
pub mod canvas_pan;
pub mod canvas_rotate;
pub mod canvas_zoom;

pub trait CanvasTool: ActionFunction {}
