use crate::app::{App, DeleteTarget, Message, Screen};
use crate::dialogs::DialogIntent;
use crate::draft::{ArgsAction, DllAction, DraftAction, EnvAction};
use crate::widgets::{
    arch_choice, arch_choices, path_text, runner_choice, runner_choices, section,
};
use application::PrefixStatus;
use iced::widget::{button, checkbox, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};
use launcher_core::GamescopeMode;

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(draft) = &app.draft else {
        return column![
            button("返回").on_press(Message::Navigate(Screen::Library)),
            text("未选择游戏"),
        ]
        .spacing(12)
        .padding(20)
        .into();
    };

    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text(if draft.is_new {
            "新建游戏"
        } else {
            "游戏详情"
        })
        .size(24),
        text(if app.running.contains_key(&draft.id) {
            "运行中"
        } else {
            ""
        })
        .size(12),
        iced::widget::horizontal_space(),
        button("保存").on_press_maybe((!app.pending.saving).then_some(Message::SaveDraft)),
        button("启动").on_press_maybe(
            (!draft.is_new && !app.running.contains_key(&draft.id) && !app.pending.deleting)
                .then_some(Message::Launch(draft.id))
        ),
        button("删除游戏").on_press_maybe(
            (!draft.is_new && !app.running.contains_key(&draft.id) && !app.pending.deleting)
                .then_some(Message::ConfirmDelete(DeleteTarget::Game(draft.id)))
        ),
    ]
    .spacing(8);

    let exe_text = path_text(draft.executable_path.as_deref());
    let basic = column![
        text_input("游戏名称", draft.name.as_str())
            .on_input(|value| Message::Draft(DraftAction::Name(value)))
            .width(Length::Fill),
        row![
            text("可执行文件："),
            text(exe_text),
            button("选择").on_press(Message::Pick(DialogIntent::PickExe)),
        ]
        .spacing(8),
        row![
            text("工作目录："),
            text(path_text(draft.working_dir.as_deref())),
            button("选择").on_press(Message::Pick(DialogIntent::PickWorkingDir)),
            button("清除").on_press_maybe(
                draft
                    .working_dir
                    .is_some()
                    .then_some(Message::Draft(DraftAction::ClearWorkingDir))
            ),
        ]
        .spacing(8),
        row![
            text("架构："),
            pick_list(arch_choices(), Some(arch_choice(draft.arch)), |choice| {
                Message::Draft(DraftAction::Arch(choice.value))
            }),
        ]
        .spacing(8),
        text(format!(
            "最近游玩：{}",
            crate::widgets::format_timestamp(draft.last_played_at)
        ))
        .size(12),
    ]
    .spacing(10);

    let runner_section = {
        let none_label = "使用默认 Runner";
        let choices = runner_choices(&app.runners, none_label);
        let selected = runner_choice(&app.runners, none_label, draft.runner_id);

        let default_prefix = app.settings.prefix_root.join(draft.id.to_string());
        let prefix_path = draft.prefix_path.clone().unwrap_or(default_prefix);
        let status_text = prefix_status_label(app.prefix_status.as_ref());

        let mut buttons = row![
            button("选择 Prefix 目录").on_press(Message::Pick(DialogIntent::PickPrefixDir)),
            button("清除").on_press_maybe(
                draft
                    .prefix_path
                    .is_some()
                    .then_some(Message::Draft(DraftAction::ClearPrefix))
            ),
            button("创建 / 初始化").on_press_maybe(
                (!draft.is_new && !app.pending.prefix.contains(&draft.id))
                    .then_some(Message::CreatePrefix(draft.id))
            ),
            button("删除 Prefix").on_press_maybe(
                (!draft.is_new
                    && !app.pending.deleting
                    && matches!(
                        app.prefix_status,
                        Some(PrefixStatus::Initialized | PrefixStatus::ArchMismatch { .. })
                    ))
                .then_some(Message::ConfirmDelete(DeleteTarget::Prefix {
                    game_id: draft.id,
                    path: prefix_path.clone(),
                }))
            ),
        ]
        .spacing(8);

        if draft.is_new {
            buttons = buttons.push(text("保存游戏后才能创建 Prefix").size(12));
        }

        let dxvk_text = crate::widgets::path_text(app.dxvk_dir.as_deref());
        let mut content = column![
            row![
                text("Runner："),
                pick_list(choices, Some(selected), |choice| {
                    Message::Draft(DraftAction::Runner(choice.value))
                }),
            ]
            .spacing(8),
            text(format!("Prefix：{}", prefix_path.display())),
            text(format!("状态：{status_text}")),
            buttons,
            row![
                text(format!("DXVK 目录：{dxvk_text}")),
                button("选择目录").on_press(Message::Pick(DialogIntent::PickDxvkDir)),
                button("安装 DXVK").on_press_maybe(
                    (!draft.is_new
                        && app.dxvk_dir.is_some()
                        && !app.pending.dxvk.contains(&draft.id))
                    .then_some(Message::InstallDxvk(draft.id))
                ),
            ]
            .spacing(8),
        ]
        .spacing(10);

        if let Some(PrefixStatus::ArchMismatch { existing }) = &app.prefix_status {
            content = content.push(text(format!(
                "架构不匹配：现有 prefix 为 {existing:?}，请更换 Prefix 或删除后重建"
            )));
        }
        if app.prefix_status != Some(PrefixStatus::Initialized) {
            content = content.push(text("启动前必须有一个已初始化的 Prefix").size(12));
        }

        content
    };

    let base_display = draft.display.clone();
    let gamescope_effective = app.gamescope_path.is_some()
        && application::should_use_gamescope(
            &draft.display,
            app.session,
            app.gamescope_path.is_some(),
        );

    let backend_pick = pick_list(
        crate::widgets::backend_choices(),
        Some(crate::widgets::backend_choice(draft.display.backend)),
        {
            let base = base_display.clone();
            move |choice| {
                let mut config = base.clone();
                config.backend = choice.value;
                Message::Draft(DraftAction::Display(config))
            }
        },
    );

    let gamescope_pick = pick_list(
        crate::widgets::gamescope_choices(),
        Some(crate::widgets::gamescope_choice(draft.display.gamescope)),
        {
            let base = base_display.clone();
            move |choice| {
                let mut config = base.clone();
                config.gamescope = choice.value;
                Message::Draft(DraftAction::Display(config))
            }
        },
    );

    let fullscreen = {
        let base = base_display.clone();
        checkbox("全屏", draft.display.fullscreen).on_toggle(move |value| {
            let mut config = base.clone();
            config.fullscreen = value;
            Message::Draft(DraftAction::Display(config))
        })
    };

    let monitor: iced::Element<'_, Message> = if app.session == application::SessionType::X11 {
        let base = base_display.clone();
        pick_list(
            crate::widgets::monitor_choices(&app.x11_outputs),
            Some(crate::widgets::monitor_choice(
                &app.x11_outputs,
                draft.display.monitor.clone(),
            )),
            move |choice| {
                let mut config = base.clone();
                config.monitor = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .into()
    } else {
        let value = draft.display.monitor.clone().unwrap_or_default();
        let base = base_display.clone();
        text_input("输出名（可留空）", &value)
            .on_input(move |value| {
                let mut config = base.clone();
                config.monitor = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                Message::Draft(DraftAction::Display(config))
            })
            .into()
    };

    let resolution_controls = if !gamescope_effective {
        column![text(
            "当前不会使用 gamescope：分辨率与刷新率不生效；X11 只设置 __GL_SYNC_DISPLAY_DEVICE。"
        )
        .size(12)]
    } else {
        column![
            row![
                text("分辨率："),
                text_input("宽", draft.resolution_width.as_str())
                    .on_input(|value| Message::Draft(DraftAction::ResolutionWidth(value)))
                    .width(Length::Fixed(100.0)),
                text("x"),
                text_input("高", draft.resolution_height.as_str())
                    .on_input(|value| Message::Draft(DraftAction::ResolutionHeight(value)))
                    .width(Length::Fixed(100.0)),
            ]
            .spacing(8),
            row![
                text("刷新率："),
                text_input("例如 144", draft.refresh_rate.as_str())
                    .on_input(|value| Message::Draft(DraftAction::RefreshRate(value)))
                    .width(Length::Fixed(120.0)),
            ]
            .spacing(8),
        ]
        .spacing(8)
    };

    let mut display_section = column![
        row![text("后端："), backend_pick].spacing(8),
        text("后端字段仅作记录；实际 X11/Wayland 以启动时的会话检测为准。").size(12),
        row![text("gamescope："), gamescope_pick].spacing(8),
        row![text("显示器："), monitor].spacing(8),
        fullscreen,
        resolution_controls,
    ]
    .spacing(8);

    if app.gamescope_path.is_none() {
        let hint = match draft.display.gamescope {
            GamescopeMode::Forced => Some("强制 gamescope 但未检测到 gamescope，启动会被拒绝"),
            GamescopeMode::Auto => {
                Some("未检测到 gamescope，自动模式不会生效，分辨率/刷新率不会应用")
            }
            GamescopeMode::Off => None,
        };
        if let Some(hint) = hint {
            display_section = display_section.push(text(hint).size(12));
        }
    }

    let mut env_rows = column![].spacing(8);
    for (index, entry) in draft.env_rows.iter().enumerate() {
        env_rows = env_rows.push(
            row![
                text_input("KEY", entry.key.as_str())
                    .on_input(move |key| Message::Draft(DraftAction::Env(EnvAction::Key {
                        index,
                        key
                    })))
                    .width(Length::FillPortion(2)),
                text_input("VALUE", entry.value.as_str())
                    .on_input(
                        move |value| Message::Draft(DraftAction::Env(EnvAction::Value {
                            index,
                            value
                        }))
                    )
                    .width(Length::FillPortion(3)),
                button("删除").on_press(Message::Draft(DraftAction::Env(EnvAction::Remove(index)))),
            ]
            .spacing(8),
        );
    }
    env_rows = env_rows
        .push(button("添加环境变量").on_press(Message::Draft(DraftAction::Env(EnvAction::Add))));

    let env_section = column![
        env_rows,
        text("WINEDLLOVERRIDES 请使用下方的结构化 DLL Override，不要手填。").size(12),
    ]
    .spacing(8);

    let mut dll_rows = column![].spacing(8);
    for (index, entry) in draft.dll_rows.iter().enumerate() {
        dll_rows = dll_rows.push(
            row![
                text_input("DLL 名称（不含 .dll）", entry.name.as_str())
                    .on_input(
                        move |name| Message::Draft(DraftAction::Dll(DllAction::Name {
                            index,
                            name
                        }))
                    )
                    .width(Length::FillPortion(2)),
                pick_list(
                    crate::widgets::override_mode_choices(),
                    Some(crate::widgets::override_mode_choice(entry.mode)),
                    move |choice| Message::Draft(DraftAction::Dll(DllAction::Mode {
                        index,
                        mode: choice.value,
                    })),
                ),
                button("删除").on_press(Message::Draft(DraftAction::Dll(DllAction::Remove(index)))),
            ]
            .spacing(8),
        );
    }
    dll_rows = dll_rows.push(
        button("添加 DLL Override").on_press(Message::Draft(DraftAction::Dll(DllAction::Add))),
    );

    let mut args_rows = column![].spacing(8);
    for (index, argument) in draft.args_rows.iter().enumerate() {
        args_rows =
            args_rows.push(
                row![
                    text_input("参数", argument.as_str())
                        .on_input(move |value| Message::Draft(DraftAction::Args(
                            ArgsAction::Value { index, value }
                        )))
                        .width(Length::Fill),
                    button("删除")
                        .on_press(Message::Draft(DraftAction::Args(ArgsAction::Remove(index)))),
                ]
                .spacing(8),
            );
    }
    args_rows = args_rows
        .push(button("添加参数").on_press(Message::Draft(DraftAction::Args(ArgsAction::Add))));

    let mut content = column![header].spacing(12).padding(20);
    if draft.is_new {
        content = content.push(text("保存游戏后才能启动、创建 Prefix 或安装 DXVK").size(12));
    }
    content = content
        .push(section("基本信息", basic))
        .push(section("兼容层与 Prefix", runner_section))
        .push(section("显示", display_section))
        .push(section("环境变量", env_section))
        .push(section("DLL Override", dll_rows))
        .push(section("启动参数", args_rows));

    scrollable(content).height(Length::Fill).into()
}

fn prefix_status_label(status: Option<&PrefixStatus>) -> &'static str {
    match status {
        None => "未知（未解析 Runner 或未加载）",
        Some(PrefixStatus::Missing) => "未创建",
        Some(PrefixStatus::NotInitialized) => "目录存在，未初始化",
        Some(PrefixStatus::Initialized) => "已初始化",
        Some(PrefixStatus::ArchMismatch { .. }) => "架构不匹配",
    }
}
