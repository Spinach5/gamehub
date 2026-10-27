use crate::app::{App, Message, Screen};
use iced::widget::{button, column, container, row, scrollable, text};
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
        let state = app.launch_state_label(game.id);
        let info = column![
            row![
                text(game.name.as_str()).size(18),
                text(state.unwrap_or_default()).size(12),
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
        .width(Length::Fill);

        let actions = row![
            button("启动").on_press_maybe(state.is_none().then_some(Message::Launch(game.id))),
            button("设置").on_press(Message::SelectGame(game.id)),
            button("日志").on_press(Message::OpenLogs(game.id)),
        ]
        .spacing(4);

        let card = container(
            row![info, actions]
                .spacing(12)
                .align_y(iced::Alignment::Center),
        )
        .padding(12)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.weak.color.into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

        list = list.push(card);
    }

    column![header, scrollable(list).height(Length::Fill),]
        .spacing(16)
        .padding(20)
        .into()
}
