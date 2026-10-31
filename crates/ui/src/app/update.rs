use super::{
    App, COMPONENT_TAIL_LINES, ComponentJob, DeleteTarget, ErrorNotice, Message, NoticeKind,
    Screen, SettingsAction,
};
use application::editor::{DraftAction, GameDraft, apply_env_action, env_rows_to_map};
use application::{ComponentEvent, LaunchEvent, LaunchRequest, LaunchTuning, PrefixStatus};
use iced::Task;
use std::collections::BTreeMap;

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(screen) => {
                self.screen = screen;
                Task::none()
            }
            Message::SelectGame(id) => {
                self.selected_game = Some(id);
                self.draft = self
                    .games
                    .iter()
                    .find(|game| game.id == id)
                    .map(GameDraft::from_game);
                self.touch_draft();
                self.prefix_status = None;
                self.screen = Screen::GameDetail;
                Task::batch([
                    self.refresh_prefix_status(id),
                    self.refresh_monitors(),
                    self.refresh_installed_verbs(id),
                ])
            }
            Message::NewGame => {
                self.draft = None;
                self.selected_game = None;
                self.new_game_platform = Some(application::Platform::Windows);
                Task::none()
            }
            Message::BeginNewGame(platform) => {
                self.new_game_platform = None;
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickExe(platform), self.lang)
            }
            Message::CancelNewGame => {
                self.new_game_platform = None;
                Task::none()
            }
            Message::Loaded(data) => self.apply_loaded(data),
            Message::LoadFailed(error) => {
                self.pending.loading = false;
                self.error = Some(ErrorNotice::new(self.tr("app.error.load")).with_detail(error));
                Task::none()
            }
            Message::Draft(action) => {
                let affects_prefix = matches!(
                    &action,
                    DraftAction::Runner(_)
                        | DraftAction::Prefix(_)
                        | DraftAction::ClearPrefix
                        | DraftAction::Arch(_)
                        | DraftAction::Platform(_)
                );
                let refresh_id = self.draft.as_ref().map(|draft| draft.id);
                if let Some(draft) = &mut self.draft {
                    draft.apply(action);
                    self.touch_draft();
                }
                match (affects_prefix, refresh_id) {
                    (true, Some(id)) => {
                        self.prefix_status = None;
                        Task::batch([
                            self.refresh_prefix_status(id),
                            self.refresh_installed_verbs(id),
                        ])
                    }
                    _ => Task::none(),
                }
            }
            Message::SaveDraft => self.save_draft(),
            Message::DraftSaved {
                game_id,
                revision,
                cover_changed,
                result,
            } => self.apply_draft_saved(game_id, revision, cover_changed, result),
            Message::CoverLoaded {
                game_id,
                path,
                bytes,
            } => {
                let current = self
                    .games
                    .iter()
                    .find(|game| game.id == game_id)
                    .and_then(|game| game.cover_path.as_deref());
                if current == Some(path.as_path()) {
                    match bytes {
                        Some(bytes) => {
                            self.cover_handles
                                .insert(game_id, iced::widget::image::Handle::from_bytes(bytes));
                        }
                        None => {
                            self.cover_handles.remove(&game_id);
                        }
                    }
                }
                Task::none()
            }
            Message::Pick(intent) => crate::dialogs::pick(intent, self.lang),
            Message::DialogResult { intent, path } => self.apply_dialog_result(intent, path),
            Message::OpenDllScan => self.open_dll_scan(),
            Message::CloseDllScan | Message::DllScanCancelScan => {
                self.close_dll_scan();
                Task::none()
            }
            Message::DllScanPickDir => {
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickDllScanDir, self.lang)
            }
            Message::DllScanQueryChanged(query) => {
                if let Some(dialog) = self.dll_scan.as_mut() {
                    dialog.query = query;
                }
                Task::none()
            }
            Message::DllScanToggle(key) => {
                if let Some(dialog) = self.dll_scan.as_mut() {
                    dialog.toggle(key);
                }
                Task::none()
            }
            Message::DllScanSelectAll => {
                if let Some(dialog) = self.dll_scan.as_mut() {
                    dialog.select_all_filtered();
                }
                Task::none()
            }
            Message::DllScanInvert => {
                if let Some(dialog) = self.dll_scan.as_mut() {
                    dialog.invert_filtered();
                }
                Task::none()
            }
            Message::DllScanClear => {
                if let Some(dialog) = self.dll_scan.as_mut() {
                    dialog.selected.clear();
                }
                Task::none()
            }
            Message::DllScanSortToggled => {
                if let Some(dialog) = self.dll_scan.as_mut() {
                    dialog.sort_desc = !dialog.sort_desc;
                }
                Task::none()
            }
            Message::DllScanModeChanged(mode) => {
                if let Some(dialog) = self.dll_scan.as_mut() {
                    dialog.mode = mode;
                }
                Task::none()
            }
            Message::DllScanRescan => {
                if self.dll_scan.as_ref().is_some_and(|dialog| dialog.applying) {
                    Task::none()
                } else {
                    self.start_dll_scan()
                }
            }
            Message::DllScanFinished { seq, result } => self.dll_scan_finished(seq, result),
            Message::DllScanConfirm => self.confirm_dll_scan(),
            Message::DllScanDirSaved(result) => {
                if let Err(error) = result {
                    tracing::warn!(%error, "failed to persist DLL scan directory");
                }
                Task::none()
            }
            Message::DismissSuccess => {
                self.success = None;
                Task::none()
            }
            Message::DismissError => {
                self.error = None;
                Task::none()
            }
            Message::Error(error) => {
                self.error = Some(ErrorNotice::new(error));
                Task::none()
            }
            Message::DetectSystemWine => {
                if self.pending.detecting_wine || self.pending.importing_proton {
                    return Task::none();
                }
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                    return Task::none();
                };
                self.pending.detecting_wine = true;
                let lang = self.lang;
                Task::perform(service.detect_and_persist_runners(), move |result| {
                    Message::WineDetected(
                        result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    )
                })
            }
            Message::WineDetected(result) => {
                self.pending.detecting_wine = false;
                match result {
                    Ok(runners) => {
                        self.error = None;
                        if runners.is_empty() {
                            self.error =
                                Some(ErrorNotice::new(self.tr("app.error.wine_not_found")));
                        }
                        self.runners = runners;
                        self.runners
                            .sort_by_key(|runner| runner.name.to_lowercase());
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::new(self.tr("app.error.wine_detect")).with_detail(error),
                        );
                    }
                }
                Task::none()
            }
            Message::ImportProton => {
                if self.pending.detecting_wine || self.pending.importing_proton {
                    return Task::none();
                }
                self.pending.importing_proton = true;
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickProtonDir, self.lang)
            }
            Message::ProtonImported(result) => {
                self.pending.importing_proton = false;
                match result {
                    Ok(runner) => {
                        self.error = None;
                        self.runners.retain(|existing| existing.id != runner.id);
                        self.runners.push(runner);
                        self.runners
                            .sort_by_key(|runner| runner.name.to_lowercase());
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::new(self.tr("app.error.proton_import")).with_detail(error),
                        );
                    }
                }
                Task::none()
            }
            Message::SetDefaultRunner(id) => {
                if self.pending.settings_saving || self.pending.deleting {
                    return Task::none();
                }
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                    return Task::none();
                };
                self.pending.settings_saving = true;
                let lang = self.lang;
                Task::perform(service.save_default_runner(id), move |result| {
                    Message::DefaultRunnerSaved {
                        id,
                        result: result
                            .map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    }
                })
            }
            Message::DefaultRunnerSaved { id, result } => {
                self.pending.settings_saving = false;
                match result {
                    Ok(()) => {
                        self.settings.default_runner_id = id;
                        self.prefix_status = None;
                        if let Some(game_id) = self.selected_game {
                            return self.refresh_prefix_status(game_id);
                        }
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::new(self.tr("app.error.runner_default_save"))
                                .with_detail(error),
                        );
                    }
                }
                Task::none()
            }
            Message::CreatePrefix(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(ErrorNotice::new(
                        self.tr("app.error.save_game_first_prefix"),
                    ));
                    return Task::none();
                }
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(ErrorNotice::new(error));
                        return Task::none();
                    }
                };
                let plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return Task::none();
                    }
                };
                let dxvk = self
                    .dxvk_dir
                    .clone()
                    .map(|root| application::DxvkSource { root });
                self.pending.prefix.insert(game_id);
                let lang = self.lang;
                Task::perform(application::create_prefix(plan, dxvk), move |result| {
                    Message::PrefixCreated {
                        game_id,
                        result: result
                            .map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    }
                })
            }
            Message::PrefixCreated { game_id, result } => {
                self.pending.prefix.remove(&game_id);
                match result {
                    Ok(path) => {
                        let mut touched = false;
                        if let Some(draft) = &mut self.draft {
                            if draft.id == game_id {
                                draft.prefix_path = Some(path);
                                touched = true;
                            }
                        }
                        if touched {
                            self.touch_draft();
                        }
                        return self.refresh_prefix_status(game_id);
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::new(self.tr("app.error.prefix_create")).with_detail(error),
                        );
                    }
                }
                Task::none()
            }
            Message::InstallDxvk(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.save_game_first_dxvk")));
                    return Task::none();
                }
                let Some(dxvk_dir) = self.dxvk_dir.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.dxvk_dir_required")));
                    return Task::none();
                };
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(ErrorNotice::new(error));
                        return Task::none();
                    }
                };
                let plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return Task::none();
                    }
                };
                self.pending.dxvk.insert(game_id);
                let lang = self.lang;
                Task::perform(
                    application::install_dxvk(plan, application::DxvkSource { root: dxvk_dir }),
                    move |result| Message::DxvkInstalled {
                        game_id,
                        result: result
                            .map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    },
                )
            }
            Message::DxvkInstalled { game_id, result } => {
                self.pending.dxvk.remove(&game_id);
                if let Err(error) = result {
                    self.error = Some(
                        ErrorNotice::new(self.tr("app.error.dxvk_install")).with_detail(error),
                    );
                }
                Task::none()
            }
            Message::ComponentPresetChanged(verb) => {
                self.component_preset = verb;
                Task::none()
            }
            Message::ComponentVerbChanged(verb) => {
                self.component_verb = verb;
                Task::none()
            }
            Message::InstallComponent { game_id, verb } => {
                if self.pending.loading || self.pending.saving || self.pending.deleting {
                    return Task::none();
                }
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(ErrorNotice::new(
                        self.tr("app.error.save_game_first_component"),
                    ));
                    return Task::none();
                }
                let Some(winetricks) = self.winetricks_path.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.winetricks_missing")));
                    return Task::none();
                };
                if self.prefix_status != Some(PrefixStatus::Initialized) {
                    self.error = Some(ErrorNotice::new(
                        self.tr("app.error.component_prefix_required"),
                    ));
                    return Task::none();
                }
                if self.components.contains_key(&game_id) {
                    return Task::none();
                }
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(ErrorNotice::new(error));
                        return Task::none();
                    }
                };
                let plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return Task::none();
                    }
                };
                if self
                    .running
                    .values()
                    .flatten()
                    .any(|path| path == &plan.path)
                    || self
                        .components
                        .values()
                        .any(|job| job.plan.path == plan.path)
                {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.component_running")));
                    return Task::none();
                }
                let verb = verb.trim().to_owned();
                if verb.is_empty() || verb.starts_with('-') {
                    self.error = Some(ErrorNotice::new(
                        self.tr("app.error.component_verb_invalid"),
                    ));
                    return Task::none();
                }
                self.components.insert(
                    game_id,
                    ComponentJob {
                        plan: plan.clone(),
                        verb: verb.clone(),
                        process_group: None,
                        cancelling: false,
                        tail: Vec::new(),
                    },
                );
                let log_path = self.paths.component_log_path(game_id);
                component_task(plan, winetricks, verb, log_path, game_id, self.lang)
            }
            Message::ComponentEvent { game_id, event } => {
                if let Some(job) = self.components.get_mut(&game_id) {
                    match event {
                        ComponentEvent::Started { process_group } => {
                            job.process_group = Some(process_group);
                        }
                        ComponentEvent::Line(line) => push_component_tail(&mut job.tail, line),
                    }
                }
                Task::none()
            }
            Message::ComponentFinished { game_id, outcome } => {
                let Some(job) = self.components.remove(&game_id) else {
                    return Task::none();
                };
                if !outcome.success && !job.cancelling {
                    let detail = match outcome.exit_code {
                        Some(code) => format!("{} · exit {code}", outcome.verb),
                        None => outcome.verb,
                    };
                    self.error = Some(
                        ErrorNotice::new(self.tr("app.error.component_failed")).with_detail(detail),
                    );
                }
                let prefix = job.plan.path.clone();
                Task::perform(application::installed_components(job.plan), move |verbs| {
                    Message::ComponentsLoaded {
                        prefix: prefix.clone(),
                        verbs,
                    }
                })
            }
            Message::ComponentFailed { game_id, error } => {
                self.components.remove(&game_id);
                self.error = Some(
                    ErrorNotice::new(self.tr("app.error.component_failed")).with_detail(error),
                );
                Task::none()
            }
            Message::CancelComponent(game_id) => {
                let Some(job) = self.components.get_mut(&game_id) else {
                    return Task::none();
                };
                let Some(process_group) = job.process_group else {
                    return Task::none();
                };
                job.cancelling = true;
                let lang = self.lang;
                Task::perform(
                    application::cancel_component(process_group),
                    move |result| Message::ComponentCancelFinished {
                        game_id,
                        result: result
                            .map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    },
                )
            }
            Message::ComponentCancelFinished { game_id, result } => {
                if let Err(error) = result {
                    if let Some(job) = self.components.get_mut(&game_id) {
                        job.cancelling = false;
                    }
                    self.error = Some(
                        ErrorNotice::new(self.tr("app.error.component_cancel")).with_detail(error),
                    );
                }
                Task::none()
            }
            Message::ComponentsLoaded { prefix, verbs } => {
                self.installed_verbs.insert(prefix, verbs);
                Task::none()
            }
            Message::RefreshPrefixStatus(game_id) => self.refresh_prefix_status(game_id),
            Message::PrefixStatusLoaded { game_id, status } => {
                if self.selected_game == Some(game_id) {
                    self.prefix_status = Some(status);
                }
                Task::none()
            }
            Message::Settings(action) => {
                let affects_prefix = matches!(
                    &action,
                    SettingsAction::DefaultRunner(_) | SettingsAction::PrefixRoot(_)
                );
                match action {
                    SettingsAction::Env(action) => {
                        apply_env_action(&mut self.settings.global_env_rows, action);
                    }
                    SettingsAction::DefaultRunner(id) => {
                        self.settings.default_runner_id = id;
                    }
                    SettingsAction::PrefixRoot(path) => {
                        self.settings.prefix_root = path;
                    }
                    SettingsAction::EnableEsync(value) => {
                        self.settings.enable_esync = value;
                    }
                    SettingsAction::EnableFsync(value) => {
                        self.settings.enable_fsync = value;
                    }
                    SettingsAction::EnableMangoHud(value) => {
                        self.settings.enable_mangohud = value;
                    }
                    SettingsAction::AutoInstallDxvkNvapi(value) => {
                        self.settings.auto_install_dxvk_nvapi = value;
                    }
                    SettingsAction::DxvkNvapiSource(path) => {
                        self.settings.dxvk_nvapi_source = path;
                    }
                    SettingsAction::Theme(mode) => {
                        if self.pending.loading {
                            return Task::none();
                        }
                        self.settings.theme = mode;
                        if self.theme_saving {
                            return Task::none();
                        }
                        let Some(service) = self.service.clone() else {
                            self.error =
                                Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                            return Task::none();
                        };
                        self.theme_saving = true;
                        return save_theme(service, mode, self.lang);
                    }
                    SettingsAction::Language(pref) => {
                        if self.pending.loading {
                            return Task::none();
                        }
                        self.settings.language = pref;
                        self.lang = pref.resolve();
                        if self.language_saving {
                            return Task::none();
                        }
                        let Some(service) = self.service.clone() else {
                            self.error =
                                Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                            return Task::none();
                        };
                        self.language_saving = true;
                        return save_language(service, pref, self.lang);
                    }
                }
                match (affects_prefix, self.selected_game) {
                    (true, Some(id)) => {
                        self.prefix_status = None;
                        self.refresh_prefix_status(id)
                    }
                    _ => Task::none(),
                }
            }
            Message::SaveSettings => {
                if self.pending.loading || self.pending.settings_saving {
                    return Task::none();
                }
                let mut values = match self.settings.to_map() {
                    Ok(values) => values,
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::form(self.tr("app.error.settings_validation"))
                                .with_detail(error),
                        );
                        return Task::none();
                    }
                };
                values.remove(application::SETTING_UI_THEME);
                values.remove(application::SETTING_UI_LANGUAGE);
                values.remove(application::SETTING_UI_FONT);
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                    return Task::none();
                };
                self.pending.settings_saving = true;
                let lang = self.lang;
                Task::perform(service.save_settings(values), move |result| {
                    Message::SettingsSaved(
                        result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    )
                })
            }
            Message::SettingsSaved(result) => {
                self.pending.settings_saving = false;
                match result {
                    Ok(()) => {
                        if self
                            .error
                            .as_ref()
                            .is_some_and(|notice| notice.kind == NoticeKind::Form)
                        {
                            self.error = None;
                        }
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::form(self.tr("app.error.settings_save"))
                                .with_detail(error),
                        );
                    }
                }
                Task::none()
            }
            Message::ThemeSaved { mode, result } => {
                self.theme_saving = false;
                match result {
                    Ok(()) => {
                        if self
                            .error
                            .as_ref()
                            .is_some_and(|notice| notice.kind == NoticeKind::ThemeSave)
                        {
                            self.error = None;
                        }
                        if self.settings.theme != mode {
                            let Some(service) = self.service.clone() else {
                                return Task::none();
                            };
                            self.theme_saving = true;
                            return save_theme(service, self.settings.theme, self.lang);
                        }
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::theme_save(self.tr("app.error.theme_save"))
                                .with_detail(error),
                        );
                        if self.settings.theme != mode {
                            if let Some(service) = self.service.clone() {
                                self.theme_saving = true;
                                return save_theme(service, self.settings.theme, self.lang);
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::LanguageSaved { pref, result } => {
                self.language_saving = false;
                match result {
                    Ok(()) => {
                        if self
                            .error
                            .as_ref()
                            .is_some_and(|notice| notice.kind == NoticeKind::LanguageSave)
                        {
                            self.error = None;
                        }
                        if self.settings.language != pref {
                            let Some(service) = self.service.clone() else {
                                return Task::none();
                            };
                            self.language_saving = true;
                            return save_language(service, self.settings.language, self.lang);
                        }
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::language_save(self.tr("app.error.language_save"))
                                .with_detail(error),
                        );
                        if self.settings.language != pref {
                            if let Some(service) = self.service.clone() {
                                self.language_saving = true;
                                return save_language(service, self.settings.language, self.lang);
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::FontPanelToggled => {
                self.font_panel_open = !self.font_panel_open;
                Task::none()
            }
            Message::FontSearchChanged(query) => {
                self.font_search = query;
                Task::none()
            }
            Message::FontCjkOnlyToggled(value) => {
                self.font_cjk_only = value;
                Task::none()
            }
            Message::ScanFonts => {
                if self.fonts_loading {
                    return Task::none();
                }
                self.fonts_loading = true;
                self.fonts_error = None;
                Self::scan_fonts()
            }
            Message::FontsLoaded(result) => {
                self.fonts_loading = false;
                match result {
                    Ok(fonts) => {
                        self.fonts = fonts;
                        self.fonts_error = None;
                        self.apply_saved_font();
                    }
                    Err(error) => {
                        self.fonts_error =
                            Some(self.tr_args("errors.font_scan", &[("error", &error)]));
                    }
                }
                Task::none()
            }
            Message::FontSelected(selected) => {
                if self.pending.loading {
                    return Task::none();
                }
                let name = selected.filter(|name| {
                    name != crate::theme::FONT_FAMILY
                        && self.fonts.iter().any(|font| font.name == name)
                });
                match name
                    .as_deref()
                    .and_then(|name| self.fonts.iter().find(|font| font.name == name).copied())
                {
                    Some(entry) => {
                        crate::theme::set_active_font(Some((entry.name, entry.has_bold)))
                    }
                    None => crate::theme::set_active_font(None),
                }
                self.settings.font = name.clone();
                self.font_panel_open = false;
                self.font_missing_reported = false;
                if self.font_saving {
                    return Task::none();
                }
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                    return Task::none();
                };
                self.font_saving = true;
                save_font(service, name, self.lang)
            }
            Message::FontSaved { name, result } => {
                self.font_saving = false;
                match result {
                    Ok(()) => {
                        if self
                            .error
                            .as_ref()
                            .is_some_and(|notice| notice.kind == NoticeKind::FontSave)
                        {
                            self.error = None;
                        }
                        if self.settings.font != name {
                            let Some(service) = self.service.clone() else {
                                return Task::none();
                            };
                            self.font_saving = true;
                            return save_font(service, self.settings.font.clone(), self.lang);
                        }
                    }
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::font_save(self.tr("app.error.font_save"))
                                .with_detail(error),
                        );
                        if self.settings.font != name {
                            if let Some(service) = self.service.clone() {
                                self.font_saving = true;
                                return save_font(service, self.settings.font.clone(), self.lang);
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::WindowResized(width) => {
                self.window_width = width;
                Task::none()
            }
            Message::ConfirmDelete(target) => {
                self.confirming = Some(target);
                Task::none()
            }
            Message::CancelDelete => {
                self.confirming = None;
                Task::none()
            }
            Message::DeleteConfirmed => {
                if self.pending.deleting || self.pending.saving {
                    return Task::none();
                }
                let Some(target) = self.confirming.clone() else {
                    return Task::none();
                };
                if match &target {
                    DeleteTarget::Game(id) => self.components.contains_key(id),
                    DeleteTarget::Runner(id) => self
                        .components
                        .values()
                        .any(|job| job.plan.runner.id == *id),
                    DeleteTarget::Prefix { path, .. } => {
                        self.components.values().any(|job| job.plan.path == *path)
                    }
                } {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.component_running")));
                    return Task::none();
                }
                if let DeleteTarget::Prefix { game_id, path } = &target {
                    let saved = self
                        .games
                        .iter()
                        .find(|game| game.id == *game_id)
                        .map(|game| {
                            game.prefix_path.clone().unwrap_or_else(|| {
                                self.settings.prefix_root.join(game_id.to_string())
                            })
                        });
                    if saved.as_deref() != Some(path.as_path()) {
                        self.confirming = None;
                        self.error =
                            Some(ErrorNotice::new(self.tr("app.error.prefix_unsaved_delete")));
                        return Task::none();
                    }
                }
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                    return Task::none();
                };
                self.pending.deleting = true;
                let running = self.running.clone();
                let default_runner_id = self.settings.default_runner_id;
                let lang = self.lang;
                Task::perform(
                    service.delete(
                        target.clone(),
                        running,
                        default_runner_id,
                        self.paths.logs_dir.clone(),
                    ),
                    move |result| Message::DeleteFinished {
                        target: target.clone(),
                        result: result
                            .map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    },
                )
            }
            Message::DeleteFinished { target, result } => {
                self.pending.deleting = false;
                self.confirming = None;
                match result {
                    Ok(()) => {
                        self.prefix_status = None;
                        match target {
                            DeleteTarget::Runner(id) => {
                                self.runners.retain(|runner| runner.id != id);
                                if self.settings.default_runner_id == Some(id) {
                                    self.settings.default_runner_id = None;
                                }
                                let mut touched = false;
                                if let Some(draft) = &mut self.draft {
                                    if draft.runner_id == Some(id) {
                                        draft.runner_id = None;
                                        touched = true;
                                    }
                                }
                                if touched {
                                    self.touch_draft();
                                }
                            }
                            DeleteTarget::Prefix { game_id, path } => {
                                let mut touched = false;
                                if let Some(draft) = &mut self.draft {
                                    if draft.id == game_id {
                                        draft.prefix_path = None;
                                        touched = true;
                                    }
                                }
                                if touched {
                                    self.touch_draft();
                                }
                                if let Some(game) =
                                    self.games.iter_mut().find(|game| game.id == game_id)
                                {
                                    if game.prefix_path.as_deref() == Some(path.as_path()) {
                                        game.prefix_path = None;
                                    }
                                }
                            }
                            DeleteTarget::Game(id) => {
                                self.games.retain(|game| game.id != id);
                                self.cover_handles.remove(&id);
                                if self.selected_game == Some(id) {
                                    self.selected_game = None;
                                    self.draft = None;
                                    self.screen = Screen::Library;
                                }
                            }
                        }
                        self.error = None;
                        if let Some(id) = self.selected_game {
                            return self.refresh_prefix_status(id);
                        }
                    }
                    Err(error) => {
                        self.error =
                            Some(ErrorNotice::new(self.tr("app.error.delete")).with_detail(error));
                    }
                }
                Task::none()
            }
            Message::Launch(game_id) => {
                if self.pending.saving || self.pending.deleting {
                    return Task::none();
                }
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(ErrorNotice::new(
                        self.tr("app.error.save_game_first_launch"),
                    ));
                    return Task::none();
                }
                if self.running.contains_key(&game_id) {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.already_running")));
                    return Task::none();
                }
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(ErrorNotice::new(error));
                        return Task::none();
                    }
                };
                let (runner, prefix_path) = if game.platform.is_native() {
                    (None, None)
                } else {
                    let prefix_plan = match self.prefix_plan_for(&game) {
                        Ok(plan) => plan,
                        Err(error) => {
                            self.error = Some(error);
                            return Task::none();
                        }
                    };
                    if self
                        .running
                        .values()
                        .flatten()
                        .any(|path| path == &prefix_plan.path)
                    {
                        self.error = Some(ErrorNotice::new(self.tr("app.error.prefix_busy")));
                        return Task::none();
                    }
                    if self
                        .components
                        .values()
                        .any(|job| job.plan.path == prefix_plan.path)
                    {
                        self.error = Some(ErrorNotice::new(self.tr("app.error.component_running")));
                        return Task::none();
                    }
                    (Some(prefix_plan.runner.clone()), Some(prefix_plan.path))
                };

                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                    return Task::none();
                };
                let global_env = match env_rows_to_map(&self.settings.global_env_rows) {
                    Ok(env) => env,
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::new(self.tr("app.error.global_env_invalid"))
                                .with_detail(crate::i18n::errors::draft_error(self.lang, &error)),
                        );
                        return Task::none();
                    }
                };

                let request = LaunchRequest {
                    game,
                    runner,
                    session: self.session,
                    global_env,
                    inherited_env: Self::inherited_env(),
                    steam_compat_client: self.paths.steam_compat_client.clone(),
                    default_prefix_root: self.settings.prefix_root.clone(),
                    gamescope_path: self.gamescope_path.clone(),
                    tuning: LaunchTuning {
                        enable_esync: self.settings.enable_esync,
                        enable_fsync: self.settings.enable_fsync,
                        enable_mangohud: self.settings.enable_mangohud,
                        mangohud_path: self.mangohud_path.clone(),
                        enable_dxvk_nvapi: self.settings.auto_install_dxvk_nvapi,
                    },
                    log_path: Some(self.paths.game_log_path(game_id)),
                    dxvk_nvapi_source: self.settings.dxvk_nvapi_source.clone(),
                };

                self.running.insert(game_id, prefix_path);
                self.starting.insert(game_id);
                launch_task(service, request, self.lang)
            }
            Message::LaunchEvent(event) => {
                match event {
                    LaunchEvent::Started {
                        game_id,
                        started_at,
                    } => {
                        self.starting.remove(&game_id);
                        if let Some(game) = self.games.iter_mut().find(|game| game.id == game_id) {
                            game.last_played_at = Some(started_at);
                        }
                        if let Some(draft) = &mut self.draft {
                            if draft.id == game_id {
                                draft.last_played_at = Some(started_at);
                            }
                        }
                    }
                    LaunchEvent::Finished { game_id, outcome } => {
                        self.starting.remove(&game_id);
                        self.running.remove(&game_id);
                        self.stopping.remove(&game_id);
                        let session_seconds = outcome.session_seconds.max(0);
                        if let Some(game) = self.games.iter_mut().find(|game| game.id == game_id) {
                            game.playtime_seconds =
                                game.playtime_seconds.saturating_add(session_seconds);
                        }
                        if let Some(draft) = &mut self.draft {
                            if draft.id == game_id {
                                draft.playtime_seconds =
                                    draft.playtime_seconds.saturating_add(session_seconds);
                            }
                        }
                        if !outcome.success && !outcome.stopped {
                            self.error = Some(ErrorNotice::new(self.tr_args(
                                "launch.exited_with_code",
                                &[("code", &format!("{:?}", outcome.exit_code))],
                            )));
                        }
                    }
                    LaunchEvent::Failed { game_id, error } => {
                        self.starting.remove(&game_id);
                        self.running.remove(&game_id);
                        self.stopping.remove(&game_id);
                        self.error =
                            Some(ErrorNotice::new(self.tr("launch.failed")).with_detail(error));
                    }
                }
                Task::none()
            }
            Message::LaunchFailed { game_id, error } => {
                self.starting.remove(&game_id);
                self.running.remove(&game_id);
                self.stopping.remove(&game_id);
                self.error = Some(ErrorNotice::new(self.tr("launch.failed")).with_detail(error));
                Task::none()
            }
            Message::Stop(game_id) => {
                if !self.running.contains_key(&game_id) {
                    return Task::none();
                }
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
                    return Task::none();
                };
                self.stopping.insert(game_id);
                let lang = self.lang;
                Task::perform(service.stop_game(game_id), move |result| {
                    Message::StopFinished {
                        game_id,
                        result: result
                            .map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                    }
                })
            }
            Message::StopFinished { game_id, result } => {
                if let Err(error) = result {
                    self.stopping.remove(&game_id);
                    self.error =
                        Some(ErrorNotice::new(self.tr("launch.stop_failed")).with_detail(error));
                }
                Task::none()
            }
            Message::RefreshMonitors => self.refresh_monitors(),
            Message::MonitorsLoaded(result) => {
                match result {
                    Ok(monitors) => self.monitors = monitors,
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::new(self.tr("app.error.monitors_scan")).with_detail(error),
                        );
                    }
                }
                Task::none()
            }
            Message::OpenLogs(game_id) => {
                self.logs_game = Some(game_id);
                self.logs_content.clear();
                self.logs_error = None;
                self.screen = Screen::Logs;
                self.load_logs(game_id)
            }
            Message::RefreshLogs => match self.logs_game {
                Some(game_id) => self.load_logs(game_id),
                None => Task::none(),
            },
            Message::LogsLoaded { game_id, result } => {
                if self.logs_game == Some(game_id) {
                    match result {
                        Ok(content) => {
                            self.logs_content = content;
                            self.logs_error = None;
                        }
                        Err(error) => {
                            self.logs_error = Some(error);
                            self.logs_content.clear();
                        }
                    }
                }
                Task::none()
            }
        }
    }
}

