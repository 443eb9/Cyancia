use crate::core::window::{self, Id};

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, WindowEvent};

/// A rectangle in the physical coordinate space of the native window.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalBounds {
    pub position: PhysicalPosition<f64>,
    pub size: (f64, f64),
}

/// Tracks the pointer state and the interactive window sessions.
pub struct Router {
    /// The logical window that last received a pointer press, used to tag
    /// keyboard events.
    focused: Option<Id>,
    /// The last known pointer position, in the physical coordinate space of
    /// the native window.
    cursor: Option<PhysicalPosition<f64>>,
    /// An interactive window move session started by `window::drag`.
    drag: Option<Drag>,
    /// An interactive window resize session started by
    /// `window::drag_resize` or a press on a window border.
    drag_resize: Option<DragResize>,
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    offset: PhysicalPosition<f64>,
    size: (f64, f64),
}

/// An interactive resize session of a logical window.
#[derive(Debug, Clone, Copy)]
struct DragResize {
    id: Id,
    direction: window::Direction,
    start_bounds: PhysicalBounds,
    start_cursor: PhysicalPosition<f64>,
}

/// The result of routing a native pointer event.
pub enum Routed {
    /// The event should be delivered to the user interface as-is.
    Deliver(WindowEvent),
    /// A `window::drag` session moved a window. The event that ended the
    /// session, if any, still needs to be delivered to keep the widget
    /// state consistent.
    Moved {
        position: PhysicalPosition<f64>,
        release: Option<WindowEvent>,
    },
    /// A `window::drag_resize` session—or a press on the resize border of
    /// a resizable window—resized a window.
    Resized {
        id: Id,
        bounds: PhysicalBounds,
        release: Option<WindowEvent>,
    },
    /// The event was consumed by an interactive session.
    Consumed,
}

impl Router {
    pub fn new() -> Self {
        Self {
            focused: None,
            cursor: None,
            drag: None,
            drag_resize: None,
        }
    }

    pub fn focused(&self) -> Option<Id> {
        self.focused
    }

    pub fn cursor(&self) -> Option<PhysicalPosition<f64>> {
        self.cursor
    }

    /// Focuses a logical window.
    pub fn focus(&mut self, id: Id) {
        self.focused = Some(id);
    }

    /// Forgets a window that is about to be removed.
    pub fn forget(&mut self, id: Id) {
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    /// Starts an interactive move session for the given window, like
    /// `Window::drag_window` does on the desktop platforms.
    ///
    /// The session is driven by the pointer and ends when a button is
    /// released.
    pub fn start_drag(&mut self, bounds: PhysicalBounds) -> bool {
        let Some(cursor) = self.cursor else {
            return false;
        };

        self.drag = Some(Drag {
            offset: PhysicalPosition::new(
                cursor.x - bounds.position.x,
                cursor.y - bounds.position.y,
            ),
            size: bounds.size,
        });

        true
    }

    /// Starts an interactive resize session, like
    /// `Window::drag_resize_window` does on the desktop platforms.
    pub fn start_drag_resize(
        &mut self,
        id: Id,
        direction: window::Direction,
        bounds: PhysicalBounds,
    ) -> bool {
        let Some(cursor) = self.cursor else {
            return false;
        };

        self.drag_resize = Some(DragResize {
            id,
            direction,
            start_bounds: bounds,
            start_cursor: cursor,
        });

        true
    }

    /// Routes a native pointer event.
    pub fn pointer_event(
        &mut self,
        event: WindowEvent,
        clamp_position: impl FnOnce(PhysicalBounds) -> PhysicalPosition<f64>,
    ) -> Routed {
        let position = pointer_position(&event);

        if let Some(position) = position {
            self.cursor = Some(position);
        }

        // Interactive sessions take priority: they consume the pointer
        // events that drive them and end on release.
        if self.drag.is_some() || self.drag_resize.is_some() {
            let release = matches!(
                &event,
                WindowEvent::PointerButton {
                    state: ElementState::Released,
                    ..
                }
            )
            .then(|| event.clone());

            if let Some(drag) = self.drag {
                if let Some(cursor) = position {
                    if release.is_some() {
                        self.drag = None;
                    }

                    let target =
                        PhysicalPosition::new(cursor.x - drag.offset.x, cursor.y - drag.offset.y);

                    return Routed::Moved {
                        position: clamp_position(PhysicalBounds {
                            position: target,
                            size: drag.size,
                        }),
                        release,
                    };
                }
            } else if let Some(drag_resize) = self.drag_resize {
                if let Some(cursor) = position {
                    if release.is_some() {
                        self.drag_resize = None;
                    }

                    return Routed::Resized {
                        id: drag_resize.id,
                        bounds: resized_bounds(&drag_resize, cursor),
                        release,
                    };
                }
            }

            // Events without a position cannot drive the sessions.
            return Routed::Consumed;
        }

        Routed::Deliver(event)
    }
}

/// Returns the position carried by a pointer event, if any.
fn pointer_position(event: &WindowEvent) -> Option<PhysicalPosition<f64>> {
    match event {
        WindowEvent::PointerMoved { position, .. }
        | WindowEvent::PointerEntered { position, .. }
        | WindowEvent::PointerButton { position, .. } => Some(*position),
        _ => None,
    }
}

/// Computes the bounds produced by an interactive resize session.
fn resized_bounds(drag_resize: &DragResize, cursor: PhysicalPosition<f64>) -> PhysicalBounds {
    let dx = cursor.x - drag_resize.start_cursor.x;
    let dy = cursor.y - drag_resize.start_cursor.y;

    let mut position = drag_resize.start_bounds.position;
    let (mut width, mut height) = drag_resize.start_bounds.size;

    match drag_resize.direction {
        window::Direction::North => {
            position.y += dy;
            height -= dy;
        }
        window::Direction::South => height += dy,
        window::Direction::East => width += dx,
        window::Direction::West => {
            position.x += dx;
            width -= dx;
        }
        window::Direction::NorthEast => {
            position.y += dy;
            height -= dy;
            width += dx;
        }
        window::Direction::NorthWest => {
            position.x += dx;
            width -= dx;
            position.y += dy;
            height -= dy;
        }
        window::Direction::SouthEast => {
            width += dx;
            height += dy;
        }
        window::Direction::SouthWest => {
            position.x += dx;
            width -= dx;
            height += dy;
        }
    }

    PhysicalBounds {
        position,
        size: (width.max(1.0), height.max(1.0)),
    }
}
