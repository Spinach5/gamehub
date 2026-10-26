use crate::app::{App, Message, Screen};
use crate::dialogs::DialogIntent;
use crate::draft::DraftAction;
use crate::widgets::{arch_choice, arch_choices, path_text, section};
use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

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

    let content = column![header, section("基本信息", basic),]
        .spacing(12)
        .padding(20);

    scrollable(content).height(Length::Fill).into()
}
