use crate::app::{App, Message, Screen};
use crate::theme::{font, tokens};
use crate::widgets::{Tone, badges, controls, cover, layout, truncate};
use application::Game;
use iced::font::Weight;
use iced::widget::{column, container, pick_list, row, scrollable, stack, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    if app.pending.loading {
        return layout::centered_message(app.tr("common.loading"));
    }

    let running_count = app
        .games
        .iter()
        .filter(|game| app.running.contains_key(&game.id))
        .count();
    let header = layout::page_header(
        app.tr("library.title"),
        app.tr_plural_args(
            "library.subtitle.one",
            "library.subtitle.other",
            app.games.len() as u64,
            &[
                ("games", &app.games.len().to_string()),
                ("running", &running_count.to_string()),
            ],
        ),
        row![
            controls::secondary(app.tr("nav.runners"))
                .on_press(Message::Navigate(Screen::RunnerManager)),
            controls::primary(app.tr("library.new_game")).on_press(Message::NewGame),
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
                app.tr("library.empty.title"),
                app.tr("library.empty.body"),
                Some(
                    controls::primary(app.tr("library.new_game"))
                        .on_press(Message::NewGame)
                        .into(),
                ),
            ));
        }
    } else {
        content = content.push(
            row![
                text(app.tr("library.all_games"))
                    .font(font(Weight::Bold))
                    .size(16),
                text(app.games.len().to_string())
                    .size(12)
                    .font(font(Weight::Normal))
                    .style(|theme| iced::widget::text::Style {
                        color: Some(tokens(theme).text_faint),
                    }),
                iced::widget::horizontal_space(),
                layout::faint(app.tr("library.sort.label")).size(11),
                pick_list(
                    crate::widgets::library_sort_choices(app.lang),
                    Some(crate::widgets::library_sort_choice(
                        app.lang,
                        app.library_sort
                    )),
                    |choice| Message::LibrarySortChanged(choice.value),
                )
                .font(font(Weight::Normal))
                .style(crate::styles::pick_list)
                .menu_style(crate::styles::menu),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
        content = content.push(grid(app));
    }

    let scrollbar_alpha = app.scrollbar_alpha();
    scrollable(content)
        .style(move |theme, status| crate::styles::scrollable(theme, status, scrollbar_alpha))
        .on_scroll(|_| Message::ScrollbarActivity)
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
                    crate::widgets::format_timestamp(app.lang, game.last_played_at)
                ))
                .size(11),
            ]
            .spacing(4)
            .width(Length::Fill),
            row![
                launch_or_stop(app, game, false),
                controls::secondary(app.tr("nav.logs")).on_press(Message::OpenLogs(game.id)),
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
    let (columns, card_width, cover_width, cover_height) =
        grid_metrics(app.window_width, app.games.len());

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
            layout::faint(crate::widgets::format_timestamp(
                app.lang,
                game.last_played_at
            ))
            .size(11)
            .width(Length::Fill)
            .height(Length::Fixed(13.0))
            .wrapping(iced::widget::text::Wrapping::None),
            layout::faint(app.tr_args(
                "library.playtime",
                &[(
                    "time",
                    &crate::widgets::format_playtime(app.lang, game.playtime_seconds),
                )],
            ))
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
                controls::secondary_small(app.tr("nav.settings"))
                    .on_press(Message::SelectGame(game.id)),
                controls::quiet_small(app.tr("nav.logs")).on_press(Message::OpenLogs(game.id)),
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
            controls::secondary_small(app.tr("launch.state.starting")).into()
        } else {
            controls::secondary(app.tr("launch.state.starting")).into()
        };
    }

    if app.running.contains_key(&game.id) {
        let stopping = app.stopping.contains(&game.id);
        let label = if stopping {
            app.tr("launch.state.stopping")
        } else {
            app.tr("launch.stop")
        };
        let press = (!stopping).then_some(Message::Stop(game.id));
        return if small {
            controls::danger_small(label).on_press_maybe(press).into()
        } else {
            controls::danger(label).on_press_maybe(press).into()
        };
    }

    let press = (!app.pending.saving && !app.pending.deleting).then_some(Message::Launch(game.id));
    if small {
        controls::primary_small(app.tr("launch.start"))
            .on_press_maybe(press)
            .into()
    } else {
        controls::primary(app.tr("launch.start"))
            .on_press_maybe(press)
            .into()
    }
}

fn runner_label(app: &App, game: &Game) -> String {
    if game.platform.is_native() {
        return app.tr("choices.platform.linux").to_owned();
    }
    match game.runner_id {
        None => app.tr("launch.default_runner").to_owned(),
        Some(id) => app
            .runners
            .iter()
            .find(|runner| runner.id == id)
            .map(|runner| runner.name.clone())
            .unwrap_or_else(|| app.tr("launch.runner_missing").to_owned()),
    }
}

