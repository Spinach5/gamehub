use crate::app::{App, Message, Screen};
use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        text("游戏库").size(28),
        iced::widget::horizontal_space(),
        button("新建游戏").on_press(Message::NewGame),
        button("Runner 管理").on_press(Message::Navigate(Screen::RunnerManager)),
        button("设置").on_press(Message::Navigate(Screen::Settings)),
    ]
    .spacing(8);

    let mut list = column![].spacing(8);
    if app.games.is_empty() {
        list = list.push(text("还没有游戏。点击“新建游戏”选择一个 .exe。").size(14));
    }
    for game in &app.games {
        let running = app.running.contains_key(&game.id);
        let card = column![
            row![
                text(game.name.as_str()).size(18),
                text(if running { "运行中" } else { "" }).size(12),
            ]
            .spacing(8),
            text(game.executable_path.display().to_string()).size(12),
            text(format!(
                "最近游玩：{}",
                crate::widgets::format_timestamp(game.last_played_at)
            ))
            .size(12),
        ]
        .spacing(4)
        .padding(12);

        list = list.push(
            button(card)
                .on_press(Message::SelectGame(game.id))
                .width(Length::Fill),
        );
    }

    column![header, scrollable(list).height(Length::Fill),]
        .spacing(16)
        .padding(20)
        .into()
}
