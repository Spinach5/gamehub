use crate::app::{App, Message, SettingsAction};
use crate::draft::EnvAction;
use crate::styles;
use crate::theme::{ThemeMode, font};
use crate::widgets::{Tone, badges, controls, layout, middle_truncate};
use iced::font::Weight;
use iced::widget::{column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    if app.pending.loading {
        return layout::centered_message("正在加载…");
    }

    let header = layout::page_header(
        "设置",
        "外观、默认兼容层与全局启动选项",
        controls::primary("保存")
            .on_press_maybe((!app.pending.settings_saving).then_some(Message::SaveSettings)),
    );

    let appearance = layout::section_card(
        "外观",
        Some("切换立即生效并保存".to_owned()),
        row![
            layout::muted("主题").size(13).width(Length::Fill),
            theme_segment(app.settings.theme),
        ]
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    );

    let default_runner = layout::section_card(
        "默认 Runner",
        Some("未单独指定 Runner 的游戏会使用它".to_owned()),
        pick_list(
            crate::widgets::runner_choices(&app.runners, "无默认（使用游戏级选择）"),
            Some(crate::widgets::runner_choice(
                &app.runners,
                "无默认（使用游戏级选择）",
                app.settings.default_runner_id,
            )),
            |choice| Message::Settings(SettingsAction::DefaultRunner(choice.value)),
        )
        .style(styles::pick_list)
        .menu_style(styles::menu),
    );

    let prefix_root = layout::section_card(
        "Prefix 根目录",
        Some("新建 Prefix 的默认父目录".to_owned()),
        row![
            layout::faint(middle_truncate(
                &app.settings.prefix_root.display().to_string(),
                56,
            ))
            .size(12)
            .width(Length::Fill)
            .height(Length::Fixed(16.0))
            .wrapping(iced::widget::text::Wrapping::None),
            controls::secondary_small("选择目录")
                .on_press(Message::Pick(crate::dialogs::DialogIntent::PickPrefixRoot)),
        ]
        .spacing(8)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    );

    column![
        header,
        scrollable(
            column![
                appearance,
                default_runner,
                prefix_root,
                global_env_card(app),
                compatibility_card(app),
            ]
            .spacing(16)
            .width(Length::Fill),
        )
        .style(styles::scrollable)
        .height(Length::Fill),
    ]
    .spacing(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn theme_segment<'a>(selected: ThemeMode) -> Element<'a, Message> {
    let item = |mode: ThemeMode| {
        let active = selected == mode;
        let style: fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style =
            if active {
                styles::segment_selected
            } else {
                styles::segment
            };

        iced::widget::button(text(mode.label()).size(12).font(font(if active {
            Weight::Semibold
        } else {
            Weight::Normal
        })))
        .padding([7, 12])
        .style(style)
        .on_press(Message::Settings(SettingsAction::Theme(mode)))
    };

    container(row![item(ThemeMode::Light), item(ThemeMode::Dark)].spacing(2))
        .padding(2)
        .style(styles::segment_track)
        .into()
}

fn global_env_card(app: &App) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);

    for (index, entry) in app.settings.global_env_rows.iter().enumerate() {
        rows =
            rows.push(
                row![
                    text_input("KEY", entry.key.as_str())
                        .on_input(move |key| Message::Settings(SettingsAction::Env(
                            EnvAction::Key { index, key }
                        )))
                        .style(styles::text_input)
                        .width(Length::FillPortion(2)),
                    text_input("VALUE", entry.value.as_str())
                        .on_input(move |value| Message::Settings(SettingsAction::Env(
                            EnvAction::Value { index, value }
                        )))
                        .style(styles::text_input)
                        .width(Length::FillPortion(3)),
                    controls::quiet_small("删除").on_press(Message::Settings(SettingsAction::Env(
                        EnvAction::Remove(index)
                    ))),
                ]
                .spacing(8)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center),
            );
    }

    rows = rows
        .push(
            controls::secondary_small("添加变量")
                .on_press(Message::Settings(SettingsAction::Env(EnvAction::Add))),
        )
        .push(layout::faint("WINEDLLOVERRIDES 请使用游戏级结构化 DLL Override，不要手填").size(11));

    layout::section_card("全局环境变量", None, rows)
}

fn compatibility_card(app: &App) -> Element<'_, Message> {
    let source = app
        .settings
        .dxvk_nvapi_source
        .as_deref()
        .map(|path| middle_truncate(&path.display().to_string(), 56))
        .unwrap_or_else(|| "未选择".to_owned());
    let mut content = column![
        controls::switch_row(
            "启用 Esync",
            "减少同步开销（部分旧游戏可能不兼容）",
            app.settings.enable_esync,
            |value| Message::Settings(SettingsAction::EnableEsync(value)),
        ),
        controls::switch_row(
            "启用 Fsync",
            "减少文件系统同步开销",
            app.settings.enable_fsync,
            |value| Message::Settings(SettingsAction::EnableFsync(value)),
        ),
        controls::switch_row(
            "启用 MangoHud",
            match &app.mangohud_path {
                Some(_) => "启动时叠加性能监控",
                None => "未检测到 mangohud，启用后启动会被拒绝",
            },
            app.settings.enable_mangohud,
            |value| Message::Settings(SettingsAction::EnableMangoHud(value)),
        ),
        controls::switch_row(
            "自动安装并更新 DXVK-NVAPI",
            "Wine 游戏启动前检查并更新",
            app.settings.auto_install_dxvk_nvapi,
            |value| Message::Settings(SettingsAction::AutoInstallDxvkNvapi(value)),
        ),
        row![
            layout::faint(format!("DXVK-NVAPI 目录：{source}"))
                .size(11)
                .width(Length::Fill)
                .height(Length::Fixed(14.0))
                .wrapping(iced::widget::text::Wrapping::None),
            controls::secondary_small("选择目录").on_press(Message::Pick(
                crate::dialogs::DialogIntent::PickDxvkNvapiDir
            )),
            controls::quiet_small("清除").on_press_maybe(
                app.settings
                    .dxvk_nvapi_source
                    .is_some()
                    .then_some(Message::Settings(SettingsAction::DxvkNvapiSource(None)))
            ),
        ]
        .spacing(8)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(8)
    .width(Length::Fill);

    if app.settings.auto_install_dxvk_nvapi && app.settings.dxvk_nvapi_source.is_none() {
        content = content.push(
            row![
                badges::badge("需要配置", Tone::Warning),
                layout::faint("已启用自动安装，但未选择 DXVK-NVAPI 目录；Wine 游戏启动会失败")
                    .size(11),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
    }

    if let Some(path) = &app.mangohud_path {
        content = content.push(
            layout::faint(format!(
                "已检测到 MangoHud：{}",
                middle_truncate(&path.display().to_string(), 56)
            ))
            .size(11)
            .width(Length::Fill)
            .height(Length::Fixed(14.0))
            .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    layout::section_card("兼容层增强", None, content)
}
