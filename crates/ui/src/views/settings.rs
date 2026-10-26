use crate::app::{App, Message, Screen};
use iced::Element;
use iced::widget::{button, column, row, text};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("设置").size(24),
    ]
    .spacing(12);

    let default_runner = app
        .settings
        .default_runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.clone())
        .unwrap_or_else(|| "未设置".into());

    let mut env_list = column![].spacing(4);
    if app.settings.global_env_rows.is_empty() {
        env_list = env_list.push(text("（空）"));
    }
    for entry in &app.settings.global_env_rows {
        env_list = env_list.push(text(format!("{}={}", entry.key, entry.value)));
    }

    column![
        header,
        crate::widgets::section("默认 Runner", text(default_runner)),
        crate::widgets::section(
            "Prefix 根目录",
            text(app.settings.prefix_root.display().to_string()),
        ),
        crate::widgets::section("全局环境变量", env_list),
    ]
    .spacing(12)
    .padding(20)
    .into()
}
