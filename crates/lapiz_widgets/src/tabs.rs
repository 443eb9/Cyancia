use iced_core::{
    Border, Element, Event, Layout, Length, Pixels, Point, Rectangle, Renderer as _, Shadow, Shell,
    Size, Theme, Vector, alignment, layout, overlay, pointer,
    pointer::mouse,
    renderer,
    text::{self, LineHeight, Renderer as _, Shaping, Text, paragraph, paragraph::Plain},
    widget::{self, Tree, tree},
};
use iced_widget::button::{Status, Style};
use lapiz_runtime::Renderer;

use crate::{
    button::{Button, activated_style, transparent},
    flex::{self, Flex},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    #[default]
    Line,
    Block,
}

struct Tab<'a, Message> {
    content: Element<'a, Message, Theme, Renderer>,
    selected: bool,
    message: Option<Message>,
}

pub struct TabBar<'a, Message> {
    tabs: Vec<Tab<'a, Message>>,
    variant: Variant,
    width: Length,
    height: Length,
}

impl<'a, Message> TabBar<'a, Message> {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            variant: Variant::Line,
            width: Length::Fit,
            height: Length::Fixed(26.0),
        }
    }

    pub fn push(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        selected: bool,
        message: Message,
    ) -> Self {
        self.tabs.push(Tab {
            content: content.into(),
            selected,
            message: Some(message),
        });
        self
    }

    pub fn push_disabled(
        mut self,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        selected: bool,
    ) -> Self {
        self.tabs.push(Tab {
            content: content.into(),
            selected,
            message: None,
        });
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

    pub fn line(mut self) -> Self {
        self.variant = Variant::Line;
        self
    }

    pub fn block(mut self) -> Self {
        self.variant = Variant::Block;
        self
    }
}

impl<Message> Default for TabBar<'_, Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message: 'a> From<TabBar<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: TabBar<'a, Message>) -> Self {
        let variant = value.variant;
        let tabs = value.tabs.into_iter().map(move |tab| {
            let selected = tab.selected;
            Button::new(tab.content)
                .height(Length::Fill)
                .padding([0, 12])
                .style(move |theme, status| style(theme, status, variant, selected))
                .on_press_maybe(tab.message)
                .into()
        });
        Flex::row(tabs)
            .width(value.width)
            .height(value.height)
            .style(tab_bar)
            .into()
    }
}

fn style(theme: &Theme, status: Status, variant: Variant, selected: bool) -> Style {
    if variant == Variant::Block && selected {
        return activated_style(theme, status);
    }
    let p = theme.palette();
    let mut style = transparent(theme, status);
    if selected {
        style.text_color = p.background.base.text;
        if variant == Variant::Line {
            style.border.width = 0.0;
            style.shadow = Shadow {
                color: p.primary.base.color,
                offset: Vector::new(0.0, 2.0),
                blur_radius: 0.0,
            };
        }
    }
    style
}

fn tab_bar(theme: &Theme, _status: flex::Status) -> flex::Style {
    let p = theme.palette();
    flex::Style::default().border(Border {
        radius: 0.0.into(),
        width: 1.0,
        color: p.background.strong.color,
    })
}

// TODO Dock groups should also use this
pub struct TabbedView<'a, Message> {
    pages: Vec<Page<'a, Message>>,
    selected: usize,
    on_select: Box<dyn Fn(usize) -> Message + 'a>,
    font_size: Pixels,
    padding: f32,
}

struct Page<'a, Message> {
    title: String,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a> TabbedView<'a, Message> {
    pub fn new(selected: usize, on_select: impl Fn(usize) -> Message + 'a) -> Self {
        Self {
            pages: Vec::new(),
            selected,
            on_select: Box::new(on_select),
            font_size: Pixels(11.0),
            padding: 7.0,
        }
    }

    pub fn tab(
        mut self,
        title: impl Into<String>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.pages.push(Page {
            title: title.into(),
            content: content.into(),
        });
        self
    }

    pub fn font_size(mut self, font_size: impl Into<Pixels>) -> Self {
        self.font_size = font_size.into();
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }
}

impl<'a, Message: 'a> From<TabbedView<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(value: TabbedView<'a, Message>) -> Self {
        let TabbedView {
            pages,
            selected,
            on_select,
            font_size,
            padding,
        } = value;
        let bar_height = font_size.0 + padding * 2.0;
        let mut children = Vec::with_capacity(pages.len() + 1);
        children.push(
            TabRow {
                titles: pages.iter().map(|page| page.title.clone()).collect(),
                selected,
                on_select,
                font_size,
                padding,
            }
            .into(),
        );
        children.extend(pages.into_iter().map(|page| page.content));

        Element::new(TabbedViewWidget {
            children,
            selected,
            bar_height,
        })
    }
}

