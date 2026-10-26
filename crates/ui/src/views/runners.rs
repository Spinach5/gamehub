use crate::app::{App, DeleteTarget, Message, Screen};
use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("Runner 管理").size(24),
        iced::widget::horizontal_space(),
        button("重新探测系统 Wine")
            .on_press_maybe((!app.pending.detecting_wine).then_some(Message::DetectSystemWine)),
        button("导入 Proton 目录")
            .on_press_maybe((!app.pending.importing_proton).then_some(Message::ImportProton)),
    ]
    .spacing(8);

    let mut list = column![].spacing(10);
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
                text(if runner.verified {
                    "已验证"
                } else {
                    "未验证"
                })
                .size(12),
                text(if is_default { "默认" } else { "" }).size(12),
                iced::widget::horizontal_space(),
                button("设为默认").on_press_maybe(
                    (!is_default).then_some(Message::SetDefaultRunner(Some(runner.id)))
                ),
                button("删除").on_press(Message::ConfirmDelete(DeleteTarget::Runner(runner.id))),
            ]
            .spacing(8),
        );
    }

    column![header, scrollable(list).height(Length::Fill),]
        .spacing(16)
        .padding(20)
        .into()
}
