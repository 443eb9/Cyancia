use iced_core::{Color, Theme};

pub fn surface(theme: &Theme) -> Color {
    theme.extended_palette().background.weakest.color
}

pub fn raised(theme: &Theme) -> Color {
    let p = theme.extended_palette();
    if p.is_dark {
        p.background.weaker.color
    } else {
        p.background.base.color
    }
}

pub fn field(theme: &Theme) -> Color {
    theme.extended_palette().background.base.color
}
