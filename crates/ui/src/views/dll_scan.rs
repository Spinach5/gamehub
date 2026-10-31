use crate::app::{DLL_SCAN_RENDER_LIMIT, DllScanDialog, DllScanPhase, Message};
use crate::styles;
use crate::theme::font;
use crate::widgets::{Tone, badge, controls, layout, override_mode_choice, override_mode_choices};
use iced::font::Weight;
use iced::widget::{checkbox, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

pub fn view<'a>(lang: crate::i18n::Lang, dialog: &'a DllScanDialog) -> Element<'a, Message> {
    let header = row![
        text(crate::i18n::tr(lang, "detail.dll_scan.title"))
            .font(font(Weight::Bold))
            .size(18),
        iced::widget::horizontal_space(),
        controls::quiet(crate::i18n::tr(lang, "common.close"))
            .on_press_maybe((!dialog.applying).then_some(Message::CloseDllScan)),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let directory = match dialog.root.as_deref() {
        Some(root) => row![
            text(crate::i18n::tr_args(
                lang,
                "detail.dll_scan.scan_dir",
                &[("path", &root.display().to_string())],
            ))
            .size(11)
            .font(font(Weight::Normal))
            .width(Length::Fill),
            controls::secondary_small(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                .on_press_maybe((!dialog.applying).then_some(Message::DllScanPickDir)),
            controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.rescan"))
                .on_press_maybe((!dialog.applying).then_some(Message::DllScanRescan)),
        ],
        None => row![
            layout::faint(crate::i18n::tr(lang, "detail.dll_scan.no_dir")).size(11),
            iced::widget::horizontal_space(),
            controls::secondary_small(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                .on_press(Message::DllScanPickDir),
        ],
    }
    .spacing(8)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let body: Element<'a, Message> = match &dialog.phase {
        DllScanPhase::NeedDirectory => column![
            layout::muted(crate::i18n::tr(lang, "detail.dll_scan.need_dir")).size(13),
            row![
                controls::secondary(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                    .on_press(Message::DllScanPickDir),
                controls::quiet(crate::i18n::tr(lang, "common.close"))
                    .on_press(Message::CloseDllScan),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into(),
        DllScanPhase::Scanning => column![
            layout::muted(crate::i18n::tr(lang, "detail.dll_scan.scanning")).size(13),
            controls::secondary(crate::i18n::tr(lang, "detail.dll_scan.cancel_scan"))
                .on_press(Message::DllScanCancelScan),
        ]
        .spacing(12)
        .into(),
        DllScanPhase::Failed(error) => column![
            text(crate::i18n::errors::dll_scan_error(lang, error))
                .size(13)
                .font(font(Weight::Normal))
                .style(|theme| iced::widget::text::Style {
                    color: Some(styles::tone_colors(theme, Tone::Danger).1),
                })
                .width(Length::Fill),
            row![
                controls::secondary(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                    .on_press(Message::DllScanPickDir),
                controls::quiet(crate::i18n::tr(lang, "detail.dll_scan.rescan"))
                    .on_press(Message::DllScanRescan),
                controls::quiet(crate::i18n::tr(lang, "common.close"))
                    .on_press(Message::CloseDllScan),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into(),
        DllScanPhase::Ready(outcome) => ready_body(lang, dialog, outcome),
    };

    // Only the result phase has a variable-length body; it stretches so the
    // scrollable list yields space instead of pushing the footer out of the
    // window when warnings are present.
    let ready = matches!(dialog.phase, DllScanPhase::Ready(_));
    let mut content = column![header, directory, body]
        .spacing(12)
        .width(Length::Fill);
    if ready {
        content = content.height(Length::Fill);
    }
    let card = container(content)
        .padding(22)
        .max_width(780)
        .max_height(640)
        .style(styles::modal_card);
    let card = if ready {
        card.height(Length::Fill)
    } else {
        card
    };

    iced::widget::opaque(
        container(card)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(styles::backdrop),
    )
}

fn ready_body<'a>(
    lang: crate::i18n::Lang,
    dialog: &'a DllScanDialog,
    outcome: &'a application::DllScanOutcome,
) -> Element<'a, Message> {
    let filtered = dialog.filtered();
    let mut summary = row![
        layout::muted(format_count(
            lang,
            "detail.dll_scan.found.one",
            "detail.dll_scan.found.other",
            outcome.entries.len(),
        ))
        .size(11),
        layout::muted(format_count(
            lang,
            "detail.dll_scan.selected.one",
            "detail.dll_scan.selected.other",
            dialog.selected.len(),
        ))
        .size(11),
    ]
    .spacing(14)
    .width(Length::Fill);
    if outcome.skipped_non_utf8 > 0 {
        summary = summary.push(
            layout::faint(format_count(
                lang,
                "detail.dll_scan.skipped_non_utf8.one",
                "detail.dll_scan.skipped_non_utf8.other",
                outcome.skipped_non_utf8,
            ))
            .size(11),
        );
    }

    let mut warnings = column![].spacing(4).width(Length::Fill);
    if outcome.truncated {
        warnings = warnings.push(warning_text(
            lang,
            crate::i18n::tr(lang, "detail.dll_scan.truncated").to_owned(),
        ));
    }
    if outcome.depth_limited {
        warnings = warnings.push(warning_text(
            lang,
            crate::i18n::tr_args(
                lang,
                "detail.dll_scan.depth_limited",
                &[("depth", &application::DLL_SCAN_MAX_DEPTH.to_string())],
            ),
        ));
    }
    if outcome.unreadable_dirs > 0 {
        warnings = warnings.push(warning_text(
            lang,
            format_count(
                lang,
                "detail.dll_scan.unreadable_dirs.one",
                "detail.dll_scan.unreadable_dirs.other",
                outcome.unreadable_dirs,
            ),
        ));
    }

    let toolbar = row![
        text_input(
            crate::i18n::tr(lang, "detail.dll_scan.search_placeholder"),
            dialog.query.as_str()
        )
        .on_input(Message::DllScanQueryChanged)
        .style(styles::text_input)
        .font(font(Weight::Normal))
        .width(Length::Fill),
        controls::quiet_small(if dialog.sort_desc {
            crate::i18n::tr(lang, "detail.dll_scan.sort_desc")
        } else {
            crate::i18n::tr(lang, "detail.dll_scan.sort_asc")
        })
        .on_press_maybe((!dialog.applying).then_some(Message::DllScanSortToggled)),
        controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.select_all"))
            .on_press_maybe((!dialog.applying).then_some(Message::DllScanSelectAll)),
        controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.invert"))
            .on_press_maybe((!dialog.applying).then_some(Message::DllScanInvert)),
        controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.clear"))
            .on_press_maybe((!dialog.applying).then_some(Message::DllScanClear)),
    ]
    .spacing(8)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let mode = row![
        layout::faint(crate::i18n::tr(lang, "detail.dll_scan.mode")).size(11),
        pick_list(
            override_mode_choices(lang),
            Some(override_mode_choice(lang, dialog.mode)),
            |choice: crate::widgets::Choice<application::OverrideMode>| {
                Message::DllScanModeChanged(choice.value)
            },
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
        .font(font(Weight::Normal)),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let mut list = column![].spacing(2).width(Length::Fill);
    if outcome.entries.is_empty() {
        list = list.push(
            container(layout::muted(crate::i18n::tr(lang, "detail.dll_scan.empty")).size(12))
                .padding(20)
                .width(Length::Fill)
                .center_x(Length::Fill),
        );
    } else if filtered.is_empty() {
        list = list.push(
            container(layout::muted(crate::i18n::tr(lang, "detail.dll_scan.no_matches")).size(12))
                .padding(20)
                .width(Length::Fill)
                .center_x(Length::Fill),
        );
    } else {
        for entry in &filtered {
            let existing = dialog.existing.contains(&entry.key);
            let checked = existing || dialog.selected.contains(&entry.key);
            let key = entry.key.clone();
            let toggle = if existing || dialog.applying {
                None
            } else {
                Some(move |_value: bool| Message::DllScanToggle(key.clone()))
            };
            let name: Element<'a, Message> = if existing {
                layout::faint(entry.name.as_str()).size(13).into()
            } else {
                text(entry.name.as_str())
                    .size(13)
                    .font(font(Weight::Normal))
                    .into()
            };
            let mut line = row![
                checkbox("", checked)
                    .on_toggle_maybe(toggle)
                    .style(styles::checkbox)
                    .size(16.0),
                column![
                    name,
                    layout::faint(entry.relative_path.display().to_string()).size(10),
                ]
                .spacing(1)
                .width(Length::Fill),
            ]
            .spacing(10)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center);
            if existing {
                line = line.push(badge(
                    crate::i18n::tr(lang, "detail.dll_scan.added_badge"),
                    Tone::Info,
                ));
            }
            list = list.push(line);
        }
    }

    let render_limited =
        filtered.len() >= DLL_SCAN_RENDER_LIMIT && outcome.entries.len() > filtered.len();

    let mut footer = row![
        layout::muted(format_count(
            lang,
            "detail.dll_scan.selected.one",
            "detail.dll_scan.selected.other",
            dialog.selected.len(),
        ))
        .size(11),
        iced::widget::horizontal_space(),
        controls::secondary(crate::i18n::tr(lang, "common.cancel"))
            .on_press_maybe((!dialog.applying).then_some(Message::CloseDllScan)),
    ]
    .spacing(8)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);
    let confirm = if dialog.applying {
        controls::primary(crate::i18n::tr(lang, "detail.dll_scan.applying"))
    } else {
        controls::primary(crate::i18n::tr(lang, "detail.dll_scan.confirm"))
            .on_press_maybe((!dialog.selected.is_empty()).then_some(Message::DllScanConfirm))
    };
    footer = footer.push(confirm);

    let mut content = column![summary, warnings]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill);
    if render_limited {
        content = content.push(
            layout::faint(format_count(
                lang,
                "detail.dll_scan.render_limited",
                "detail.dll_scan.render_limited",
                DLL_SCAN_RENDER_LIMIT,
            ))
            .size(10),
        );
    }
    content = content.push(toolbar).push(mode);

    let panel = container(
        scrollable(list)
            .height(Length::Fill)
            .style(styles::scrollable),
    )
    .padding(8)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(styles::list_panel);

    content = content.push(panel);
    if let Some(error) = &dialog.apply_error {
        content = content.push(
            text(error.clone())
                .size(12)
                .font(font(Weight::Normal))
                .style(|theme| iced::widget::text::Style {
                    color: Some(styles::tone_colors(theme, Tone::Danger).1),
                }),
        );
    }
    content.push(footer).into()
}

fn format_count(
    lang: crate::i18n::Lang,
    one: &'static str,
    other: &'static str,
    count: usize,
) -> String {
    crate::i18n::tr_plural_args(
        lang,
        one,
        other,
        count as u64,
        &[("count", &count.to_string())],
    )
}

fn warning_text<'a>(lang: crate::i18n::Lang, value: String) -> Element<'a, Message> {
    let _ = lang;
    text(value)
        .size(11)
        .font(font(Weight::Normal))
        .style(|theme| iced::widget::text::Style {
            color: Some(styles::tone_colors(theme, Tone::Warning).1),
        })
        .into()
}
