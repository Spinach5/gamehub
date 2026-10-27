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
            controls::secondary("返回").on_press(Message::Navigate(Screen::Library)),
            layout::empty_state("未选择游戏", "请从游戏库打开一个游戏的日志", None),
        ]
        .spacing(16)
        .into();
    };

    let name = app
        .games
        .iter()
        .find(|game| game.id == game_id)
        .map(|game| game.name.as_str())
        .unwrap_or("游戏");

    let header = layout::page_header(
        format!("{name} 的日志"),
        format!("每秒自动刷新，显示最后 {} 行", crate::app::LOG_TAIL_LINES),
        row![
            controls::secondary("返回").on_press(Message::Navigate(Screen::Library)),
            controls::secondary("刷新").on_press(Message::RefreshLogs),
        ]
        .spacing(8),
    );

    let line_count = if app.logs_content.is_empty() {
        0
    } else {
        app.logs_content.lines().count()
    };

    let body: Element<'_, Message> = if let Some(error) = &app.logs_error {
        container(
            text(format!("读取日志失败：{error}"))
                .size(13)
                .style(log_text_style),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
    } else if app.logs_content.is_empty() {
        container(text("该游戏还没有启动日志").size(13).style(log_text_style))
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
            text(format!("{line_count} 行"))
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
