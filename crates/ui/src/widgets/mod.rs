pub mod badges;
mod choices;
pub mod controls;
pub mod cover;
pub mod layout;
pub mod modal;

pub use badges::badge;
pub use choices::*;
pub use controls::*;
pub use cover::cover;
pub use layout::*;
pub use modal::confirm_delete;

use crate::app::{ErrorNotice, Message};
use crate::styles;
pub use crate::styles::Tone;
use crate::theme::font;
use chrono::{DateTime, Local};
use iced::font::Weight;
use iced::widget::{column, container, text};
use iced::{Border, Element, Length};
use std::path::Path;

pub fn error_banner(lang: crate::i18n::Lang, error: &ErrorNotice) -> Element<'_, Message> {
    let mut content = column![
        iced::widget::row![
            badge(crate::i18n::tr(lang, "widgets.error"), Tone::Danger),
            text(error.title.clone())
                .size(14)
                .font(font(Weight::Semibold)),
            iced::widget::horizontal_space(),
            quiet_small(crate::i18n::tr(lang, "common.close")).on_press(Message::DismissError),
        ]
        .spacing(10)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center)
    ]
    .spacing(6)
    .width(Length::Fill);

    if let Some(detail) = &error.detail {
        content = content.push(
            muted(detail.clone())
                .size(12)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        );
    }

    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(|theme| {
            let (_, danger) = styles::tone_colors(theme, Tone::Danger);
            container::Style {
                background: Some(crate::theme::tokens(theme).surface.into()),
                border: Border {
                    color: danger,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Truncates `value` to `max_chars` characters, appending `…` when shortened.
pub fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Shortens `value` to `max_chars` characters, keeping both ends and
/// inserting `…` in the middle.
pub fn middle_truncate(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_owned();
    }
    let keep = max_chars.saturating_sub(1);
    let front = keep / 2;
    let back = keep - front;
    let head: String = chars[..front].iter().collect();
    let tail: String = chars[chars.len() - back..].iter().collect();
    format!("{head}…{tail}")
}

pub fn path_text(lang: crate::i18n::Lang, path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| crate::i18n::tr(lang, "common.none").into())
}

pub fn format_timestamp(lang: crate::i18n::Lang, timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0))
        .map(|date| {
            date.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| crate::i18n::tr(lang, "time.never").into())
}

#[cfg(test)]
mod tests {
    use super::middle_truncate;

    #[test]
    fn middle_truncate_keeps_both_ends() {
        assert_eq!(middle_truncate("short", 10), "short");
        assert_eq!(
            middle_truncate("/very/long/path/to/proton", 10),
            "/ver…roton"
        );
    }
}
