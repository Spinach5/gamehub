use crate::app::{App, DeleteTarget, Message, Screen};
use crate::dialogs::DialogIntent;
use crate::styles;
use crate::theme::font;
use crate::widgets::{Tone, badges, controls, cover, layout, path_text};
use application::editor::{ArgsAction, DllAction, DraftAction, EnvAction, GameDraft};
use application::{GamescopeMode, PrefixStatus};
use iced::font::Weight;
use iced::widget::{
    column, container, horizontal_space, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length};
use uuid::Uuid;

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(draft) = &app.draft else {
        return column![
            controls::secondary(app.tr("common.back")).on_press(Message::Navigate(Screen::Library)),
            layout::empty_state(
                app.tr("detail.empty.title"),
                app.tr("detail.empty.body"),
                None
            ),
        ]
        .spacing(16)
        .into();
    };

    let launch_state = app.launch_state_label(draft.id);
    let subtitle = if draft.is_new {
        app.tr("detail.subtitle.unsaved").to_owned()
    } else {
        let time = crate::widgets::format_timestamp(app.lang, draft.last_played_at);
        let env = draft.env_rows.len().to_string();
        let dll = draft.dll_rows.len().to_string();
        app.tr_args(
            "detail.subtitle.saved",
            &[("time", &time), ("env", &env), ("dll", &dll)],
        )
    };
    let header = row![
        controls::secondary(app.tr("common.back")).on_press(Message::Navigate(Screen::Library)),
        column![
            text(if draft.is_new {
                app.tr("detail.new_title").to_owned()
            } else {
                draft.name.clone()
            })
            .font(font(Weight::Bold))
            .size(22)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            layout::muted(subtitle).size(11),
        ]
        .spacing(2)
        .width(Length::Fill),
        controls::danger(app.tr("detail.delete_game")).on_press_maybe(
            (!draft.is_new
                && launch_state.is_none()
                && !app.pending.deleting
                && !app.pending.saving)
                .then_some(Message::ConfirmDelete(DeleteTarget::Game(draft.id))),
        ),
        controls::primary(app.tr("common.save")).on_press_maybe(
            (!app.pending.saving && !app.pending.deleting).then_some(Message::SaveDraft)
        ),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let left = column![
        basic_card(app, draft),
        runner_card(app, draft),
        display_card(app, draft),
        env_card(app, draft),
        dll_card(app, draft),
        args_card(app, draft),
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

fn basic_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let content = column![
        layout::field(
            app.tr("detail.name"),
            text_input(app.tr("detail.name.placeholder"), draft.name.as_str())
                .on_input(|value| Message::Draft(DraftAction::Name(value)))
                .style(styles::text_input)
                .width(Length::Fill)
        ),
        layout::field(
            app.tr("detail.executable"),
            row![
                layout::faint(path_text(app.lang, draft.executable_path.as_deref()))
                    .size(11)
                    .width(Length::Fill),
                controls::secondary_small(app.tr("common.choose"))
                    .on_press(Message::Pick(DialogIntent::PickExe)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        layout::field(
            app.tr("detail.working_dir"),
            row![
                layout::faint(path_text(app.lang, draft.working_dir.as_deref()))
                    .size(11)
                    .width(Length::Fill),
                controls::secondary_small(app.tr("common.choose"))
                    .on_press(Message::Pick(DialogIntent::PickWorkingDir)),
                controls::quiet_small(app.tr("common.clear")).on_press_maybe(
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
            app.tr("detail.arch"),
            pick_list(
                crate::widgets::arch_choices(app.lang),
                Some(crate::widgets::arch_choice(app.lang, draft.arch)),
                |choice| Message::Draft(DraftAction::Arch(choice.value)),
            )
            .style(styles::pick_list)
            .menu_style(styles::menu)
        ),
    ]
    .spacing(10)
    .width(Length::Fill);

    layout::section_card(app.tr("detail.section.basic"), None, content)
}

fn runner_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let none_label = app.tr("choices.runner.default");
    let choices = crate::widgets::runner_choices(&app.runners, none_label);
    let selected = crate::widgets::runner_choice(&app.runners, none_label, draft.runner_id);
    let default_prefix = app.settings.prefix_root.join(draft.id.to_string());
    let prefix_path = draft.prefix_path.clone().unwrap_or(default_prefix);
    let status = app.prefix_status.as_ref();
    let dxvk_text = path_text(app.lang, app.dxvk_dir.as_deref());
    let dxvk_label = app.tr_args("detail.dxvk_dir", &[("path", dxvk_text.as_str())]);

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
            app.tr("detail.status"),
            row![
                badges::badge(prefix_status_label(status, app.lang), prefix_tone(status)),
                layout::faint(dxvk_label).size(11).width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        row![
            controls::secondary_small(app.tr("detail.choose_prefix_dir"))
                .on_press(Message::Pick(DialogIntent::PickPrefixDir)),
            controls::quiet_small(app.tr("common.clear")).on_press_maybe(
                draft
                    .prefix_path
                    .is_some()
                    .then_some(Message::Draft(DraftAction::ClearPrefix))
            ),
            controls::danger_small(app.tr("detail.delete_prefix")).on_press_maybe(
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
                app.tr("detail.creating")
            } else {
                app.tr("detail.create_prefix")
            })
            .on_press_maybe(
                (!draft.is_new && !app.pending.prefix.contains(&draft.id))
                    .then_some(Message::CreatePrefix(draft.id))
            ),
        ]
        .spacing(8),
        row![
            controls::secondary_small(app.tr("detail.choose_dxvk_dir"))
                .on_press(Message::Pick(DialogIntent::PickDxvkDir)),
            controls::primary_small(if app.pending.dxvk.contains(&draft.id) {
                app.tr("detail.installing")
            } else {
                app.tr("detail.install_dxvk")
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
        content = content.push(layout::faint(app.tr("detail.save_first_compat")).size(11));
    }
    if let Some(PrefixStatus::ArchMismatch { existing }) = &app.prefix_status {
        let existing = existing
            .map(|arch| crate::widgets::arch_choice(app.lang, arch).label)
            .unwrap_or_else(|| app.tr("common.unknown").to_owned());
        let hint = app.tr_args("detail.arch_mismatch", &[("existing", existing.as_str())]);
        content = content.push(layout::faint(hint).size(11));
    }
    if app.prefix_status != Some(PrefixStatus::Initialized) {
        content = content.push(layout::faint(app.tr("detail.prefix_required")).size(11));
    }

    layout::section_card(app.tr("detail.section.compat"), None, content)
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
            crate::widgets::backend_choices(app.lang),
            Some(crate::widgets::backend_choice(
                app.lang,
                draft.display.backend,
            )),
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
            crate::widgets::gamescope_choices(app.lang),
            Some(crate::widgets::gamescope_choice(
                app.lang,
                draft.display.gamescope,
            )),
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
            crate::widgets::monitor_choices(app.lang, &app.monitors),
            Some(crate::widgets::monitor_choice(
                app.lang,
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
            app.tr("detail.fullscreen"),
            app.tr("detail.fullscreen.hint"),
            draft.display.fullscreen,
            move |value| {
                let mut config = base.clone();
                config.fullscreen = value;
                Message::Draft(DraftAction::Display(config))
            },
        )
    };

    let mut content = column![
        layout::field(app.tr("detail.backend"), backend_pick),
        layout::faint(app.tr("detail.backend.hint")).size(11),
        layout::field("gamescope", gamescope_pick),
        layout::field(
            app.tr("detail.monitor"),
            row![
                monitor_pick,
                controls::secondary_small(app.tr("common.refresh"))
                    .on_press(Message::RefreshMonitors),
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
                    layout::muted(app.tr("detail.resolution"))
                        .size(12)
                        .width(Length::Fixed(72.0)),
                    text_input(app.tr("detail.width"), draft.resolution_width.as_str())
                        .on_input(|value| Message::Draft(DraftAction::ResolutionWidth(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(90.0)),
                    layout::muted("×").size(12),
                    text_input(app.tr("detail.height"), draft.resolution_height.as_str())
                        .on_input(|value| Message::Draft(DraftAction::ResolutionHeight(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(90.0)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                row![
                    layout::muted(app.tr("detail.refresh_rate"))
                        .size(12)
                        .width(Length::Fixed(72.0)),
                    text_input(
                        app.tr("detail.refresh_rate.placeholder"),
                        draft.refresh_rate.as_str()
                    )
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
        content = content.push(layout::faint(app.tr("detail.gamescope.off_hint")).size(11));
    }

    if !app.monitors.iter().any(|output| output.connected) {
        content = content.push(layout::faint(app.tr("detail.monitors.none")).size(11));
    }

    if app.session == application::SessionType::Wayland {
        if gamescope_effective {
            content = content.push(layout::faint(app.tr("detail.gamescope.monitor_hint")).size(11));
        } else if draft.display.monitor.is_some() {
            let hint = if std::env::var_os("DISPLAY").is_some() {
                app.tr("detail.monitor.x11_hint")
            } else {
                app.tr("detail.monitor.wayland_hint")
            };
            content = content.push(layout::faint(hint).size(11));
        }
    }

    if app.gamescope_path.is_none() {
        if let Some(hint) = match draft.display.gamescope {
            GamescopeMode::Forced => Some(app.tr("detail.gamescope.unavailable_forced")),
            GamescopeMode::Auto => Some(app.tr("detail.gamescope.unavailable_auto")),
            GamescopeMode::Off => None,
        } {
            content = content.push(layout::faint(hint).size(11));
        }
    }

    layout::section_card(app.tr("detail.section.display"), None, content)
}

fn env_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
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
                controls::quiet_small(app.tr("common.delete"))
                    .on_press(Message::Draft(DraftAction::Env(EnvAction::Remove(index)))),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }
    rows = rows.push(
        controls::secondary_small(app.tr("detail.add_env"))
            .on_press(Message::Draft(DraftAction::Env(EnvAction::Add))),
    );
    rows = rows.push(layout::faint(app.tr("detail.env.override_hint")).size(11));

    layout::section_card(app.tr("detail.section.env"), None, rows)
}

fn dll_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, entry) in draft.dll_rows.iter().enumerate() {
        rows = rows.push(
            row![
                text_input(app.tr("detail.dll.name_placeholder"), entry.name.as_str())
                    .on_input(
                        move |name| Message::Draft(DraftAction::Dll(DllAction::Name {
                            index,
                            name
                        }))
                    )
                    .style(styles::text_input)
                    .width(Length::FillPortion(2)),
                pick_list(
                    crate::widgets::override_mode_choices(app.lang),
                    Some(crate::widgets::override_mode_choice(app.lang, entry.mode)),
                    move |choice| Message::Draft(DraftAction::Dll(DllAction::Mode {
                        index,
                        mode: choice.value,
                    })),
                )
                .style(styles::pick_list)
                .menu_style(styles::menu),
                controls::quiet_small(app.tr("common.delete"))
                    .on_press(Message::Draft(DraftAction::Dll(DllAction::Remove(index)))),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }
    rows = rows.push(
        controls::secondary_small(app.tr("detail.add_dll"))
            .on_press(Message::Draft(DraftAction::Dll(DllAction::Add))),
    );

    layout::section_card(app.tr("detail.section.dll"), None, rows)
}

fn args_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, argument) in draft.args_rows.iter().enumerate() {
        rows =
            rows.push(
                row![
                    text_input(app.tr("detail.arg.placeholder"), argument.as_str())
                        .on_input(move |value| Message::Draft(DraftAction::Args(
                            ArgsAction::Value { index, value }
                        )))
                        .style(styles::text_input)
                        .width(Length::Fill),
                    controls::quiet_small(app.tr("common.delete"))
                        .on_press(Message::Draft(DraftAction::Args(ArgsAction::Remove(index)))),
                ]
                .spacing(8)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center),
            );
    }
    rows = rows.push(
        controls::secondary_small(app.tr("detail.add_arg"))
            .on_press(Message::Draft(DraftAction::Args(ArgsAction::Add))),
    );

    layout::section_card(app.tr("detail.section.args"), None, rows)
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
        controls::secondary(app.tr("launch.state.starting")).into()
    } else if app.running.contains_key(&draft.id) {
        let stopping = app.stopping.contains(&draft.id);
        controls::danger(if stopping {
            app.tr("launch.state.stopping")
        } else {
            app.tr("launch.stop_game")
        })
        .on_press_maybe((!stopping).then_some(Message::Stop(draft.id)))
        .into()
    } else {
        controls::primary(app.tr("launch.start_game"))
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
        .unwrap_or_else(|| app.tr("launch.default_runner").to_owned());

    let status = app.prefix_status.as_ref();
    let dxvk_source = if app.dxvk_dir.is_some() {
        app.tr("detail.selected")
    } else {
        app.tr("common.none")
    };
    let facts = column![
        fact("Prefix", prefix_status_label(status, app.lang).to_owned()),
        fact(app.tr("detail.fact.dxvk_source"), dxvk_source.to_owned()),
        fact(
            app.tr("detail.fact.recently_played"),
            crate::widgets::format_timestamp(app.lang, draft.last_played_at),
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
                crate::widgets::arch_choice(app.lang, draft.arch).label
            ))
            .size(11),
        )
        .push(launch);
    if draft.is_new {
        details = details.push(layout::faint(app.tr("detail.save_first")).size(11));
    }
    details = details
        .push(
            row![
                controls::secondary_small(app.tr("detail.cover.choose"))
                    .on_press(Message::Pick(DialogIntent::PickCover)),
                controls::secondary_small(app.tr("detail.cover.restore"))
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

fn prefix_status_label(status: Option<&PrefixStatus>, lang: crate::i18n::Lang) -> &'static str {
    match status {
        None => crate::i18n::tr(lang, "detail.prefix_status.unknown"),
        Some(PrefixStatus::Missing) => crate::i18n::tr(lang, "detail.prefix_status.missing"),
        Some(PrefixStatus::NotInitialized) => {
            crate::i18n::tr(lang, "detail.prefix_status.not_initialized")
        }
        Some(PrefixStatus::Initialized) => {
            crate::i18n::tr(lang, "detail.prefix_status.initialized")
        }
        Some(PrefixStatus::ArchMismatch { .. }) => {
            crate::i18n::tr(lang, "detail.prefix_status.arch_mismatch")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use application::Arch;

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
            prefix_status_label(Some(&PrefixStatus::Initialized), &crate::i18n::ZH_CN),
            "已初始化"
        );
        assert_eq!(
            prefix_status_label(None, &crate::i18n::ZH_CN),
            "未知（未解析 Runner 或未加载）"
        );
        assert_eq!(
            prefix_status_label(Some(&PrefixStatus::Missing), &crate::i18n::ZH_CN),
            "未创建"
        );
        assert_eq!(
            prefix_status_label(Some(&PrefixStatus::NotInitialized), &crate::i18n::ZH_CN),
            "目录存在，未初始化"
        );
        assert_eq!(
            prefix_status_label(
                Some(&PrefixStatus::ArchMismatch {
                    existing: Some(Arch::Win64)
                }),
                &crate::i18n::ZH_CN
            ),
            "架构不匹配"
        );
        assert_eq!(
            prefix_status_label(Some(&PrefixStatus::Initialized), &crate::i18n::EN_US),
            "Initialized"
        );
        assert_eq!(
            prefix_status_label(Some(&PrefixStatus::NotInitialized), &crate::i18n::EN_US),
            "Folder exists, not initialized"
        );
        assert_eq!(
            prefix_status_label(None, &crate::i18n::EN_US),
            "Unknown (runner not resolved or not loaded)"
        );
    }
}
