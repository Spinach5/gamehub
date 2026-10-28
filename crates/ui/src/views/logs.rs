use crate::app::{App, Message, Screen};
use crate::styles;
use crate::theme;
use crate::widgets::{controls, layout};
use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length};

fn log_text_style(theme: &iced::Theme) -> iced::widget::text::Style {
    iced::widget::text::Style {
        color: Some(theme::tokens(theme).log_text),
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(game_id) = app.logs_game else {
        return column![
            controls::secondary(app.tr("common.back")).on_press(Message::Navigate(Screen::Library)),
            layout::empty_state(app.tr("logs.empty.title"), app.tr("logs.empty.body"), None),
        ]
        .spacing(16)
        .into();
    };

    let name = app
        .games
        .iter()
        .find(|game| game.id == game_id)
        .map(|game| game.name.as_str())
        .unwrap_or_else(|| app.tr("logs.game_fallback"));

    let lines = crate::app::LOG_TAIL_LINES.to_string();
    let header = layout::page_header(
        app.tr_args("logs.title", &[("name", name)]),
        app.tr_args("logs.subtitle", &[("lines", &lines)]),
        row![
            controls::secondary(app.tr("common.back")).on_press(Message::Navigate(Screen::Library)),
            controls::secondary(app.tr("common.refresh")).on_press(Message::RefreshLogs),
        ]
        .spacing(8),
    );

    let line_count = if app.logs_content.is_empty() {
        0
    } else {
        app.logs_content.lines().count()
    };
    let count = line_count.to_string();

    let body: Element<'_, Message> = if let Some(error) = &app.logs_error {
        container(
            text(app.tr_args("logs.read_failed", &[("error", error)]))
                .size(13)
                .style(log_text_style),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
    } else if app.logs_content.is_empty() {
        container(text(app.tr("logs.no_logs")).size(13).style(log_text_style))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into()
    } else {
        scrollable(
            text(app.logs_content.as_str())
                .font(iced::Font::MONOSPACE)
                .size(12)
                .wrapping(iced::widget::text::Wrapping::None)
                .style(log_text_style),
        )
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .style(styles::log_scrollable)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    };

    let mut panel_content = column![]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill);
    if app.logs_error.is_none() {
        panel_content = panel_content.push(
            text(app.tr_plural_args(
                "logs.line_count.one",
                "logs.line_count.other",
                line_count as u64,
                &[("count", &count)],
            ))
            .size(10)
            .style(log_text_style),
        );
    }
    panel_content = panel_content.push(body);

    let panel = container(panel_content)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(styles::log_panel);

    column![header, panel]
        .spacing(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