struct TabbedViewWidget<'a, Message> {
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    selected: usize,
    bar_height: f32,
}

impl<Message> iced_core::Widget<Message, Theme, Renderer> for TabbedViewWidget<'_, Message> {
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.children);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
        let bar_height = self.bar_height.min(size.height);
        let content_height = (size.height - bar_height).max(0.0);

        let mut nodes = Vec::with_capacity(self.children.len());
        let row = self.children[0].as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &layout::Limits::new(Size::ZERO, Size::new(size.width, bar_height)),
        );
        nodes.push(row);

        let page_index = self.selected + 1;
        for (index, child) in self.children.iter_mut().enumerate().skip(1) {
            if index == page_index {
                let node = child.as_widget_mut().layout(
                    &mut tree.children[index],
                    renderer,
                    &layout::Limits::new(Size::ZERO, Size::new(size.width, content_height)),
                );
                nodes.push(node.move_to(Point::new(0.0, bar_height)));
            } else {
                nodes.push(layout::Node::new(Size::ZERO));
            }
        }

        layout::Node::with_children(size, nodes)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let layouts = layout.children().collect::<Vec<_>>();
        self.children[0].as_widget_mut().update(
            &mut tree.children[0],
            event,
            layouts[0],
            cursor,
            renderer,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }
        let page_index = self.selected + 1;
        if let (Some(page), Some(state), Some(layout)) = (
            self.children.get_mut(page_index),
            tree.children.get_mut(page_index),
            layouts.get(page_index),
        ) {
            page.as_widget_mut()
                .update(state, event, *layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let layouts = layout.children().collect::<Vec<_>>();
        let row = self.children[0].as_widget().mouse_interaction(
            &tree.children[0],
            layouts[0],
            cursor,
            viewport,
            renderer,
        );
        let page_index = self.selected + 1;
        let page = match (
            self.children.get(page_index),
            tree.children.get(page_index),
            layouts.get(page_index),
        ) {
            (Some(child), Some(state), Some(layout)) => child
                .as_widget()
                .mouse_interaction(state, *layout, cursor, viewport, renderer),
            _ => mouse::Interaction::None,
        };
        row.max(page)
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
        let layouts = layout.children().collect::<Vec<_>>();
        self.children[0].as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layouts[0],
            cursor,
            viewport,
        );
        let page_index = self.selected + 1;
        if let (Some(page), Some(state), Some(layout)) = (
            self.children.get(page_index),
            tree.children.get(page_index),
            layouts.get(page_index),
        ) {
            page.as_widget()
                .draw(state, renderer, theme, style, *layout, cursor, viewport);
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let layouts = layout.children().collect::<Vec<_>>();
        let page_index = self.selected + 1;
        for index in [0, page_index] {
            let (Some(child), Some(state), Some(layout)) = (
                self.children.get_mut(index),
                tree.children.get_mut(index),
                layouts.get(index),
            ) else {
                continue;
            };
            child
                .as_widget_mut()
                .operate(state, *layout, renderer, operation);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let page_index = self.selected + 1;
        let page = self.children.get_mut(page_index)?;
        let state = tree.children.get_mut(page_index)?;
        let layout = layout.children().nth(page_index)?;
        page.as_widget_mut()
            .overlay(state, layout, renderer, viewport, translation)
    }
}

#[derive(Debug, Default)]
struct TabRowState {
    hovered: Option<usize>,
    pressed: Option<usize>,
    labels: Vec<Plain<<Renderer as text::Renderer>::Paragraph>>,
    bounds: Vec<Rectangle>,
}

fn hit_test(bounds: &[Rectangle], cursor_rel: Point) -> Option<usize> {
    for (i, bounds) in bounds.iter().enumerate() {
        if bounds.contains(cursor_rel) {
            return Some(i);
        }
    }

    None
}

struct TabRow<'a, Message> {
    titles: Vec<String>,
    selected: usize,
    on_select: Box<dyn Fn(usize) -> Message + 'a>,
    font_size: Pixels,
    padding: f32,
}

impl<Message> TabRow<'_, Message> {
    fn height(&self) -> f32 {
        self.font_size.0 + self.padding * 2.0
    }
}

