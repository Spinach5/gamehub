use crate::app::{App, Message, Screen};
use crate::theme::{font, tokens};
use crate::widgets::{Tone, badges, controls, cover, layout, truncate};
use application::Game;
use iced::font::Weight;
use iced::widget::{column, container, row, scrollable, stack, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    if app.pending.loading {
        return layout::centered_message("正在加载…");
    }

    let running_count = app
        .games
        .iter()
        .filter(|game| app.running.contains_key(&game.id))
        .count();
    let header = layout::page_header(
        "游戏库",
        format!("{} 个游戏 · {} 个正在运行", app.games.len(), running_count),
        row![
            controls::secondary("Runner 管理").on_press(Message::Navigate(Screen::RunnerManager)),
            controls::primary("＋ 新建游戏").on_press(Message::NewGame),
        ]
        .spacing(8),
    );

    let mut content = column![header].spacing(20).width(Length::Fill);

    if let Some(game) = app
        .games
        .iter()
        .find(|game| app.launch_state_label(game.id).is_some())
    {
        content = content.push(hero(app, game));
    }

    if app.games.is_empty() {
        if app.error.is_none() {
            content = content.push(layout::empty_state(
                "还没有游戏",
                "点击下方按钮选择一个 .exe 开始",
                Some(
                    controls::primary("＋ 新建游戏")
                        .on_press(Message::NewGame)
                        .into(),
                ),
            ));
        }
    } else {
        content = content.push(
            row![
                text("全部游戏").font(font(Weight::Bold)).size(16),
                text(app.games.len().to_string()).size(12).style(|theme| {
                    iced::widget::text::Style {
                        color: Some(tokens(theme).text_faint),
                    }
                }),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
        content = content.push(grid(app));
    }

    scrollable(content)
        .style(crate::styles::scrollable)
        .height(Length::Fill)
        .into()
}

fn hero<'a>(app: &'a App, game: &'a Game) -> Element<'a, Message> {
    let handle = app.cover_handles.get(&game.id).cloned();
    let state = app.launch_state_label(game.id).unwrap_or_default();
    let tone = status_tone(
        app.starting.contains(&game.id),
        app.stopping.contains(&game.id),
    );

    container(
        row![
            cover(game.name.as_str(), game.id, handle, 56.0, 76.0),
            column![
                badges::badge(state, tone),
                text(game.name.clone())
                    .font(font(Weight::Bold))
                    .size(18)
                    .width(Length::Fill)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
                layout::muted(format!(
                    "{} · {}",
                    runner_label(app, game),
                    crate::widgets::format_timestamp(game.last_played_at)
                ))
                .size(11),
            ]
            .spacing(4)
            .width(Length::Fill),
            row![
                launch_or_stop(app, game, false),
                controls::secondary("日志").on_press(Message::OpenLogs(game.id)),
            ]
            .spacing(8),
        ]
        .spacing(14)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding(16)
    .width(Length::Fill)
    .style(crate::styles::card)
    .into()
}

fn grid(app: &App) -> Element<'_, Message> {
    let (columns, card_width, cover_width, cover_height) = grid_metrics(app.window_width);

    let mut rows = column![].spacing(CARD_GAP);
    for chunk in app.games.chunks(columns) {
        let mut line = row![].spacing(CARD_GAP).align_y(iced::Alignment::Start);
        for game in chunk {
            line = line.push(game_card(app, game, card_width, cover_width, cover_height));
        }
        rows = rows.push(line);
    }
    rows.into()
}

fn game_card<'a>(
    app: &'a App,
    game: &'a Game,
    card_width: f32,
    cover_width: f32,
    cover_height: f32,
) -> Element<'a, Message> {
    let handle = app.cover_handles.get(&game.id).cloned();
    let mut cover_stack = stack![cover(
        game.name.as_str(),
        game.id,
        handle,
        cover_width,
        cover_height
    )];

    if let Some(label) = app.launch_state_label(game.id) {
        let tone = status_tone(
            app.starting.contains(&game.id),
            app.stopping.contains(&game.id),
        );
        cover_stack = cover_stack.push(container(badges::badge(label, tone)).padding(8));
    }

    container(
        column![
            cover_stack,
            text(truncate(game.name.as_str(), 24))
                .font(font(Weight::Semibold))
                .size(14)
                .width(Length::Fill)
                .height(Length::Fixed(38.0))
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            layout::faint(crate::widgets::format_timestamp(game.last_played_at))
                .size(11)
                .width(Length::Fill)
                .height(Length::Fixed(13.0))
                .wrapping(iced::widget::text::Wrapping::None),
            layout::faint(truncate(&runner_label(app, game), 18))
                .size(11)
                .width(Length::Fill)
                .height(Length::Fixed(13.0))
                .wrapping(iced::widget::text::Wrapping::None),
            row![
                launch_or_stop(app, game, true),
                controls::secondary_small("设置").on_press(Message::SelectGame(game.id)),
                controls::quiet_small("日志").on_press(Message::OpenLogs(game.id)),
            ]
            .spacing(6),
        ]
        .spacing(8),
    )
    .padding(10)
    .width(Length::Fixed(card_width))
    .style(crate::styles::card)
    .into()
}

