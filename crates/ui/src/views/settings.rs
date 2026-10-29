use crate::app::{App, Message, SettingsAction};
use crate::styles;
use crate::theme::{ThemeMode, font};
use crate::widgets::{Choice, Tone, badges, controls, layout, middle_truncate};
use application::editor::EnvAction;
use iced::font::Weight;
use iced::widget::{column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    if app.pending.loading {
        return layout::centered_message(app.tr("common.loading"));
    }

    let header = layout::page_header(
        app.tr("settings.title"),
        app.tr("settings.subtitle"),
        controls::primary(app.tr("common.save"))
            .on_press_maybe((!app.pending.settings_saving).then_some(Message::SaveSettings)),
    );

    let appearance = layout::section_card(
        app.tr("settings.appearance.title"),
        Some(app.tr("settings.appearance.hint").to_owned()),
        column![
            row![
                layout::muted(app.tr("settings.theme"))
                    .size(13)
                    .width(Length::Fill),
                theme_segment(app.lang, app.settings.theme),
            ]
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
            row![
                layout::muted(app.tr("settings.language"))
                    .size(13)
                    .width(Length::Fill),
                pick_list(
                    language_choices(app.lang),
                    Some(language_choice(app.lang, app.settings.language)),
                    |choice| Message::Settings(SettingsAction::Language(choice.value)),
                )
                .style(styles::pick_list)
                .menu_style(styles::menu),
            ]
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(10)
        .width(Length::Fill),
    );

    let default_runner = layout::section_card(
        app.tr("settings.default_runner.title"),
        Some(app.tr("settings.default_runner.hint").to_owned()),
        pick_list(
            crate::widgets::runner_choices(&app.runners, app.tr("choices.runner.none")),
            Some(crate::widgets::runner_choice(
                &app.runners,
                app.tr("choices.runner.none"),
                app.settings.default_runner_id,
            )),
            |choice| Message::Settings(SettingsAction::DefaultRunner(choice.value)),
        )
        .style(styles::pick_list)
        .menu_style(styles::menu),
    );

    let prefix_root = layout::section_card(
        app.tr("settings.prefix_root.title"),
        Some(app.tr("settings.prefix_root.hint").to_owned()),
        row![
            layout::faint(middle_truncate(
                &app.settings.prefix_root.display().to_string(),
                56,
            ))
            .size(12)
            .width(Length::Fill)
            .height(Length::Fixed(16.0))
            .wrapping(iced::widget::text::Wrapping::None),
            controls::secondary_small(app.tr("settings.prefix_root.choose"))
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

fn theme_segment<'a>(lang: crate::i18n::Lang, selected: ThemeMode) -> Element<'a, Message> {
    let item = |mode: ThemeMode| {
        let active = selected == mode;
        let style: fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style =
            if active {
                styles::segment_selected
            } else {
                styles::segment
            };

        iced::widget::button(text(mode.label(lang)).size(12).font(font(if active {
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

fn language_choices(lang: crate::i18n::Lang) -> Vec<Choice<crate::i18n::LanguagePref>> {
    use crate::i18n::{LANGUAGES, LanguagePref};
    std::iter::once(LanguagePref::Auto)
        .chain(LANGUAGES.iter().map(LanguagePref::Fixed))
        .map(|pref| Choice::new(pref, language_label(lang, pref)))
        .collect()
}

fn language_choice(
    lang: crate::i18n::Lang,
    pref: crate::i18n::LanguagePref,
) -> Choice<crate::i18n::LanguagePref> {
    Choice::new(pref, language_label(lang, pref))
}

fn language_label(lang: crate::i18n::Lang, pref: crate::i18n::LanguagePref) -> String {
    match pref {
        crate::i18n::LanguagePref::Auto => {
            crate::i18n::tr(lang, "settings.language.auto").to_owned()
        }
        crate::i18n::LanguagePref::Fixed(language) => language.native_name.to_owned(),
    }
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
                    controls::quiet_small(app.tr("common.delete")).on_press(Message::Settings(
                        SettingsAction::Env(EnvAction::Remove(index))
                    )),
                ]
                .spacing(8)
                .width(Length::Fill)
                .align_y(iced::Alignment::Center),
            );
    }

    rows = rows
        .push(
            controls::secondary_small(app.tr("settings.global_env.add"))
                .on_press(Message::Settings(SettingsAction::Env(EnvAction::Add))),
        )
        .push(layout::faint(app.tr("settings.global_env.override_hint")).size(11));

    layout::section_card(app.tr("settings.global_env.title"), None, rows)
}

fn compatibility_card(app: &App) -> Element<'_, Message> {
    let source = app
        .settings
        .dxvk_nvapi_source
        .as_deref()
        .map(|path| middle_truncate(&path.display().to_string(), 56))
        .unwrap_or_else(|| app.tr("common.none").to_owned());
    let mut content = column![
        controls::switch_row(
            app.tr("settings.esync"),
            app.tr("settings.esync.hint"),
            app.settings.enable_esync,
            |value| Message::Settings(SettingsAction::EnableEsync(value)),
        ),
        controls::switch_row(
            app.tr("settings.fsync"),
            app.tr("settings.fsync.hint"),
            app.settings.enable_fsync,
            |value| Message::Settings(SettingsAction::EnableFsync(value)),
        ),
        controls::switch_row(
            app.tr("settings.mangohud"),
            match &app.mangohud_path {
                Some(_) => app.tr("settings.mangohud.hint"),
                None => app.tr("settings.mangohud.missing"),
            },
            app.settings.enable_mangohud,
            |value| Message::Settings(SettingsAction::EnableMangoHud(value)),
        ),
        controls::switch_row(
            app.tr("settings.dxvk_auto"),
            app.tr("settings.dxvk_auto.hint"),
            app.settings.auto_install_dxvk_nvapi,
            |value| Message::Settings(SettingsAction::AutoInstallDxvkNvapi(value)),
        ),
        row![
            layout::faint(app.tr_args("settings.dxvk_dir", &[("path", source.as_str())]))
                .size(11)
                .width(Length::Fill)
                .height(Length::Fixed(14.0))
                .wrapping(iced::widget::text::Wrapping::None),
            controls::secondary_small(app.tr("detail.choose_dxvk_dir")).on_press(Message::Pick(
                crate::dialogs::DialogIntent::PickDxvkNvapiDir
            )),
            controls::quiet_small(app.tr("common.clear")).on_press_maybe(
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
                badges::badge(app.tr("settings.dxvk_missing.badge"), Tone::Warning),
                layout::faint(app.tr("settings.dxvk_missing.hint")).size(11),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
    }

    if let Some(path) = &app.mangohud_path {
        let detected = middle_truncate(&path.display().to_string(), 56);
        content = content.push(
            layout::faint(
                app.tr_args("settings.mangohud.detected", &[("path", detected.as_str())]),
            )
            .size(11)
            .width(Length::Fill)
            .height(Length::Fixed(14.0))
            .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    layout::section_card(app.tr("settings.compat.title"), None, content)
}
