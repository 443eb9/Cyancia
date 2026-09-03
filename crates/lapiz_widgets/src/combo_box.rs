use iced_core::{Element, Length, Padding, Pixels, Theme};
use iced_wgpu::Renderer;
use iced_widget::{overlay::menu, pick_list, text_input};

pub use iced_widget::combo_box::Catalog;

#[derive(Debug, Clone)]
pub struct State<T> {
    combo: iced_widget::combo_box::State<T>,
    options: Vec<T>,
}

impl<T> State<T>
where
    T: std::fmt::Display + Clone,
{
    pub fn new(options: Vec<T>) -> Self {
        Self {
            combo: iced_widget::combo_box::State::new(options.clone()),
            options,
        }
    }

    pub fn options(&self) -> &[T] {
        &self.options
    }

    pub fn push(&mut self, option: T) {
        self.combo.push(option.clone());
        self.options.push(option);
    }
}

pub struct ComboBox<'a, T, Message>
where
    T: std::fmt::Display + Clone,
{
    state: &'a State<T>,
    placeholder: String,
    selected: Option<&'a T>,
    on_selected: Box<dyn Fn(T) -> Message + 'static>,
    searchable: bool,
    width: Length,
    menu_height: Length,
    padding: Padding,
    size: Pixels,
    input_class: <Theme as text_input::Catalog>::Class<'a>,
    menu_class: <Theme as menu::Catalog>::Class<'a>,
}

impl<'a, T, Message> ComboBox<'a, T, Message>
where
    T: std::fmt::Display + Clone,
{
    pub fn new(
        state: &'a State<T>,
        placeholder: &str,
        selected: Option<&'a T>,
        on_selected: impl Fn(T) -> Message + 'static,
    ) -> Self {
        Self {
            state,
            placeholder: placeholder.to_owned(),
            selected,
            on_selected: Box::new(on_selected),
            searchable: true,
            width: Length::Shrink,
            menu_height: Length::Shrink,
            padding: Padding::from([5, 8]),
            size: Pixels(12.0),
            input_class: Box::new(crate::text_input::default),
            menu_class: Box::new(menu_style),
        }
    }

    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    pub fn selection_only(self) -> Self {
        self.searchable(false)
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn menu_height(mut self, height: impl Into<Length>) -> Self {
        self.menu_height = height.into();
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    pub fn input_style(
        mut self,
        style: impl Fn(&Theme, text_input::Status) -> text_input::Style + 'a,
    ) -> Self {
        self.input_class = Box::new(style);
        self
    }

    pub fn menu_style(mut self, style: impl Fn(&Theme) -> menu::Style + 'a) -> Self {
        self.menu_class = Box::new(style);
        self
    }

    pub fn input_class(
        mut self,
        class: impl Into<<Theme as text_input::Catalog>::Class<'a>>,
    ) -> Self {
        self.input_class = class.into();
        self
    }

    pub fn menu_class(mut self, class: impl Into<<Theme as menu::Catalog>::Class<'a>>) -> Self {
        self.menu_class = class.into();
        self
    }
}

impl<'a, T, Message> From<ComboBox<'a, T, Message>> for Element<'a, Message, Theme, Renderer>
where
    T: std::fmt::Display + Clone + PartialEq + 'a + 'static,
    Message: Clone + 'a + 'static,
{
    fn from(value: ComboBox<'a, T, Message>) -> Self {
        if value.searchable {
            iced_widget::ComboBox::new(
                &value.state.combo,
                &value.placeholder,
                value.selected,
                value.on_selected,
            )
            .width(value.width)
            .menu_height(value.menu_height)
            .padding(value.padding)
            .size(value.size)
            .input_class(value.input_class)
            .menu_class(value.menu_class)
            .into()
        } else {
            iced_widget::PickList::new(
                value.state.options.as_slice(),
                value.selected,
                value.on_selected,
            )
            .placeholder(value.placeholder)
            .width(value.width)
            .menu_height(value.menu_height)
            .padding(value.padding)
            .text_size(value.size)
            .style(pick_list_style)
            .menu_class(value.menu_class)
            .into()
        }
    }
}

pub fn menu_style(theme: &Theme) -> menu::Style {
    iced_widget::overlay::menu::default(theme)
}

pub fn pick_list_style(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    iced_widget::pick_list::default(theme, status)
}