fn launch_or_stop<'a>(app: &'a App, game: &'a Game, small: bool) -> Element<'a, Message> {
    if app.starting.contains(&game.id) {
        return if small {
            controls::secondary_small("启动中").into()
        } else {
            controls::secondary("启动中").into()
        };
    }

    if app.running.contains_key(&game.id) {
        let stopping = app.stopping.contains(&game.id);
        let label = if stopping { "结束中" } else { "结束" };
        let press = (!stopping).then_some(Message::Stop(game.id));
        return if small {
            controls::danger_small(label).on_press_maybe(press).into()
        } else {
            controls::danger(label).on_press_maybe(press).into()
        };
    }

    let press = (!app.pending.saving && !app.pending.deleting).then_some(Message::Launch(game.id));
    if small {
        controls::primary_small("启动").on_press_maybe(press).into()
    } else {
        controls::primary("启动").on_press_maybe(press).into()
    }
}

fn runner_label(app: &App, game: &Game) -> String {
    match game.runner_id {
        None => "默认 Runner".to_owned(),
        Some(id) => app
            .runners
            .iter()
            .find(|runner| runner.id == id)
            .map(|runner| runner.name.clone())
            .unwrap_or_else(|| "Runner 缺失".to_owned()),
    }
}

const SIDEBAR_WIDTH: f32 = 200.0;
const CONTENT_PADDING: f32 = 48.0;
const CARD_GAP: f32 = 16.0;
const CARD_MAX_WIDTH: f32 = 240.0;
const CARD_PADDING: f32 = 10.0;
const MIN_GRID_WIDTH: f32 = 360.0;
const FOUR_COLUMN_MIN: f32 = 840.0;

fn grid_metrics(window_width: f32) -> (usize, f32, f32, f32) {
    let content_width = (window_width - SIDEBAR_WIDTH - CONTENT_PADDING).max(MIN_GRID_WIDTH);
    let columns = if content_width >= FOUR_COLUMN_MIN {
        4
    } else {
        3
    };
    let card_width =
        ((content_width - CARD_GAP * (columns as f32 - 1.0)) / columns as f32).min(CARD_MAX_WIDTH);
    let cover_width = card_width - CARD_PADDING * 2.0;
    let cover_height = cover_width * 4.0 / 3.0;
    (columns, card_width, cover_width, cover_height)
}

fn status_tone(starting: bool, stopping: bool) -> Tone {
    if starting {
        Tone::Starting
    } else if stopping {
        Tone::Warning
    } else {
        Tone::Live
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_uses_three_columns_at_min_window() {
        assert_eq!(grid_metrics(900.0).0, 3);
    }

    #[test]
    fn grid_uses_four_columns_at_default_window() {
        assert_eq!(grid_metrics(1180.0).0, 4);
    }

    #[test]
    fn grid_uses_four_columns_at_design_threshold() {
        assert_eq!(grid_metrics(1100.0).0, 4);
    }

    #[test]
    fn grid_caps_card_width_on_wide_windows() {
        let (_, card_width, _, _) = grid_metrics(2000.0);
        assert_eq!(card_width, CARD_MAX_WIDTH);
    }

    #[test]
    fn truncate_only_adds_ellipsis_when_needed() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcde…");
    }
}
