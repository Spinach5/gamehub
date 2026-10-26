use crate::app::{App, Message, Screen, SettingsAction};
use crate::draft::EnvAction;
use crate::widgets::{runner_choice, runner_choices, section};
use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("设置").size(24),
        iced::widget::horizontal_space(),
        button("保存")
            .on_press_maybe((!app.pending.settings_saving).then_some(Message::SaveSettings)),
    ]
    .spacing(8);

    let default_runner = section(
        "默认 Runner",
        pick_list(
            runner_choices(&app.runners, "无默认（使用游戏级选择）"),
            Some(runner_choice(
                &app.runners,
                "无默认（使用游戏级选择）",
                app.settings.default_runner_id,
            )),
            |choice| Message::Settings(SettingsAction::DefaultRunner(choice.value)),
        ),
    );

    let prefix_root = section(
        "Prefix 根目录",
        column![
            text(app.settings.prefix_root.display().to_string()),
            button("选择目录")
                .on_press(Message::Pick(crate::dialogs::DialogIntent::PickPrefixRoot)),
        ]
        .spacing(8),
    );

    let mut env_rows = column![].spacing(8);
    for (index, entry) in app.settings.global_env_rows.iter().enumerate() {
        env_rows =
            env_rows.push(
                row![
                    text_input("KEY", entry.key.as_str())
                        .on_input(move |key| Message::Settings(SettingsAction::Env(
                            EnvAction::Key { index, key }
                        )))
                        .width(Length::FillPortion(2)),
                    text_input("VALUE", entry.value.as_str())
                        .on_input(move |value| Message::Settings(SettingsAction::Env(
                            EnvAction::Value { index, value }
                        )))
                        .width(Length::FillPortion(3)),
                    button("删除").on_press(Message::Settings(SettingsAction::Env(
                        EnvAction::Remove(index)
                    ))),
                ]
                .spacing(8),
            );
    }
    env_rows = env_rows
        .push(button("添加变量").on_press(Message::Settings(SettingsAction::Env(EnvAction::Add))));

    let global_env = section("全局环境变量", env_rows);

    let content = column![header, default_runner, prefix_root, global_env,]
        .spacing(12)
        .padding(20);

    scrollable(content).height(Length::Fill).into()
}
