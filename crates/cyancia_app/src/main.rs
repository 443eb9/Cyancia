use crate::main_view::MainView;

mod main_view;

fn main() {
    iced::application(MainView::new, MainView::update, MainView::view)
        .subscription(MainView::subscription)
        .run()
        .unwrap();
}
