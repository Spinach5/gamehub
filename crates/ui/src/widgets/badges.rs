use crate::app::Message;
use crate::styles::{self, Tone};
use crate::theme;
use iced::Element;
use iced::font::Weight;
use iced::widget::{container, text};

pub fn badge<'a>(label: impl Into<String>, tone: Tone) -> Element<'a, Message> {
    container(
        text(label.into())
            .size(11)
            .font(theme::font(Weight::Bold))
            .style(move |theme| iced::widget::text::Style {
                color: Some(styles::tone_colors(theme, tone).1),
            }),
    )
    .padding([3, 9])
    .style(move |theme| styles::badge(theme, tone))
    .into()
}
