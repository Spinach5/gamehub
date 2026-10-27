use crate::app::{App, Message, Screen, SettingsAction};
use crate::draft::EnvAction;
use crate::widgets::{runner_choice, runner_choices, section};
use iced::widget::{button, checkbox, column, pick_list, row, scrollable, text, text_input};
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

    let nvapi_source = app.settings.dxvk_nvapi_source.as_deref();
    let mut compatibility = column![
        checkbox(
            "自动在前缀安装并更新 DXVK-NVAPI",
            app.settings.auto_install_dxvk_nvapi
        )
        .on_toggle(|value| Message::Settings(SettingsAction::AutoInstallDxvkNvapi(value))),
        row![
            text(format!(
                "DXVK-NVAPI 目录：{}",
                crate::widgets::path_text(nvapi_source)
            )),
            button("选择目录").on_press(Message::Pick(
                crate::dialogs::DialogIntent::PickDxvkNvapiDir
            )),
            button("清除").on_press_maybe(
                app.settings
                    .dxvk_nvapi_source
                    .is_some()
                    .then_some(Message::Settings(SettingsAction::DxvkNvapiSource(None)))
            ),
        ]
        .spacing(8),
    ]
    .spacing(8);

    if app.settings.auto_install_dxvk_nvapi && app.settings.dxvk_nvapi_source.is_none() {
        compatibility = compatibility
            .push(text("已启用自动安装，但未选择 DXVK-NVAPI 目录；Wine 游戏启动会失败").size(12));
    }

    compatibility = compatibility
        .push(
            checkbox("启用 Esync", app.settings.enable_esync)
                .on_toggle(|value| Message::Settings(SettingsAction::EnableEsync(value))),
        )
        .push(
            checkbox("启用 Fsync", app.settings.enable_fsync)
                .on_toggle(|value| Message::Settings(SettingsAction::EnableFsync(value))),
        )
        .push(
            checkbox("启用 MangoHud", app.settings.enable_mangohud)
                .on_toggle(|value| Message::Settings(SettingsAction::EnableMangoHud(value))),
        )
        .push(
            text(match &app.mangohud_path {
                Some(path) => format!("已检测到：{}", path.display()),
                None => "未检测到 mangohud，启用后启动会被拒绝".to_string(),
            })
            .size(12),
        );

    let compatibility = section("兼容层增强", compatibility);

    let content = column![
        header,
        default_runner,
        prefix_root,
        global_env,
        compatibility,
    ]
    .spacing(12)
    .padding(20);

    scrollable(content).height(Length::Fill).into()
}
