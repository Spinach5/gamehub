use crate::app::Message;
use crate::styles;
use crate::theme;
use crate::widgets::layout::muted;
use iced::font::Weight;
use iced::widget::{Button, button, text, toggler};
use iced::{Element, Length};

fn label<'a>(value: &'a str, size: u16) -> iced::widget::Text<'a> {
    text(value).size(size).font(theme::font(Weight::Semibold))
}

pub fn primary<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 13))
        .padding([9, 14])
        .style(styles::primary)
}

pub fn secondary<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 13))
        .padding([9, 14])
        .style(styles::secondary)
}

pub fn danger<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 13))
        .padding([9, 14])
        .style(styles::danger)
}

pub fn quiet<'a>(value: &'a str) -> Button<'a, Message> {
    button(text(value).size(12))
        .padding([6, 8])
        .style(styles::quiet)
}

pub fn primary_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 11))
        .padding([7, 10])
        .style(styles::primary)
}

pub fn secondary_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 11))
        .padding([7, 10])
        .style(styles::secondary)
}

pub fn danger_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 11))
        .padding([7, 10])
        .style(styles::danger)
}

pub fn quiet_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(text(value).size(11))
        .padding([7, 8])
        .style(styles::quiet)
}

pub fn switch_row<'a>(
    title: impl Into<String>,
    description: impl Into<String>,
    value: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    iced::widget::row![
        iced::widget::column![
            text(title.into())
                .size(13)
                .font(theme::font(Weight::Semibold)),
            muted(description).size(11),
        ]
        .spacing(2)
        .width(Length::Fill),
        toggler(value)
            .size(24.0)
            .on_toggle(on_toggle)
            .style(styles::toggler),
    ]
    .spacing(16)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center)
    .into()
}
