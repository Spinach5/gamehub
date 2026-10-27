use crate::app::{App, Message, Screen};
use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(game_id) = app.logs_game else {
        return column![
            button("返回").on_press(Message::Navigate(Screen::Library)),
            text("未选择游戏"),
        ]
        .spacing(12)
        .padding(20)
        .into();
    };

    let name = app
        .games
        .iter()
        .find(|game| game.id == game_id)
        .map(|game| game.name.as_str())
        .unwrap_or("游戏");

    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text(format!("{name} 的日志")).size(24),
        iced::widget::horizontal_space(),
        button("刷新").on_press(Message::RefreshLogs),
    ]
    .spacing(8);

    let body: Element<'_, Message> = if app.logs_content.is_empty() {
        text("该游戏还没有启动日志").size(14).into()
    } else {
        scrollable(
            text(app.logs_content.as_str())
                .font(iced::Font::MONOSPACE)
                .size(12),
        )
        .height(Length::Fill)
        .into()
    };

    column![header, body]
        .spacing(12)
        .padding(20)
        .height(Length::Fill)
        .into()
}
