use crate::app::{App, DeleteTarget, Message, Screen};
use crate::dialogs::DialogIntent;
use crate::draft::DraftAction;
use crate::widgets::{
    arch_choice, arch_choices, path_text, runner_choice, runner_choices, section,
};
use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};
use prefix::PrefixStatus;

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
        iced::widget::horizontal_space(),
        button("保存").on_press_maybe((!app.pending.saving).then_some(Message::SaveDraft)),
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

        content
    };

    let content = column![
        header,
        section("基本信息", basic),
        section("兼容层与 Prefix", runner_section),
    ]
    .spacing(12)
    .padding(20);

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
