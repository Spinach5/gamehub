use crate::app::Message;
use crate::styles::{self, Tone};
use crate::theme;
use crate::widgets::layout::muted;
use iced::font::Weight;
use iced::widget::{Button, button, container, text, toggler, tooltip};
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
    button(text(value).size(12).font(theme::font(Weight::Normal)))
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
    button(text(value).size(11).font(theme::font(Weight::Normal)))
        .padding([7, 8])
        .style(styles::quiet)
}

pub fn switch_row<'a>(
    title: impl Into<String>,
    description: impl Into<String>,
    value: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    let label = iced::widget::column![
        text(title.into())
            .size(13)
            .font(theme::font(Weight::Semibold)),
        muted(description).size(11),
    ]
    .spacing(2)
    .width(Length::Fill);

    switch_row_shell(label.into(), value, on_toggle)
}

/// A switch row that keeps its explanation in a hover tooltip, so the card
/// stays compact until the user asks for the details.
pub fn switch_row_with_tooltip<'a>(
    title: impl Into<String>,
    tooltip_text: impl Into<String>,
    value: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    let label = iced::widget::row![
        text(title.into())
            .size(13)
            .font(theme::font(Weight::Semibold)),
        tooltip(
            info_icon(),
            text(tooltip_text.into())
                .size(11)
                .width(Length::Fixed(280.0))
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            tooltip::Position::Top,
        )
        .gap(6)
        .padding(8)
        .style(styles::tooltip_panel),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    switch_row_shell(label.into(), value, on_toggle)
}

fn switch_row_shell<'a>(
    label: Element<'a, Message>,
    value: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    iced::widget::row![
        label,
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

fn info_icon<'a>() -> Element<'a, Message> {
    container(
        text("!")
            .size(10)
            .font(theme::font(Weight::Bold))
            .style(|theme| iced::widget::text::Style {
                color: Some(styles::tone_colors(theme, Tone::Info).1),
            }),
    )
    .center(16)
    .style(|theme| styles::badge(theme, Tone::Info))
    .into()
}