fn save_theme(
    service: application::AppService,
    mode: crate::theme::ThemeMode,
    lang: crate::i18n::Lang,
) -> Task<Message> {
    let values = BTreeMap::from([(
        application::SETTING_UI_THEME.to_string(),
        Some(mode.as_setting().to_string()),
    )]);
    Task::perform(service.save_settings(values), move |result| {
        Message::ThemeSaved {
            mode,
            result: result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
        }
    })
}

fn save_language(
    service: application::AppService,
    pref: crate::i18n::LanguagePref,
    lang: crate::i18n::Lang,
) -> Task<Message> {
    let values = BTreeMap::from([(
        application::SETTING_UI_LANGUAGE.to_string(),
        Some(pref.as_setting().to_string()),
    )]);
    Task::perform(service.save_settings(values), move |result| {
        Message::LanguageSaved {
            pref,
            result: result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
        }
    })
}

fn save_font(
    service: application::AppService,
    name: Option<String>,
    lang: crate::i18n::Lang,
) -> Task<Message> {
    let values = BTreeMap::from([(application::SETTING_UI_FONT.to_string(), name.clone())]);
    Task::perform(service.save_settings(values), move |result| {
        Message::FontSaved {
            name: name.clone(),
            result: result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
        }
    })
}

fn push_component_tail(tail: &mut Vec<String>, line: String) {
    tail.push(line);
    if tail.len() > COMPONENT_TAIL_LINES {
        let excess = tail.len() - COMPONENT_TAIL_LINES;
        tail.drain(..excess);
    }
}