impl<Message> iced_core::Widget<Message, Theme, Renderer> for TabRow<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<TabRowState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(TabRowState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height()))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<TabRowState>();
        state.labels.clear();
        state.bounds.clear();
        let height = self.height();
        let available = limits.max().width;

        let mut natural_widths = Vec::with_capacity(self.titles.len());
        for title in &self.titles {
            let p = paragraph::Plain::new(Text {
                content: title.clone(),
                bounds: limits.max(),
                size: self.font_size,
                line_height: LineHeight::Relative(1.0),
                font: renderer.default_font(),
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: Shaping::Auto,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: None,
            });
            natural_widths.push(p.min_width() + self.padding * 2.0);
            state.labels.push(p);
        }

        let total = natural_widths.iter().sum::<f32>();
        let overflow = total > available;

        let mut x = 0.0;
        for (i, natural) in natural_widths.iter().enumerate() {
            let width = if overflow {
                if i + 1 == natural_widths.len() {
                    (available - x).max(0.0)
                } else {
                    natural * available / total
                }
            } else {
                *natural
            };
            state.bounds.push(Rectangle {
                x,
                y: 0.0,
                width,
                height,
            });
            x += width;
        }
        layout::Node::new(limits.resolve(Length::Fill, Length::Fixed(height), limits.max()))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<TabRowState>();
        let bounds = layout.bounds();
        let p = theme.palette();

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..Default::default()
            },
            p.background.base.color,
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    y: bounds.y + bounds.height - 1.0,
                    height: 1.0,
                    ..bounds
                },
                ..Default::default()
            },
            p.background.strong.color,
        );

        for i in 0..self.titles.len() {
            let tab_rect_rel = state.bounds[i];
            let tab_rect = Rectangle {
                x: tab_rect_rel.x + bounds.x,
                y: tab_rect_rel.y + bounds.y,
                ..tab_rect_rel
            };

            let is_selected = i == self.selected;
            let is_hovered = state.hovered == Some(i);
            let (background, text_color) = if is_selected {
                (p.background.weakest.color, p.background.base.text)
            } else if is_hovered {
                (p.primary.weak.color, p.primary.weak.text)
            } else {
                (p.background.base.color, p.background.weak.text)
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: tab_rect,
                    ..Default::default()
                },
                background,
            );
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: tab_rect.x + tab_rect.width - 1.0,
                        width: 1.0,
                        ..tab_rect
                    },
                    ..Default::default()
                },
                p.background.strong.color,
            );
            if is_selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            height: 2.0,
                            ..tab_rect
                        },
                        ..Default::default()
                    },
                    p.primary.base.color,
                );
            }

            let mut label = state.labels[i]
                .as_text()
                .with_content(state.labels[i].content().to_string());
            let inner = Rectangle {
                x: tab_rect.x + self.padding,
                width: (tab_rect.width - self.padding * 2.0).max(0.0),
                ..tab_rect
            };
            let position = if state.labels[i].min_width() > inner.width {
                // Keep the start of the title visible instead of cutting both ends.
                label.align_x = text::Alignment::Left;
                Point::new(inner.x, tab_rect.center().y)
            } else {
                tab_rect.center()
            };
            let clip = if label.align_x == text::Alignment::Left {
                inner
            } else {
                tab_rect
            };
            renderer.fill_text(label, position, text_color, clip);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<TabRowState>();

        match event {
            Event::Pointer(pointer::Event::PointerMoved { position, .. }) => {
                let hovered = bounds
                    .contains(*position)
                    .then(|| Point::new(position.x - bounds.x, position.y - bounds.y))
                    .and_then(|position| hit_test(&state.bounds, position));
                if state.hovered != hovered {
                    state.hovered = hovered;
                    shell.request_redraw();
                }
                if state.pressed.is_some() {
                    shell.capture_event();
                }
            }

            Event::Pointer(event @ pointer::Event::PointerPressed { position, .. })
                if event.is_primary_press() =>
            {
                if !bounds.contains(*position) {
                    return;
                }

                if let Some(index) = hit_test(
                    &state.bounds,
                    Point::new(position.x - bounds.x, position.y - bounds.y),
                ) {
                    state.pressed = Some(index);
                    shell.capture_event();
                }
            }

            Event::Pointer(event @ pointer::Event::PointerReleased { position, .. })
                if event.is_primary_release() =>
            {
                if let Some(index) = state.pressed.take() {
                    let released_on = bounds
                        .contains(*position)
                        .then(|| Point::new(position.x - bounds.x, position.y - bounds.y))
                        .and_then(|position| hit_test(&state.bounds, position));
                    if released_on == Some(index) {
                        shell.publish((self.on_select)(index));
                    }
                    shell.capture_event();
                    shell.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<TabRowState>();
        match cursor.position_in(layout.bounds()) {
            Some(position) => match hit_test(&state.bounds, position) {
                Some(_) => mouse::Interaction::Pointer,
                None => mouse::Interaction::default(),
            },
            None => mouse::Interaction::default(),
        }
    }
}

impl<'a, Message: 'a> From<TabRow<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(widget: TabRow<'a, Message>) -> Self {
        Element::new(widget)
    }
}
