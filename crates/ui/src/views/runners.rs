use crate::app::{App, DeleteTarget, Message};
use crate::styles;
use crate::theme::font;
use crate::widgets::{Tone, badges, controls, layout, middle_truncate, truncate};
use application::Runner;
use iced::font::Weight;
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Length};
use launcher_core::RunnerKind;

pub fn view(app: &App) -> Element<'_, Message> {
    if app.pending.loading {
        return layout::centered_message("正在加载…");
    }

    let busy = app.pending.detecting_wine || app.pending.importing_proton;
    let header = layout::page_header(
        "Runner 管理",
        "探测系统 Wine，或导入自备的 Proton / GE-Proton 目录",
        row![
            controls::secondary("重新探测系统 Wine")
                .on_press_maybe((!busy).then_some(Message::DetectSystemWine)),
            controls::primary("导入 Proton 目录")
                .on_press_maybe((!busy).then_some(Message::ImportProton)),
        ]
        .spacing(8),
    );

    let body: Element<'_, Message> = if app.runners.is_empty() {
        if app.error.is_none() {
            layout::empty_state(
                "尚未检测到 Runner",
                "先探测系统 Wine，或导入 Proton 目录",
                Some(
                    row![
                        controls::secondary("探测系统 Wine")
                            .on_press_maybe((!busy).then_some(Message::DetectSystemWine)),
                        controls::primary("导入 Proton 目录")
                            .on_press_maybe((!busy).then_some(Message::ImportProton)),
                    ]
                    .spacing(8)
                    .into(),
                ),
            )
        } else {
            layout::centered_message("暂无可用的 Runner")
        }
    } else {
        let mut grid = column![].spacing(16).width(Length::Fill);
        for chunk in app.runners.chunks(2) {
            let mut line = row![]
                .spacing(16)
                .width(Length::Fill)
                .align_y(iced::Alignment::Start);
            for runner in chunk {
                line = line.push(runner_card(app, runner));
            }
            if chunk.len() == 1 {
                line = line.push(iced::widget::horizontal_space());
            }
            grid = grid.push(line);
        }
        grid.into()
    };

    column![
        header,
        scrollable(body)
            .style(styles::scrollable)
            .height(Length::Fill)
    ]
    .spacing(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn runner_card<'a>(app: &'a App, runner: &'a Runner) -> Element<'a, Message> {
    let is_default = app.settings.default_runner_id == Some(runner.id);
    let in_use = app
        .games
        .iter()
        .filter(|game| game.runner_id == Some(runner.id))
        .count();
    let can_delete = !is_default && in_use == 0 && !app.pending.deleting;
    let kind = match runner.kind {
        RunnerKind::Wine => "Wine",
        RunnerKind::Proton => "Proton",
    };

    let mut title = row![
        text(truncate(runner.name.as_str(), 14))
            .font(font(Weight::Bold))
            .size(15)
            .width(Length::Fill)
            .height(Length::Fixed(20.0))
            .wrapping(iced::widget::text::Wrapping::None),
        badges::badge(kind, Tone::Neutral),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    if is_default {
        title = title.push(badges::badge("默认", Tone::Info));
    }

    let verified = if runner.verified {
        badges::badge("已验证", Tone::Live)
    } else {
        badges::badge("未验证", Tone::Warning)
    };

    let version = row![
        layout::faint(truncate(
            runner.version_string.as_deref().unwrap_or("版本未知"),
            22
        ))
        .size(11)
        .width(Length::Fill)
        .height(Length::Fixed(14.0))
        .wrapping(iced::widget::text::Wrapping::None),
        verified,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let path = layout::faint(middle_truncate(
        &runner.binary_path.display().to_string(),
        48,
    ))
    .size(11)
    .width(Length::Fill)
    .height(Length::Fixed(14.0))
    .wrapping(iced::widget::text::Wrapping::None);

    let actions = row![
        controls::secondary_small("设为默认").on_press_maybe(
            (!is_default && !app.pending.settings_saving && !app.pending.deleting)
                .then_some(Message::SetDefaultRunner(Some(runner.id)))
        ),
        controls::danger_small("删除").on_press_maybe(
            can_delete.then_some(Message::ConfirmDelete(DeleteTarget::Runner(runner.id)))
        ),
    ]
    .spacing(8);

    let hint = if is_default {
        "默认 Runner，请先更换".to_owned()
    } else if in_use > 0 {
        format!("{in_use} 个游戏正在使用")
    } else {
        String::new()
    };

    let content = column![
        title,
        version,
        path,
        actions,
        layout::faint(hint)
            .size(10)
            .width(Length::Fill)
            .height(Length::Fixed(13.0))
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(8)
    .width(Length::Fill);

    layout::card(content)
}