fn launch_task(
    service: application::AppService,
    request: LaunchRequest,
    lang: crate::i18n::Lang,
) -> Task<Message> {
    let game_id = request.game.id;
    Task::stream(iced::stream::channel(64, move |mut output| async move {
        use iced::futures::SinkExt;

        let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let handle =
            tokio::spawn(async move { service.launch_game_with_events(request, events).await });

        while let Some(event) = receiver.recv().await {
            if matches!(event, application::LaunchEvent::Failed { .. }) {
                continue;
            }
            if output.send(Message::LaunchEvent(event)).await.is_err() {
                return;
            }
        }

        if let Ok(Err(error)) = handle.await {
            let _ = output
                .send(Message::LaunchFailed {
                    game_id,
                    error: crate::i18n::errors::app_error(lang, &error),
                })
                .await;
        }
    }))
}

fn component_task(
    plan: application::PrefixPlan,
    winetricks: std::path::PathBuf,
    verb: String,
    log_path: std::path::PathBuf,
    game_id: uuid::Uuid,
    lang: crate::i18n::Lang,
) -> Task<Message> {
    Task::stream(iced::stream::channel(64, move |mut output| async move {
        use iced::futures::SinkExt;

        let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            application::install_component(plan, winetricks, &verb, log_path, events).await
        });

        while let Some(event) = receiver.recv().await {
            if output
                .send(Message::ComponentEvent { game_id, event })
                .await
                .is_err()
            {
                return;
            }
        }

        match handle.await {
            Ok(Ok(outcome)) => {
                let _ = output
                    .send(Message::ComponentFinished { game_id, outcome })
                    .await;
            }
            Ok(Err(error)) => {
                let _ = output
                    .send(Message::ComponentFailed {
                        game_id,
                        error: crate::i18n::errors::app_error(lang, &error),
                    })
                    .await;
            }
            Err(error) => {
                let _ = output
                    .send(Message::ComponentFailed {
                        game_id,
                        error: error.to_string(),
                    })
                    .await;
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use crate::app::{App, ErrorNotice, Message, NoticeKind, ServiceError, SettingsAction};
    use crate::i18n::LanguagePref;
    use application::AppPaths;
    use std::path::PathBuf;

    fn app() -> App {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-update-test"));
        let (mut app, _) = App::new(paths, Err(ServiceError::DatabaseLocked));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::ZH_CN);
        app.lang = &crate::i18n::ZH_CN;
        app
    }

    #[test]
    fn component_tail_keeps_only_the_latest_lines() {
        let mut tail = Vec::new();
        for index in 0..(super::COMPONENT_TAIL_LINES + 5) {
            super::push_component_tail(&mut tail, format!("line {index}"));
        }

        assert_eq!(tail.len(), super::COMPONENT_TAIL_LINES);
        assert_eq!(tail.first().map(String::as_str), Some("line 5"));
    }

    #[test]
    fn install_component_without_winetricks_raises_a_notice() {
        let mut app = app();
        app.winetricks_path = None;

        let _ = app.update(Message::InstallComponent {
            game_id: uuid::Uuid::new_v4(),
            verb: "vcrun2019".to_owned(),
        });

        let error = app
            .error
            .as_ref()
            .expect("missing winetricks should surface a notice");
        assert_eq!(error.title, app.tr("app.error.winetricks_missing"));
        assert!(app.components.is_empty());
    }

    #[test]
    fn install_component_requires_a_saved_game() {
        let mut app = app();
        let draft = application::editor::GameDraft::new(
            PathBuf::from("/tmp/gamehub/exe/game.exe"),
            application::Platform::Windows,
        );
        let game_id = draft.id;
        app.draft = Some(draft);
        app.winetricks_path = Some(PathBuf::from("/usr/bin/winetricks"));

        let _ = app.update(Message::InstallComponent {
            game_id,
            verb: "vcrun2019".to_owned(),
        });

        let error = app
            .error
            .as_ref()
            .expect("unsaved draft should surface a notice");
        assert_eq!(error.title, app.tr("app.error.save_game_first_component"));
        assert!(app.components.is_empty());
    }

    #[test]
    fn components_loaded_records_verbs_per_prefix() {
        let mut app = app();
        let prefix = PathBuf::from("/tmp/gamehub/prefixes/one");

        let _ = app.update(Message::ComponentsLoaded {
            prefix: prefix.clone(),
            verbs: vec!["corefonts".to_owned(), "vcrun2019".to_owned()],
        });

        assert_eq!(
            app.installed_verbs.get(&prefix).map(Vec::as_slice),
            Some(["corefonts".to_owned(), "vcrun2019".to_owned()].as_slice())
        );
    }

    #[test]
    fn duplicate_runner_actions_are_ignored_while_busy() {
        let mut app = app();

        app.pending.detecting_wine = true;
        let _ = app.update(Message::DetectSystemWine);
        assert!(app.error.is_none());
        assert!(app.pending.detecting_wine);

        app.pending.detecting_wine = false;
        app.pending.importing_proton = true;
        let _ = app.update(Message::ImportProton);
        assert!(app.error.is_none());
        assert!(app.pending.importing_proton);

        app.pending.importing_proton = false;
        app.pending.settings_saving = true;
        let _ = app.update(Message::SetDefaultRunner(None));
        assert!(app.error.is_none());
        assert!(app.pending.settings_saving);
    }

    #[test]
    fn switching_language_updates_app_state() {
        let mut app = app();
        app.settings.language = crate::i18n::LanguagePref::Auto;
        let _ = app.update(Message::Settings(SettingsAction::Language(
            crate::i18n::LanguagePref::Fixed(&crate::i18n::EN_US),
        )));
        assert_eq!(app.lang, &crate::i18n::EN_US);
        assert_eq!(
            app.settings.language,
            crate::i18n::LanguagePref::Fixed(&crate::i18n::EN_US)
        );
    }

    #[test]
    fn language_selection_while_saving_updates_state_without_new_save() {
        let mut app = app();
        app.language_saving = true;

        let _ = app.update(Message::Settings(SettingsAction::Language(
            LanguagePref::Fixed(&crate::i18n::EN_US),
        )));

        assert_eq!(app.lang, &crate::i18n::EN_US);
        assert_eq!(
            app.settings.language,
            LanguagePref::Fixed(&crate::i18n::EN_US)
        );
        assert!(app.language_saving);
    }

    #[test]
    fn language_saved_clears_language_notice() {
        let mut app = app();
        app.language_saving = true;
        app.error = Some(ErrorNotice::language_save("x"));

        let _ = app.update(Message::LanguageSaved {
            pref: app.settings.language,
            result: Ok(()),
        });

        assert!(app.error.is_none());
        assert!(!app.language_saving);
    }

    #[test]
    fn stale_language_save_resaves_latest_preference() {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-ui-test"));
        let service = application::AppService::open_in_memory().expect("in-memory service");
        let (mut app, _) = App::new(paths, Ok(service));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::EN_US);
        app.lang = &crate::i18n::EN_US;

        let _ = app.update(Message::LanguageSaved {
            pref: LanguagePref::Fixed(&crate::i18n::ZH_CN),
            result: Ok(()),
        });

        assert!(app.language_saving);
    }

    #[test]
    fn notice_kinds_identify_form_and_theme_errors() {
        assert_eq!(ErrorNotice::form("x").kind, NoticeKind::Form);
        assert_eq!(ErrorNotice::theme_save("x").kind, NoticeKind::ThemeSave);
        assert_eq!(
            ErrorNotice::language_save("x").kind,
            NoticeKind::LanguageSave
        );
        assert_eq!(ErrorNotice::font_save("x").kind, NoticeKind::FontSave);
        assert_eq!(ErrorNotice::new("x").kind, NoticeKind::General);
    }

    fn font_entry(name: &'static str, cjk: bool, has_bold: bool) -> crate::fonts::FontEntry {
        crate::fonts::FontEntry {
            name,
            cjk,
            has_bold,
        }
    }

    #[test]
    fn font_selection_updates_settings_and_closes_the_panel() {
        let mut app = app();
        app.fonts = vec![font_entry("DejaVu Sans", false, true)];
        app.font_panel_open = true;

        let _ = app.update(Message::FontSelected(Some("DejaVu Sans".to_owned())));

        assert_eq!(app.settings.font.as_deref(), Some("DejaVu Sans"));
        assert!(!app.font_panel_open);
    }

    #[test]
    fn selecting_the_default_font_clears_the_setting() {
        let mut app = app();
        app.settings.font = Some("DejaVu Sans".to_owned());
        app.fonts = vec![font_entry("DejaVu Sans", false, true)];

        let _ = app.update(Message::FontSelected(None));

        assert_eq!(app.settings.font, None);
    }

    #[test]
    fn unknown_selected_font_falls_back_to_the_default() {
        let mut app = app();
        app.fonts = vec![font_entry("DejaVu Sans", false, true)];

        let _ = app.update(Message::FontSelected(Some("Missing Font".to_owned())));

        assert_eq!(app.settings.font, None);
    }

    #[test]
    fn stored_font_missing_from_the_scan_reports_once() {
        let mut app = app();
        app.settings.font = Some("Uninstalled Font".to_owned());

        let _ = app.update(Message::FontsLoaded(Ok(vec![font_entry(
            "DejaVu Sans",
            false,
            true,
        )])));

        let notice = app.error.as_ref().expect("missing font should warn");
        assert_eq!(notice.title, app.tr("settings.font.missing"));
        assert!(app.font_missing_reported);

        app.error = None;
        let _ = app.update(Message::FontsLoaded(Ok(vec![font_entry(
            "DejaVu Sans",
            false,
            true,
        )])));
        assert!(app.error.is_none(), "the warning should not repeat");
    }

    #[test]
    fn stale_font_save_resaves_the_latest_choice() {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-font-test"));
        let service = application::AppService::open_in_memory().expect("in-memory service");
        let (mut app, _) = App::new(paths, Ok(service));
        app.settings.font = Some("DejaVu Sans".to_owned());

        let _ = app.update(Message::FontSaved {
            name: Some("Noto Sans CJK SC".to_owned()),
            result: Ok(()),
        });

        assert!(app.font_saving);
    }

    #[test]
    fn launch_failure_clears_state_and_shows_detail() {
        let mut app = app();
        let id = uuid::Uuid::new_v4();
        app.starting.insert(id);

        let _ = app.update(Message::LaunchFailed {
            game_id: id,
            error: "Game not found".to_string(),
        });

        assert!(!app.starting.contains(&id));
        let notice = app.error.expect("launch failure notice");
        assert_eq!(notice.title, "启动失败");
        assert_eq!(notice.detail.as_deref(), Some("Game not found"));
    }
}
