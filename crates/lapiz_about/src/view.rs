use std::sync::Arc;

use anyhow::Result;
use iced_core::{Element, Length, Size, Theme, widget::Void, window};
use iced_runtime::{
    Task,
    window::{close, drag, minimize, open, toggle_maximize},
};
use lapiz_i18n::t;
use lapiz_runtime::{
    Renderer, Services,
    windows::{WindowView, WindowViewId},
};
use lapiz_widgets::{
    button::Button, collapsible::Collapsible, flex::Flex, fluent_builder::WhenSome as _,
    label::Label, panel::Panel, scrollable::Scrollable, tabs::TabbedView, text_input::TextInput,
    title_bar::TitleBar,
};

use crate::{ABOUT_VIEW_ID, CrateEntry, GPL_LICENSE_TEXT, MIT_LICENSE_TEXT, ThirdPartyLicenses};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AboutTab {
    #[default]
    About,
    ThirdParty,
}

impl AboutTab {
    fn index(self) -> usize {
        match self {
            AboutTab::About => 0,
            AboutTab::ThirdParty => 1,
        }
    }

    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(AboutTab::About),
            1 => Some(AboutTab::ThirdParty),
            _ => None,
        }
    }
}

pub struct AboutView {
    window: window::Id,
    windows: Arc<[window::Id]>,
    tab: AboutTab,
    licenses: ThirdPartyLicenses,
    gpl_open: bool,
    mit_open: bool,
    search: String,
    filtered: Vec<usize>,
    selected: Option<usize>,
}

#[derive(Clone)]
pub enum AboutMessage {
    SelectTab(usize),
    ToggleGpl,
    ToggleMit,
    SearchChanged(String),
    SelectCrate(usize),
    Close,
    Maximize,
    Minimize,
    Drag,
}

impl WindowView for AboutView {
    type Message = AboutMessage;

    type BootParams = ();

    fn id() -> WindowViewId {
        WindowViewId::new(ABOUT_VIEW_ID)
    }

    fn boot(
        _params: Option<Self::BootParams>,
        _services: &mut Services,
    ) -> Result<(Self, Task<Self::Message>)> {
        let licenses = ThirdPartyLicenses::parse_embedded();
        let filtered = (0..licenses.crates.len()).collect();
        let (window, open) = open(window::Settings {
            decorations: false,
            size: Size {
                width: 760.0,
                height: 540.0,
            },
            #[cfg(target_os = "windows")]
            platform_specific: window::settings::PlatformSpecific {
                corner_preference: window::settings::platform::CornerPreference::DoNotRound,
                ..Default::default()
            },
            ..Default::default()
        });
        Ok((
            Self {
                window,
                windows: [window].into(),
                tab: AboutTab::default(),
                licenses,
                gpl_open: true,
                mit_open: false,
                search: String::new(),
                filtered,
                selected: None,
            },
            open.discard(),
        ))
    }

    fn view<'a>(
        &'a self,
        _: window::Id,
        _: &'a Services,
    ) -> impl Into<Element<'a, Self::Message, Theme, Renderer>> {
        let titlebar = TitleBar::new(Label::new(t!("about_title")).window_title())
            .on_close(AboutMessage::Close)
            .on_maximize(AboutMessage::Maximize)
            .on_minimize(AboutMessage::Minimize)
            .on_drag(AboutMessage::Drag);

        let tabs = TabbedView::new(self.tab.index(), AboutMessage::SelectTab)
            .tab(t!("about_tab"), self.view_about())
            .tab(t!("third_party_licenses_tab"), self.view_third_party());

        Panel::new(Flex::column([titlebar.into(), tabs.into()]))
    }

    fn update(
        &mut self,
        message: Self::Message,
        _services: &mut Services,
    ) -> impl Into<Task<Self::Message>> {
        match message {
            AboutMessage::SelectTab(index) => {
                if let Some(tab) = AboutTab::from_index(index) {
                    self.tab = tab;
                }
                Task::none()
            }
            AboutMessage::ToggleGpl => {
                self.gpl_open = !self.gpl_open;
                Task::none()
            }
            AboutMessage::ToggleMit => {
                self.mit_open = !self.mit_open;
                Task::none()
            }
            AboutMessage::SearchChanged(search) => {
                self.search = search;
                self.filter_crates();
                Task::none()
            }
            AboutMessage::SelectCrate(index) => {
                self.selected = Some(index);
                Task::none()
            }
            AboutMessage::Close => close(self.window),
            AboutMessage::Maximize => toggle_maximize(self.window),
            AboutMessage::Minimize => minimize(self.window, true),
            AboutMessage::Drag => drag(self.window),
        }
    }

    fn close(self, _: &mut Services) -> Task<()> {
        close(self.window)
    }

    fn windows(&self) -> Arc<[window::Id]> {
        self.windows.clone()
    }

    fn root_window(&self) -> Option<window::Id> {
        Some(self.window)
    }
}

