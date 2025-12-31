use std::sync::Arc;

use cyancia_canvas::CCanvas;

pub struct DestructedShell {
    pub current_canvas: Arc<CCanvas>,
    pub canvases: Vec<Arc<CCanvas>>,
}

pub struct CShell {
    current_canvas: Arc<CCanvas>,
    canvas_creation: Vec<Arc<CCanvas>>,
}

impl CShell {
    pub fn new(current_canvas: Arc<CCanvas>) -> Self {
        Self {
            current_canvas,
            canvas_creation: Vec::new(),
        }
    }

    pub fn canvas(&self) -> &CCanvas {
        &self.current_canvas
    }

    // pub fn all_canvases(&self) -> &[Arc<CCanvas>] {
    //     &self.all_canvases
    // }

    pub fn request_canvas_creation(&mut self, canvas: Arc<CCanvas>) {
        // if self.all_canvases.iter().any(|c| Arc::ptr_eq(c, &canvas)) {
        //     self.current_canvas = canvas;
        // } else {
        //     self.canvas_creation.push(canvas);
        // }
        self.canvas_creation.push(canvas);
    }

    pub fn destruct(self) -> DestructedShell {
        DestructedShell {
            current_canvas: self.current_canvas,
            canvases: self.canvas_creation,
        }
    }
}
