use crate::app::Message;
use crate::styles;
use crate::theme::font;
use crate::widgets::{controls, layout::muted};
use iced::font::Weight;
use iced::widget::{column, container, horizontal_space, opaque, row, text};
use iced::{Element, Length};

pub fn confirm_delete<'a>(
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
                controls::secondary("取消").on_press(cancel),
                controls::danger("确认删除").on_press(confirm),
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
