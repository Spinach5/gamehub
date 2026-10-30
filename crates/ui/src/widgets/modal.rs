use crate::app::Message;
use crate::styles;
use crate::theme::font;
use crate::widgets::{controls, layout::muted};
use iced::font::Weight;
use iced::widget::{column, container, horizontal_space, opaque, row, text};
use iced::{Element, Length};

pub fn platform_prompt<'a>(
    lang: crate::i18n::Lang,
    windows: Message,
    linux: Message,
    cancel: Message,
) -> Element<'a, Message> {
    let option = |button: iced::widget::Button<'a, Message>, hint: &'a str| {
        column![button.width(Length::Fill), muted(hint).size(11)]
            .spacing(6)
            .width(Length::FillPortion(1))
    };

    let card = container(
        column![
            text(crate::i18n::tr(lang, "new_game.title"))
                .font(font(Weight::Bold))
                .size(18)
                .width(Length::Fill),
            muted(crate::i18n::tr(lang, "new_game.body")).size(12),
            row![
                option(
                    controls::primary(crate::i18n::tr(lang, "new_game.windows")).on_press(windows),
                    crate::i18n::tr(lang, "new_game.windows.hint"),
                ),
                option(
                    controls::secondary(crate::i18n::tr(lang, "new_game.linux")).on_press(linux),
                    crate::i18n::tr(lang, "new_game.linux.hint"),
                ),
            ]
            .spacing(12)
            .width(Length::Fill),
            row![
                horizontal_space(),
                controls::quiet(crate::i18n::tr(lang, "common.cancel")).on_press(cancel),
            ]
            .width(Length::Fill),
        ]
        .spacing(14)
        .width(Length::Fill),
    )
    .padding(22)
    .max_width(460)
    .style(styles::modal_card);

    opaque(
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(styles::backdrop),
    )
}

pub fn confirm_delete<'a>(
    lang: crate::i18n::Lang,
    title: String,
    body: String,
    confirm: Message,
    cancel: Message,
) -> Element<'a, Message> {
    let card = container(
        column![
            text(title)
                .font(font(Weight::Bold))
                .size(18)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            muted(body)
                .size(12)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            row![
                horizontal_space(),
                controls::secondary(crate::i18n::tr(lang, "common.cancel")).on_press(cancel),
                controls::danger(crate::i18n::tr(lang, "common.confirm_delete")).on_press(confirm),
            ]
            .spacing(8)
            .width(Length::Fill),
        ]
        .spacing(14)
        .width(Length::Fill),
    )
    .padding(22)
    .max_width(430)
    .style(styles::modal_card);

    opaque(
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(styles::backdrop),
    )
}
