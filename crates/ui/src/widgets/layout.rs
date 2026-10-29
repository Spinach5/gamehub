use crate::app::Message;
use crate::styles;
use crate::theme::{self, font};
use iced::font::Weight;
use iced::widget::{button, container, text};
use iced::{Element, Length};

pub fn muted<'a>(value: impl Into<String>) -> iced::widget::Text<'a> {
    text(value.into())
        .font(font(Weight::Normal))
        .style(|t| iced::widget::text::Style {
            color: Some(theme::tokens(t).text_muted),
        })
}

pub fn faint<'a>(value: impl Into<String>) -> iced::widget::Text<'a> {
    text(value.into())
        .font(font(Weight::Normal))
        .style(|t| iced::widget::text::Style {
            color: Some(theme::tokens(t).text_faint),
        })
}

pub fn page_header<'a>(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    actions: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced::widget::row![
        iced::widget::column![
            text(title.into())
                .font(font(Weight::Bold))
                .size(26)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            muted(subtitle).size(12),
        ]
        .spacing(2)
        .width(Length::Fill),
        actions.into(),
    ]
    .width(Length::Fill)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn nav_item<'a>(label: &'a str, active: bool, message: Message) -> Element<'a, Message> {
    let style: fn(&iced::Theme, button::Status) -> button::Style = if active {
        styles::nav_active
    } else {
        styles::nav_item
    };
    let bar = container("")
        .width(Length::Fixed(3.0))
        .height(Length::Fixed(16.0))
        .style(move |t| container::Style {
            background: Some(
                if active {
                    theme::tokens(t).primary
                } else {
                    iced::Color::TRANSPARENT
                }
                .into(),
            ),
            border: iced::Border {
                radius: 2.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    button(
        iced::widget::row![
            bar,
            text(label).size(13).font(font(if active {
                Weight::Semibold
            } else {
                Weight::Normal
            })),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([9, 10])
    .width(Length::Fill)
    .style(style)
    .on_press(message)
    .into()
}

pub fn card<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(16)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

pub fn section_card<'a>(
    title: impl Into<String>,
    description: Option<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut header =
        iced::widget::column![text(title.into()).font(font(Weight::Bold)).size(16)].spacing(2);
    if let Some(description) = description {
        header = header.push(muted(description).size(12));
    }

    container(
        iced::widget::column![header, content.into()]
            .spacing(12)
            .width(Length::Fill),
    )
    .padding(18)
    .width(Length::Fill)
    .style(styles::card)
    .into()
}

pub fn field<'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced::widget::row![
        container(muted(label).size(12)).width(Length::Fixed(72.0)),
        content.into(),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn empty_state<'a>(
    title: impl Into<String>,
    hint: impl Into<String>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut content = iced::widget::column![
        text(title.into()).font(font(Weight::Bold)).size(18),
        muted(hint).size(12),
    ]
    .spacing(6)
    .align_x(iced::Alignment::Center);

    if let Some(action) = action {
        content = content.push(container(action).padding([8, 0]));
    }

    container(content)
        .width(Length::Fill)
        .padding([56, 24])
        .align_x(iced::Alignment::Center)
        .style(styles::card)
        .into()
}

pub fn centered_message(message: &str) -> Element<'_, Message> {
    container(muted(message).size(14))
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
}

pub fn fatal(lang: crate::i18n::Lang, message: &str) -> Element<'_, Message> {
    let card = container(
        iced::widget::column![
            text(crate::i18n::tr(lang, "app.fatal.title"))
                .font(font(Weight::Bold))
                .size(24),
            muted(message.to_owned())
                .size(13)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        ]
        .spacing(10)
        .width(Length::Fill),
    )
    .padding(28)
    .width(Length::Fill)
    .max_width(560)
    .style(styles::card);

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .style(styles::canvas)
        .into()
}
