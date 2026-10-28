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
        return layout::centered_message(app.tr("common.loading"));
    }

    let busy = app.pending.detecting_wine || app.pending.importing_proton;
    let header = layout::page_header(
        app.tr("runners.title"),
        app.tr("runners.subtitle"),
        row![
            controls::secondary(app.tr("runners.detect"))
                .on_press_maybe((!busy).then_some(Message::DetectSystemWine)),
            controls::primary(app.tr("runners.import"))
                .on_press_maybe((!busy).then_some(Message::ImportProton)),
        ]
        .spacing(8),
    );

    let body: Element<'_, Message> = if app.runners.is_empty() {
        if app.error.is_none() {
            layout::empty_state(
                app.tr("runners.empty.title"),
                app.tr("runners.empty.body"),
                Some(
                    row![
                        controls::secondary(app.tr("runners.detect_short"))
                            .on_press_maybe((!busy).then_some(Message::DetectSystemWine)),
                        controls::primary(app.tr("runners.import"))
                            .on_press_maybe((!busy).then_some(Message::ImportProton)),
                    ]
                    .spacing(8)
                    .into(),
                ),
            )
        } else {
            layout::centered_message(app.tr("runners.empty.none"))
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
        title = title.push(badges::badge(app.tr("runners.badge.default"), Tone::Info));
    }

    let verified = if runner.verified {
        badges::badge(app.tr("runners.badge.verified"), Tone::Live)
    } else {
        badges::badge(app.tr("runners.badge.unverified"), Tone::Warning)
    };

    let version = row![
        layout::faint(truncate(
            runner
                .version_string
                .as_deref()
                .unwrap_or(app.tr("runners.version_unknown")),
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
        controls::secondary_small(app.tr("runners.set_default")).on_press_maybe(
            (!is_default && !app.pending.settings_saving && !app.pending.deleting)
                .then_some(Message::SetDefaultRunner(Some(runner.id)))
        ),
        controls::danger_small(app.tr("common.delete")).on_press_maybe(
            can_delete.then_some(Message::ConfirmDelete(DeleteTarget::Runner(runner.id)))
        ),
    ]
    .spacing(8);

    let hint = if is_default {
        app.tr("runners.default_in_use").to_owned()
    } else if in_use > 0 {
        let count = in_use.to_string();
        app.tr_plural_args(
            "runners.in_use.one",
            "runners.in_use.other",
            in_use as u64,
            &[("count", &count)],
        )
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