const SIDEBAR_WIDTH: f32 = 200.0;
const CONTENT_PADDING: f32 = 48.0;
const SCROLLBAR_RESERVE: f32 = 12.0;
const CARD_GAP: f32 = 16.0;
const CARD_MAX_WIDTH: f32 = 240.0;
const CARD_STRETCH_MAX_WIDTH: f32 = 320.0;
const CARD_PADDING: f32 = 10.0;
const MIN_GRID_WIDTH: f32 = 360.0;

fn grid_content_width(window_width: f32) -> f32 {
    (window_width - SIDEBAR_WIDTH - CONTENT_PADDING - SCROLLBAR_RESERVE).max(MIN_GRID_WIDTH)
}

/// Picks the smallest column count that keeps every card at or below
/// `CARD_MAX_WIDTH`, then stretches the cards to fill the row. Wide windows
/// therefore gain extra columns instead of leaving a blank strip on the right.
/// When the library has fewer games than those columns, the cards grow further
/// (within `CARD_STRETCH_MAX_WIDTH`) so a short row still fills the width.
fn grid_metrics(window_width: f32, game_count: usize) -> (usize, f32, f32, f32) {
    let content_width = grid_content_width(window_width);
    let ideal_columns = ((content_width + CARD_GAP) / (CARD_MAX_WIDTH + CARD_GAP))
        .ceil()
        .max(1.0) as usize;
    let columns = ideal_columns.min(game_count.max(1));
    let fill_width = (content_width - CARD_GAP * (columns as f32 - 1.0)) / columns as f32;
    let card_width = fill_width.min(if columns < ideal_columns {
        CARD_STRETCH_MAX_WIDTH
    } else {
        CARD_MAX_WIDTH
    });
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
        assert_eq!(grid_metrics(900.0, 20).0, 3);
    }

    #[test]
    fn grid_uses_four_columns_at_default_window() {
        assert_eq!(grid_metrics(1180.0, 20).0, 4);
    }

    #[test]
    fn grid_uses_four_columns_at_design_threshold() {
        assert_eq!(grid_metrics(1100.0, 20).0, 4);
    }

    #[test]
    fn grid_adds_columns_on_wide_windows() {
        let mut previous = 0;
        for width in [900.0, 1100.0, 1180.0, 1600.0, 1920.0, 2560.0, 3440.0] {
            let columns = grid_metrics(width, 100).0;
            assert!(
                columns >= previous && columns > 0,
                "columns must grow with window width ({width})"
            );
            previous = columns;
        }
        assert!(grid_metrics(2560.0, 100).0 > grid_metrics(1180.0, 100).0);
        assert_eq!(grid_metrics(1920.0, 100).0, 7);
    }

    #[test]
    fn grid_keeps_cards_at_or_below_max_width() {
        for width in [900.0, 1100.0, 1180.0, 1920.0, 2560.0, 3440.0, 5120.0] {
            let (_, card_width, _, _) = grid_metrics(width, 100);
            assert!(
                card_width > 0.0 && card_width <= CARD_MAX_WIDTH,
                "card width {card_width} out of range at {width}"
            );
        }
    }

    #[test]
    fn grid_rows_fill_the_content_width() {
        for width in [
            900.0, 1100.0, 1180.0, 1600.0, 1920.0, 2000.0, 2560.0, 3440.0,
        ] {
            let (columns, card_width, _, _) = grid_metrics(width, 100);
            let content_width = grid_content_width(width);
            let used = card_width * columns as f32 + CARD_GAP * (columns as f32 - 1.0);
            assert!(
                (used - content_width).abs() < 0.01,
                "row uses {used}px of {content_width}px at {width}"
            );
        }
    }

    #[test]
    fn grid_stretches_cards_when_there_are_few_games() {
        let (columns, card_width, _, _) = grid_metrics(1920.0, 5);
        let content_width = grid_content_width(1920.0);
        let used = card_width * columns as f32 + CARD_GAP * (columns as f32 - 1.0);
        assert_eq!(columns, 5);
        assert!(card_width > CARD_MAX_WIDTH && card_width <= CARD_STRETCH_MAX_WIDTH);
        assert!((used - content_width).abs() < 0.01);
    }

    #[test]
    fn grid_caps_stretch_for_tiny_libraries() {
        let (_, card_width, _, _) = grid_metrics(1920.0, 2);
        assert_eq!(card_width, CARD_STRETCH_MAX_WIDTH);
    }

    #[test]
    fn truncate_only_adds_ellipsis_when_needed() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcde…");
    }

    #[test]
    fn library_subtitle_uses_plural_forms() {
        use crate::i18n::{EN_US, tr_plural_args};
        let one = tr_plural_args(
            &EN_US,
            "library.subtitle.one",
            "library.subtitle.other",
            1,
            &[("games", "1"), ("running", "1")],
        );
        let many = tr_plural_args(
            &EN_US,
            "library.subtitle.one",
            "library.subtitle.other",
            4,
            &[("games", "4"), ("running", "0")],
        );
        assert_eq!(one, "1 game · 1 running");
        assert_eq!(many, "4 games · 0 running");
    }
}
