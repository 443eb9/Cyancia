use cyancia_math::curve::CubicCurve;
use glam::Vec2;
use iced_core::{
    Background, Border, Clipboard, Color, Element, Event, Layout, Length, Point, Rectangle, Shell,
    Size, Theme, Widget,
    keyboard::{self, key},
    layout,
    mouse::{self, Cursor},
    renderer::{self, Quad},
    widget::{self, tree},
};
use iced_graphics::geometry::{Frame, Path, Stroke};

const MIN_POINT_GAP: f32 = 0.001;

pub struct CurveEdit<'a, Message> {
    curve: CubicCurve,
    on_change: Option<Box<dyn Fn(CubicCurve) -> Message + 'a>>,
    on_release: Option<Box<dyn Fn(CubicCurve) -> Message + 'a>>,
    width: Length,
    height: Length,
    style: CurveEditStyle,
}

impl<'a, Message> CurveEdit<'a, Message> {
    pub fn new(curve: CubicCurve) -> Self {
        assert!(curve.control_points().len() >= 2);

        Self {
            curve,
            on_change: None,
            on_release: None,
            width: Length::Fill,
            height: Length::Fill,
            style: CurveEditStyle::default(),
        }
    }

    pub fn on_change(mut self, callback: impl Fn(CubicCurve) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(callback));
        self
    }

    pub fn on_release(mut self, callback: impl Fn(CubicCurve) -> Message + 'a) -> Self {
        self.on_release = Some(Box::new(callback));
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn curve_style(mut self, style: CurveEditStyle) -> Self {
        self.style = style;
        self
    }

    pub fn grid_resolution(mut self, resolution: usize) -> Self {
        assert!(resolution > 0);
        self.style.grid_resolution = resolution;
        self
    }

    pub fn curve_resolution(mut self, resolution: usize) -> Self {
        assert!(resolution > 0);
        self.style.curve_resolution = resolution;
        self
    }

    pub fn control_point_radius(mut self, radius: f32) -> Self {
        assert!(radius > 0.0);
        self.style.control_point_radius = radius;
        self
    }

    pub fn curve_stroke_width(mut self, width: f32) -> Self {
        assert!(width > 0.0);
        self.style.curve_stroke_width = width;
        self
    }

    pub fn grid_stroke_width(mut self, width: f32) -> Self {
        assert!(width > 0.0);
        self.style.grid_stroke_width = width;
        self
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for CurveEdit<'_, Message>
where
    Renderer: iced_core::Renderer + iced_graphics::geometry::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CurveEditState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CurveEditState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<CurveEditState>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_in(bounds) else {
                    return;
                };

                let normalized = local_to_normalized(position, bounds.size());
                let closest = self
                    .curve
                    .control_points()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, point)| {
                        let screen = normalized_to_local(*point, bounds.size());
                        let distance = (screen - Vec2::new(position.x, position.y)).length();
                        (distance <= self.style.control_point_radius * 2.0)
                            .then_some((index, distance))
                    })
                    .min_by(|left, right| left.1.total_cmp(&right.1))
                    .map(|(index, _)| index);

                if let Some(index) = closest {
                    state.selected_index = Some(index);
                    state.dragging = true;
                    state.drag_curve = None;
                } else if let Some(on_change) = &self.on_change {
                    let mut points = self.curve.control_points().to_vec();
                    let index = points.partition_point(|point| point.x < normalized.x);
                    let min = index
                        .checked_sub(1)
                        .map_or(0.0, |previous| points[previous].x + MIN_POINT_GAP);
                    let max = points.get(index).map_or(1.0, |next| next.x - MIN_POINT_GAP);
                    assert!(min <= max);
                    points.insert(index, Vec2::new(normalized.x.clamp(min, max), normalized.y));
                    let curve = CubicCurve::new(points);
                    state.selected_index = Some(index);
                    state.dragging = true;
                    state.drag_curve = Some(curve.clone());
                    shell.publish(on_change(curve));
                }

                shell.request_redraw();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let Some(position) = cursor.position() else {
                    return;
                };
                let index = state
                    .selected_index
                    .expect("dragging without a selected point");
                let mut points = self.curve.control_points().to_vec();
                assert!(index < points.len());

                let normalized = screen_to_normalized(position, bounds);
                let min = index
                    .checked_sub(1)
                    .map_or(0.0, |previous| points[previous].x + MIN_POINT_GAP);
                let max = points
                    .get(index + 1)
                    .map_or(1.0, |next| next.x - MIN_POINT_GAP);
                assert!(min <= max);
                points[index] = Vec2::new(normalized.x.clamp(min, max), normalized.y);

                let curve = CubicCurve::new(points);
                state.drag_curve = Some(curve.clone());
                if let Some(on_change) = &self.on_change {
                    shell.publish(on_change(curve));
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                let curve = state
                    .drag_curve
                    .take()
                    .unwrap_or_else(|| self.curve.clone());
                if let Some(on_release) = &self.on_release {
                    shell.publish(on_release(curve));
                }
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(key::Named::Delete),
                ..
            }) if cursor.is_over(bounds) => {
                let Some(index) = state.selected_index else {
                    return;
                };
                if self.curve.control_points().len() <= 2 {
                    return;
                }

                let mut points = self.curve.control_points().to_vec();
                assert!(index < points.len());
                points.remove(index);
                state.selected_index = None;
                state.dragging = false;
                state.drag_curve = None;
                if let Some(on_change) = &self.on_change {
                    shell.publish(on_change(CubicCurve::new(points)));
                }
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _renderer_style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let palette = self.style.resolve(theme);
        let state = tree.state.downcast_ref::<CurveEditState>();

        renderer.fill_quad(
            Quad {
                bounds,
                border: Border {
                    color: palette.border,
                    width: self.style.border_width,
                    ..Default::default()
                },
                ..Default::default()
            },
            Background::Color(palette.background),
        );

        let mut frame = Frame::new(renderer, bounds.size());

        for index in 1..self.style.grid_resolution {
            let fraction = index as f32 / self.style.grid_resolution as f32;
            let x = fraction * bounds.width;
            let y = fraction * bounds.height;
            let vertical = Path::new(|builder| {
                builder.move_to(Point::new(x, 0.0));
                builder.line_to(Point::new(x, bounds.height));
            });
            let horizontal = Path::new(|builder| {
                builder.move_to(Point::new(0.0, y));
                builder.line_to(Point::new(bounds.width, y));
            });
            let stroke = Stroke {
                style: palette.grid.into(),
                width: self.style.grid_stroke_width,
                ..Default::default()
            };
            frame.stroke(&vertical, stroke);
            frame.stroke(&horizontal, stroke);
        }

        let sampled = self.curve.subdivide(self.style.curve_resolution);
        let curve = Path::new(|builder| {
            let first = normalized_to_local(sampled[0], bounds.size());
            builder.move_to(Point::new(first.x, first.y));
            for point in &sampled[1..] {
                let point = normalized_to_local(*point, bounds.size());
                builder.line_to(Point::new(point.x, point.y));
            }
        });
        frame.stroke(
            &curve,
            Stroke {
                style: palette.curve.into(),
                width: self.style.curve_stroke_width,
                ..Default::default()
            },
        );

        for (index, point) in self.curve.control_points().iter().enumerate() {
            let center = normalized_to_local(*point, bounds.size());
            let radius = self.style.control_point_radius;
            let origin = Point::new(center.x - radius, center.y - radius);
            let point_size = Size::new(radius * 2.0, radius * 2.0);
            if state.selected_index == Some(index) {
                frame.fill_rectangle(origin, point_size, palette.selected_control_point);
            } else {
                frame.stroke_rectangle(
                    origin,
                    point_size,
                    Stroke {
                        style: palette.control_point.into(),
                        width: self.style.control_point_stroke_width,
                        ..Default::default()
                    },
                );
            }
        }

        renderer.with_translation(iced_core::Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.draw_geometry(frame.into_geometry())
        });
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if tree.state.downcast_ref::<CurveEditState>().dragging {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a, Message, Renderer> From<CurveEdit<'a, Message>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: iced_core::Renderer + iced_graphics::geometry::Renderer + 'a,
{
    fn from(widget: CurveEdit<'a, Message>) -> Self {
        Element::new(widget)
    }
}

#[derive(Default)]
struct CurveEditState {
    selected_index: Option<usize>,
    dragging: bool,
    drag_curve: Option<CubicCurve>,
}

#[derive(Clone, Copy)]
pub struct CurveEditStyle {
    pub grid_resolution: usize,
    pub curve_resolution: usize,
    pub control_point_radius: f32,
    pub curve_stroke_width: f32,
    pub grid_stroke_width: f32,
    pub control_point_stroke_width: f32,
    pub border_width: f32,
    pub background: Option<Color>,
    pub border: Option<Color>,
    pub grid: Option<Color>,
    pub curve: Option<Color>,
    pub control_point: Option<Color>,
    pub selected_control_point: Option<Color>,
}

impl Default for CurveEditStyle {
    fn default() -> Self {
        Self {
            grid_resolution: 4,
            curve_resolution: 128,
            control_point_radius: 4.0,
            curve_stroke_width: 1.5,
            grid_stroke_width: 0.5,
            control_point_stroke_width: 1.0,
            border_width: 1.0,
            background: None,
            border: None,
            grid: None,
            curve: None,
            control_point: None,
            selected_control_point: None,
        }
    }
}

impl CurveEditStyle {
    fn resolve(self, theme: &Theme) -> ResolvedCurveEditStyle {
        let palette = theme.extended_palette();
        ResolvedCurveEditStyle {
            background: self.background.unwrap_or(palette.background.base.color),
            border: self.border.unwrap_or(palette.background.strong.color),
            grid: self.grid.unwrap_or(palette.background.strong.color),
            curve: self.curve.unwrap_or(palette.primary.base.color),
            control_point: self.control_point.unwrap_or(palette.primary.base.color),
            selected_control_point: self
                .selected_control_point
                .unwrap_or(palette.primary.strong.color),
        }
    }
}

struct ResolvedCurveEditStyle {
    background: Color,
    border: Color,
    grid: Color,
    curve: Color,
    control_point: Color,
    selected_control_point: Color,
}

fn local_to_normalized(local: Point, size: Size) -> Vec2 {
    Vec2::new(
        (local.x / size.width).clamp(0.0, 1.0),
        (1.0 - local.y / size.height).clamp(0.0, 1.0),
    )
}

fn screen_to_normalized(screen: Point, bounds: Rectangle) -> Vec2 {
    local_to_normalized(
        Point::new(screen.x - bounds.x, screen.y - bounds.y),
        bounds.size(),
    )
}

fn normalized_to_local(point: Vec2, size: Size) -> Vec2 {
    Vec2::new(point.x * size.width, (1.0 - point.y) * size.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_local_and_normalized_coordinates() {
        let size = Size::new(200.0, 100.0);
        let normalized = local_to_normalized(Point::new(50.0, 75.0), size);
        assert_eq!(normalized, Vec2::new(0.25, 0.25));
        assert_eq!(normalized_to_local(normalized, size), Vec2::new(50.0, 75.0));
    }

    #[test]
    fn converts_screen_coordinates_relative_to_bounds() {
        let bounds = Rectangle::new(Point::new(100.0, 50.0), Size::new(200.0, 100.0));
        assert_eq!(
            screen_to_normalized(Point::new(150.0, 125.0), bounds),
            Vec2::new(0.25, 0.25)
        );
    }
}
