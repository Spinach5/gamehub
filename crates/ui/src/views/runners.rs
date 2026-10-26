use crate::app::{App, Message, Screen};
use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("Runner 管理").size(24),
    ]
    .spacing(12);

    let mut list = column![].spacing(8);
    if app.runners.is_empty() {
        list = list.push(text("尚未检测到 Runner。"));
    }
    for runner in &app.runners {
        let is_default = app.settings.default_runner_id == Some(runner.id);
        list = list.push(
            row![
                text(runner.name.as_str()).size(16),
                text(format!("{:?}", runner.kind)).size(12),
                text(runner.version_string.clone().unwrap_or_default()).size(12),
                text(if is_default { "默认" } else { "" }).size(12),
            ]
            .spacing(12),
        );
    }

    column![header, scrollable(list).height(Length::Fill),]
        .spacing(16)
        .padding(20)
        .into()
}