impl AboutView {
    fn filter_crates(&mut self) {
        let query = self.search.to_lowercase();
        self.filtered = self
            .licenses
            .crates
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect();
    }

    fn view_about(&self) -> Element<'_, AboutMessage, Theme, Renderer> {
        let header = Flex::column([
            Label::new("Lapiz").strong().size(22).into(),
            Label::new(t!("about_version", version = env!("CARGO_PKG_VERSION")))
                .muted()
                .into(),
            Label::new(t!("about_license_line")).muted().into(),
        ])
        .gap(4);

        let gpl = Collapsible::new(
            Label::new(t!("gpl_license_header")).strong(),
            Panel::new(Label::new(GPL_LICENSE_TEXT).width(Length::Fill))
                .padding([8, 12])
                .width(Length::Fill),
            self.gpl_open,
        )
        .on_toggle(AboutMessage::ToggleGpl);
        let mit = Collapsible::new(
            Label::new(t!("mit_license_header")).strong(),
            Panel::new(Label::new(MIT_LICENSE_TEXT).width(Length::Fill))
                .padding([8, 12])
                .width(Length::Fill),
            self.mit_open,
        )
        .on_toggle(AboutMessage::ToggleMit);

        Scrollable::new(
            Flex::column([header.into(), gpl.into(), mit.into()])
                .gap(10)
                .padding(12)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_third_party(&self) -> Element<'_, AboutMessage, Theme, Renderer> {
        let items = if self.filtered.is_empty() {
            vec![Label::new(t!("about_no_results")).muted().into()]
        } else {
            self.filtered
                .iter()
                .map(|&index| {
                    let entry = &self.licenses.crates[index];
                    Button::new(Label::new(entry.name.clone()))
                        .width(Length::Fill)
                        .activated(self.selected == Some(index))
                        .on_press(AboutMessage::SelectCrate(index))
                        .into()
                })
                .collect::<Vec<_>>()
        };

        let sidebar = Panel::new(
            Flex::column([
                TextInput::new(&t!("about_search_placeholder"), &self.search)
                    .on_input(AboutMessage::SearchChanged)
                    .width(Length::Fill)
                    .into(),
                Scrollable::new(Flex::column(items).gap(2).width(Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
            ])
            .gap(6),
        )
        .padding(8)
        .width(240);

        let details = match self.selected.map(|index| &self.licenses.crates[index]) {
            Some(entry) => self.view_crate(entry),
            None => Void.into(),
        };

        Flex::row([sidebar.into(), details])
            .gap(8)
            .padding(8)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_crate(&self, entry: &CrateEntry) -> Element<'_, AboutMessage, Theme, Renderer> {
        let license = self.licenses.licenses.get(entry.license);
        let license_name = license
            .map(|license| license.name.clone())
            .unwrap_or_default();
        let license_text = license
            .map(|license| license.text.clone())
            .unwrap_or_default();

        let header = Flex::column(vec![
            Label::new(format!("{} {}", entry.name, entry.version))
                .strong()
                .size(14)
                .into(),
            Label::new(license_name).muted().into(),
        ])
        .gap(2)
        .when_some(entry.repository.clone(), |column, url| {
            column.push(Label::new(url).muted())
        });

        let text = Panel::new(
            Scrollable::new(Label::new(license_text).width(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill);

        Flex::column([header.into(), text.into()])
            .gap(8)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
