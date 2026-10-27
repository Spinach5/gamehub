use crate::app::{App, DeleteTarget, Message, Screen};
use crate::dialogs::DialogIntent;
use crate::draft::{ArgsAction, DllAction, DraftAction, EnvAction, GameDraft};
use crate::styles;
use crate::theme::font;
use crate::widgets::{Tone, badges, controls, cover, layout, path_text};
use application::PrefixStatus;
use iced::font::Weight;
use iced::widget::{
    column, container, horizontal_space, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length};
use launcher_core::GamescopeMode;
use uuid::Uuid;

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(draft) = &app.draft else {
        return column![
            controls::secondary("返回").on_press(Message::Navigate(Screen::Library)),
            layout::empty_state("未选择游戏", "请从游戏库选择一个游戏", None),
        ]
        .spacing(16)
        .into();
    };

    let launch_state = app.launch_state_label(draft.id);
    let header = row![
        controls::secondary("返回").on_press(Message::Navigate(Screen::Library)),
        column![
            text(if draft.is_new {
                "新建游戏".to_owned()
            } else {
                draft.name.clone()
            })
            .font(font(Weight::Bold))
            .size(22)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            layout::muted(if draft.is_new {
                "保存后可启动、创建 Prefix 或安装 DXVK".to_owned()
            } else {
                format!(
                    "最近游玩：{} · {} 个环境变量 · {} 个 DLL Override",
                    crate::widgets::format_timestamp(draft.last_played_at),
                    draft.env_rows.len(),
                    draft.dll_rows.len()
                )
            })
            .size(11),
        ]
        .spacing(2)
        .width(Length::Fill),
        controls::danger("删除游戏").on_press_maybe(
            (!draft.is_new
                && launch_state.is_none()
                && !app.pending.deleting
                && !app.pending.saving)
                .then_some(Message::ConfirmDelete(DeleteTarget::Game(draft.id))),
        ),
        controls::primary("保存").on_press_maybe(
            (!app.pending.saving && !app.pending.deleting).then_some(Message::SaveDraft)
        ),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let left = column![
        basic_card(draft),
        runner_card(app, draft),
        display_card(app, draft),
        env_card(draft),
        dll_card(draft),
        args_card(draft),
    ]
    .spacing(16)
    .width(Length::Fill);

    let body = row![
        container(
            scrollable(left)
                .style(styles::scrollable)
                .height(Length::Fill)
        )
        .width(Length::Fill)
        .height(Length::Fill),
        summary_card(app, draft),
    ]
    .spacing(24)
    .width(Length::Fill)
    .height(Length::Fill);

    column![header, body]
        .spacing(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn basic_card(draft: &GameDraft) -> Element<'_, Message> {
    let content = column![
        layout::field(
            "名称",
            text_input("游戏名称", draft.name.as_str())
                .on_input(|value| Message::Draft(DraftAction::Name(value)))
                .style(styles::text_input)
                .width(Length::Fill)
        ),
        layout::field(
            "可执行文件",
            row![
                layout::faint(path_text(draft.executable_path.as_deref()))
                    .size(11)
                    .width(Length::Fill),
                controls::secondary_small("选择").on_press(Message::Pick(DialogIntent::PickExe)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        layout::field(
            "工作目录",
            row![
                layout::faint(path_text(draft.working_dir.as_deref()))
                    .size(11)
                    .width(Length::Fill),
                controls::secondary_small("选择")
                    .on_press(Message::Pick(DialogIntent::PickWorkingDir)),
                controls::quiet_small("清除").on_press_maybe(
                    draft
                        .working_dir
                        .is_some()
                        .then_some(Message::Draft(DraftAction::ClearWorkingDir))
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        layout::field(
            "架构",
            pick_list(
                crate::widgets::arch_choices(),
                Some(crate::widgets::arch_choice(draft.arch)),
                |choice| Message::Draft(DraftAction::Arch(choice.value)),
            )
            .style(styles::pick_list)
            .menu_style(styles::menu)
        ),
    ]
    .spacing(10)
    .width(Length::Fill);

    layout::section_card("基本信息", None, content)
}

fn runner_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let none_label = "使用默认 Runner";
    let choices = crate::widgets::runner_choices(&app.runners, none_label);
    let selected = crate::widgets::runner_choice(&app.runners, none_label, draft.runner_id);
    let default_prefix = app.settings.prefix_root.join(draft.id.to_string());
    let prefix_path = draft.prefix_path.clone().unwrap_or(default_prefix);
    let status = app.prefix_status.as_ref();
    let dxvk_text = path_text(app.dxvk_dir.as_deref());

    let mut content = column![
        layout::field(
            "Runner",
            pick_list(choices, Some(selected), |choice| {
                Message::Draft(DraftAction::Runner(choice.value))
            })
            .style(styles::pick_list)
            .menu_style(styles::menu)
        ),
        layout::field(
            "Prefix",
            layout::faint(prefix_path.display().to_string())
                .size(11)
                .width(Length::Fill)
        ),
        layout::field(
            "状态",
            row![
                badges::badge(prefix_status_label(status), prefix_tone(status)),
                layout::faint(format!("DXVK 目录：{dxvk_text}"))
                    .size(11)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        row![
            controls::secondary_small("选择 Prefix 目录")
                .on_press(Message::Pick(DialogIntent::PickPrefixDir)),
            controls::quiet_small("清除").on_press_maybe(
                draft
                    .prefix_path
                    .is_some()
                    .then_some(Message::Draft(DraftAction::ClearPrefix))
            ),
            controls::danger_small("删除 Prefix").on_press_maybe(
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
        .spacing(8),
        row![
            controls::secondary_small(if app.pending.prefix.contains(&draft.id) {
                "创建中…"
            } else {
                "创建 / 初始化"
            })
            .on_press_maybe(
                (!draft.is_new && !app.pending.prefix.contains(&draft.id))
                    .then_some(Message::CreatePrefix(draft.id))
            ),
        ]
        .spacing(8),
        row![
            controls::secondary_small("选择 DXVK 目录")
                .on_press(Message::Pick(DialogIntent::PickDxvkDir)),
            controls::primary_small(if app.pending.dxvk.contains(&draft.id) {
                "安装中…"
            } else {
                "安装 DXVK"
            })
            .on_press_maybe(
                (!draft.is_new && app.dxvk_dir.is_some() && !app.pending.dxvk.contains(&draft.id))
                    .then_some(Message::InstallDxvk(draft.id))
            ),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .width(Length::Fill);

    if draft.is_new {
        content = content.push(layout::faint("保存游戏后才能创建 Prefix 或安装 DXVK").size(11));
    }
    if let Some(PrefixStatus::ArchMismatch { existing }) = &app.prefix_status {
        let existing = existing
            .map(|arch| crate::widgets::arch_choice(arch).label)
            .unwrap_or_else(|| "未知".to_owned());
        content = content.push(
            layout::faint(format!(
                "架构不匹配：现有 prefix 为 {existing}，请更换 Prefix 或删除后重建"
            ))
            .size(11),
        );
    }
    if app.prefix_status != Some(PrefixStatus::Initialized) {
        content = content.push(layout::faint("启动前必须有一个已初始化的 Prefix").size(11));
    }

    layout::section_card("兼容层与 Prefix", None, content)
}

fn display_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let base = draft.display.clone();
    let gamescope_effective = app.gamescope_path.is_some()
        && application::should_use_gamescope(
            &draft.display,
            app.session,
            app.gamescope_path.is_some(),
        );

    let backend_pick = {
        let base = base.clone();
        pick_list(
            crate::widgets::backend_choices(),
            Some(crate::widgets::backend_choice(draft.display.backend)),
            move |choice| {
                let mut config = base.clone();
                config.backend = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
    };

    let gamescope_pick = {
        let base = base.clone();
        pick_list(
            crate::widgets::gamescope_choices(),
            Some(crate::widgets::gamescope_choice(draft.display.gamescope)),
            move |choice| {
                let mut config = base.clone();
                config.gamescope = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
    };

    let monitor_pick = {
        let base = base.clone();
        pick_list(
            crate::widgets::monitor_choices(&app.monitors),
            Some(crate::widgets::monitor_choice(
                &app.monitors,
                draft.display.monitor.clone(),
            )),
            move |choice| {
                let mut config = base.clone();
                config.monitor = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
        .width(Length::Fill)
    };

    let fullscreen = {
        let base = base.clone();
        controls::switch_row(
            "全屏",
            "以全屏模式启动游戏",
            draft.display.fullscreen,
            move |value| {
                let mut config = base.clone();
                config.fullscreen = value;
                Message::Draft(DraftAction::Display(config))
            },
        )
    };

    let mut content = column![
        layout::field("后端", backend_pick),
        layout::faint("后端字段仅作记录；实际 X11/Wayland 以启动时的会话检测为准").size(11),
        layout::field("gamescope", gamescope_pick),
        layout::field(
            "显示器",
            row![
                monitor_pick,
                controls::secondary_small("刷新").on_press(Message::RefreshMonitors),
            ]
            .spacing(8)
        ),
        fullscreen,
    ]
    .spacing(10)
    .width(Length::Fill);

    if gamescope_effective {
        content = content.push(
            column![
                row![
                    layout::muted("分辨率").size(12).width(Length::Fixed(72.0)),
                    text_input("宽", draft.resolution_width.as_str())
                        .on_input(|value| Message::Draft(DraftAction::ResolutionWidth(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(90.0)),
                    layout::muted("×").size(12),
                    text_input("高", draft.resolution_height.as_str())
                        .on_input(|value| Message::Draft(DraftAction::ResolutionHeight(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(90.0)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                row![
                    layout::muted("刷新率").size(12).width(Length::Fixed(72.0)),
                    text_input("例如 144", draft.refresh_rate.as_str())
                        .on_input(|value| Message::Draft(DraftAction::RefreshRate(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(100.0)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(8),
        );
    } else {
        content = content.push(
            layout::faint(
                "当前不会使用 gamescope：分辨率与刷新率不生效；选择显示器时会临时切换系统 primary",
            )
            .size(11),
        );
    }

    if !app.monitors.iter().any(|output| output.connected) {
        content = content.push(layout::faint("未检测到已连接显示器，点击“刷新”重新扫描").size(11));
    }

    if app.session == application::SessionType::Wayland {
        if gamescope_effective {
            content = content
                .push(layout::faint("gamescope 生效时，显示器通过 gamescope 参数指定").size(11));
        } else if draft.display.monitor.is_some() {
            let hint = if std::env::var_os("DISPLAY").is_some() {
                "启动时会临时把所选显示器设为系统 primary（X11/XWayland），启动约 5 秒后恢复原 primary。"
            } else {
                "未检测到 XWayland（DISPLAY），将改用 Wine/Proton 的 Wayland 显示器变量；Wine Wayland 的全屏窗口位置无法保证。"
            };
            content = content.push(layout::faint(hint).size(11));
        }
    }

    if app.gamescope_path.is_none() {
        if let Some(hint) = match draft.display.gamescope {
            GamescopeMode::Forced => Some("强制 gamescope 但未检测到 gamescope，启动会被拒绝"),
            GamescopeMode::Auto => {
                Some("未检测到 gamescope，自动模式不会生效，分辨率/刷新率不会应用")
            }
            GamescopeMode::Off => None,
        } {
            content = content.push(layout::faint(hint).size(11));
        }
    }

    layout::section_card("显示", None, content)
}

fn env_card(draft: &GameDraft) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, entry) in draft.env_rows.iter().enumerate() {
        rows = rows.push(
            row![
                text_input("KEY", entry.key.as_str())
                    .on_input(move |key| Message::Draft(DraftAction::Env(EnvAction::Key {
                        index,
                        key
                    })))
                    .style(styles::text_input)
                    .width(Length::FillPortion(2)),
                text_input("VALUE", entry.value.as_str())
                    .on_input(
                        move |value| Message::Draft(DraftAction::Env(EnvAction::Value {
                            index,
                            value
                        }))
                    )
                    .style(styles::text_input)
                    .width(Length::FillPortion(3)),
                controls::quiet_small("删除")
                    .on_press(Message::Draft(DraftAction::Env(EnvAction::Remove(index)))),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }
    rows = rows.push(
        controls::secondary_small("添加环境变量")
            .on_press(Message::Draft(DraftAction::Env(EnvAction::Add))),
    );
    rows = rows
        .push(layout::faint("WINEDLLOVERRIDES 请使用下方的结构化 DLL Override，不要手填").size(11));

    layout::section_card("环境变量", None, rows)
}

fn dll_card(draft: &GameDraft) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, entry) in draft.dll_rows.iter().enumerate() {
        rows = rows.push(
            row![
                text_input("DLL 名称（不含 .dll）", entry.name.as_str())
                    .on_input(
                        move |name| Message::Draft(DraftAction::Dll(DllAction::Name {
                            index,
                            name
                        }))
                    )
                    .style(styles::text_input)
                    .width(Length::FillPortion(2)),
                pick_list(
                    crate::widgets::override_mode_choices(),
                    Some(crate::widgets::override_mode_choice(entry.mode)),
                    move |choice| Message::Draft(DraftAction::Dll(DllAction::Mode {
                        index,
                        mode: choice.value,
                    })),
                )
                .style(styles::pick_list)
                .menu_style(styles::menu),
                controls::quiet_small("删除")
                    .on_press(Message::Draft(DraftAction::Dll(DllAction::Remove(index)))),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }
    rows = rows.push(
        controls::secondary_small("添加 DLL Override")
            .on_press(Message::Draft(DraftAction::Dll(DllAction::Add))),
    );

    layout::section_card("DLL Override", None, rows)
}

fn args_card(draft: &GameDraft) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, argument) in draft.args_rows.iter().enumerate() {
        rows =
            rows.push(
                row![
                    text_input("参数", argument.as_str())
                        .on_input(move |value| Message::Draft(DraftAction::Args(
                            ArgsAction::Value { index, value }
                        )))
                        .style(styles::text_input)
                        .width(Length::Fill),
                    controls::quiet_small("删除")
                        .on_press(Message::Draft(DraftAction::Args(ArgsAction::Remove(index)))),
                ]
                .spacing(8)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center),
            );
    }
    rows = rows.push(
        controls::secondary_small("添加参数")
            .on_press(Message::Draft(DraftAction::Args(ArgsAction::Add))),
    );

    layout::section_card("启动参数", None, rows)
}

fn summary_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let handle = if let Some(source) = draft.cover_source.as_deref() {
        source
            .is_file()
            .then(|| iced::widget::image::Handle::from_path(source.to_path_buf()))
    } else if draft.cover_path.is_some() {
        app.cover_handles.get(&draft.id).cloned().or_else(|| {
            draft
                .cover_path
                .as_deref()
                .filter(|path| path.is_file())
                .map(|path| iced::widget::image::Handle::from_path(path.to_path_buf()))
        })
    } else {
        None
    };
    let has_preview = draft.cover_source.is_some() || draft.cover_path.is_some();

    let launch: Element<'a, Message> = if app.starting.contains(&draft.id) {
        controls::secondary("启动中").into()
    } else if app.running.contains_key(&draft.id) {
        let stopping = app.stopping.contains(&draft.id);
        controls::danger(if stopping {
            "结束中"
        } else {
            "结束游戏"
        })
        .on_press_maybe((!stopping).then_some(Message::Stop(draft.id)))
        .into()
    } else {
        controls::primary("启动游戏")
            .on_press_maybe(
                (!draft.is_new && !app.pending.deleting && !app.pending.saving)
                    .then_some(Message::Launch(draft.id)),
            )
            .into()
    };

    let runner = draft
        .runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.clone())
        .unwrap_or_else(|| "默认 Runner".to_owned());

    let status = app.prefix_status.as_ref();
    let dxvk_source = if app.dxvk_dir.is_some() {
        "已选择"
    } else {
        "未选择"
    };
    let facts = column![
        fact("Prefix", prefix_status_label(status).to_owned()),
        fact("DXVK 源", dxvk_source.to_owned()),
        fact(
            "最近游玩",
            crate::widgets::format_timestamp(draft.last_played_at),
        ),
    ]
    .spacing(6)
    .width(Length::Fill);

    let mut details = column![
        cover(draft.name.as_str(), draft.id, handle, 148.0, 197.0),
        text(draft.name.as_str())
            .font(font(Weight::Bold))
            .size(16)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
    ];
    if let Some(label) = app.launch_state_label(draft.id) {
        details = details.push(badges::badge(label, launch_tone(app, draft.id)));
    }
    details = details
        .push(
            layout::muted(format!(
                "{} · {}",
                runner,
                crate::widgets::arch_choice(draft.arch).label
            ))
            .size(11),
        )
        .push(launch);
    if draft.is_new {
        details = details.push(layout::faint("保存后才能启动").size(11));
    }
    details = details
        .push(
            row![
                controls::secondary_small("选择封面")
                    .on_press(Message::Pick(DialogIntent::PickCover)),
                controls::secondary_small("恢复默认封面")
                    .on_press_maybe(has_preview.then_some(Message::Draft(DraftAction::ClearCover))),
            ]
            .spacing(8),
        )
        .push(facts);

    container(
        scrollable(
            details
                .spacing(10)
                .width(Length::Fill)
                .align_x(iced::Alignment::Center),
        )
        .style(styles::scrollable)
        .height(Length::Fill),
    )
    .padding(16)
    .width(Length::Fixed(260.0))
    .height(Length::Fill)
    .style(styles::card)
    .into()
}

fn launch_tone(app: &App, game_id: Uuid) -> Tone {
    if app.starting.contains(&game_id) {
        Tone::Starting
    } else if app.stopping.contains(&game_id) {
        Tone::Warning
    } else {
        Tone::Live
    }
}

fn fact(label: &'static str, value: String) -> Element<'static, Message> {
    row![
        layout::muted(label).size(11),
        horizontal_space(),
        text(value).size(11).font(font(Weight::Semibold)),
    ]
    .width(Length::Fill)
    .into()
}

fn prefix_tone(status: Option<&PrefixStatus>) -> Tone {
    match status {
        Some(PrefixStatus::Initialized) => Tone::Live,
        Some(PrefixStatus::ArchMismatch { .. }) => Tone::Danger,
        Some(PrefixStatus::Missing | PrefixStatus::NotInitialized) => Tone::Starting,
        None => Tone::Neutral,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::Arch;

    #[test]
    fn prefix_status_maps_to_tones() {
        assert_eq!(prefix_tone(Some(&PrefixStatus::Initialized)), Tone::Live);
        assert_eq!(
            prefix_tone(Some(&PrefixStatus::ArchMismatch {
                existing: Some(Arch::Win32)
            })),
            Tone::Danger
        );
        assert_eq!(prefix_tone(Some(&PrefixStatus::Missing)), Tone::Starting);
        assert_eq!(
            prefix_tone(Some(&PrefixStatus::NotInitialized)),
            Tone::Starting
        );
        assert_eq!(prefix_tone(None), Tone::Neutral);
    }

    #[test]
    fn prefix_labels_are_present() {
        assert_eq!(
            prefix_status_label(Some(&PrefixStatus::Initialized)),
            "已初始化"
        );
        assert_eq!(prefix_status_label(None), "未知（未解析 Runner 或未加载）");
        assert_eq!(prefix_status_label(Some(&PrefixStatus::Missing)), "未创建");
        assert_eq!(
            prefix_status_label(Some(&PrefixStatus::NotInitialized)),
            "目录存在，未初始化"
        );
        assert_eq!(
            prefix_status_label(Some(&PrefixStatus::ArchMismatch {
                existing: Some(Arch::Win64)
            })),
            "架构不匹配"
        );
    }
}
