use super::{App, DeleteTarget, ErrorNotice, Message, Screen, SettingsAction};
use application::{LaunchEvent, LaunchRequest, LaunchTuning};
use iced::Task;

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
                    .map(crate::draft::GameDraft::from_game);
                self.prefix_status = None;
                self.screen = Screen::GameDetail;
                Task::batch([self.refresh_prefix_status(id), self.refresh_monitors()])
            }
            Message::NewGame => {
                self.draft = None;
                self.selected_game = None;
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickExe)
            }
            Message::Loaded(data) => {
                self.apply_loaded(data);
                Task::none()
            }
            Message::LoadFailed(error) => {
                self.pending.loading = false;
                self.error = Some(ErrorNotice::new("加载数据失败").with_detail(error));
                Task::none()
            }
            Message::Draft(action) => {
                let affects_prefix = matches!(
                    &action,
                    crate::draft::DraftAction::Runner(_)
                        | crate::draft::DraftAction::Prefix(_)
                        | crate::draft::DraftAction::ClearPrefix
                        | crate::draft::DraftAction::Arch(_)
                );
                let refresh_id = self.draft.as_ref().map(|draft| draft.id);
                if let Some(draft) = &mut self.draft {
                    draft.apply(action);
                }
                match (affects_prefix, refresh_id) {
                    (true, Some(id)) => {
                        self.prefix_status = None;
                        self.refresh_prefix_status(id)
                    }
                    _ => Task::none(),
                }
            }
            Message::SaveDraft => self.save_draft(),
            Message::DraftSaved(result) => self.apply_draft_saved(result),
            Message::Pick(intent) => crate::dialogs::pick(intent),
            Message::DialogResult { intent, path } => self.apply_dialog_result(intent, path),
            Message::DismissError => {
                self.error = None;
                Task::none()
            }
            Message::Error(error) => {
                self.error = Some(ErrorNotice::new(error));
                Task::none()
            }
            Message::DetectSystemWine => {
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new("数据库不可用"));
                    return Task::none();
                };
                self.pending.detecting_wine = true;
                Task::perform(service.detect_and_persist_runners(), |result| {
                    Message::WineDetected(result.map_err(|error| error.to_string()))
                })
            }
            Message::WineDetected(result) => {
                self.pending.detecting_wine = false;
                match result {
                    Ok(runners) => {
                        if runners.is_empty() {
                            self.error =
                                Some(ErrorNotice::new("未找到系统 Wine，请确认 wine 在 PATH 中"));
                        }
                        self.runners = runners;
                        self.runners
                            .sort_by_key(|runner| runner.name.to_lowercase());
                    }
                    Err(error) => {
                        self.error = Some(ErrorNotice::new("检测 Wine 失败").with_detail(error));
                    }
                }
                Task::none()
            }
            Message::ImportProton => {
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickProtonDir)
            }
            Message::ProtonImported(result) => {
                self.pending.importing_proton = false;
                match result {
                    Ok(runner) => {
                        self.runners.push(runner);
                        self.runners
                            .sort_by_key(|runner| runner.name.to_lowercase());
                    }
                    Err(error) => {
                        self.error = Some(ErrorNotice::new("导入 Proton 失败").with_detail(error));
                    }
                }
                Task::none()
            }
            Message::SetDefaultRunner(id) => {
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new("数据库不可用"));
                    return Task::none();
                };
                self.pending.settings_saving = true;
                Task::perform(service.save_default_runner(id), move |result| {
                    Message::DefaultRunnerSaved {
                        id,
                        result: result.map_err(|error| error.to_string()),
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
                        self.error =
                            Some(ErrorNotice::new("保存默认 Runner 失败").with_detail(error));
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
                    self.error = Some(ErrorNotice::new("请先保存游戏，再创建 Prefix"));
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
                Task::perform(application::create_prefix(plan, dxvk), move |result| {
                    Message::PrefixCreated {
                        game_id,
                        result: result.map_err(|error| error.to_string()),
                    }
                })
            }
            Message::PrefixCreated { game_id, result } => {
                self.pending.prefix.remove(&game_id);
                match result {
                    Ok(path) => {
                        if let Some(draft) = &mut self.draft {
                            if draft.id == game_id {
                                draft.prefix_path = Some(path);
                            }
                        }
                        return self.refresh_prefix_status(game_id);
                    }
                    Err(error) => {
                        self.error = Some(ErrorNotice::new("创建 Prefix 失败").with_detail(error));
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
                    self.error = Some(ErrorNotice::new("请先保存游戏，再安装 DXVK"));
                    return Task::none();
                }
                let Some(dxvk_dir) = self.dxvk_dir.clone() else {
                    self.error = Some(ErrorNotice::new("请先选择 DXVK 目录"));
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
                Task::perform(
                    application::install_dxvk(plan, application::DxvkSource { root: dxvk_dir }),
                    move |result| Message::DxvkInstalled {
                        game_id,
                        result: result.map_err(|error| error.to_string()),
                    },
                )
            }
            Message::DxvkInstalled { game_id, result } => {
                self.pending.dxvk.remove(&game_id);
                if let Err(error) = result {
                    self.error = Some(ErrorNotice::new("安装 DXVK 失败").with_detail(error));
                }
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
                        crate::draft::apply_env_action(&mut self.settings.global_env_rows, action);
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
                let values = match self.settings.to_map() {
                    Ok(values) => values,
                    Err(error) => {
                        self.error = Some(ErrorNotice::new("设置校验失败").with_detail(error));
                        return Task::none();
                    }
                };
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new("数据库不可用"));
                    return Task::none();
                };
                self.pending.settings_saving = true;
                Task::perform(service.save_settings(values), |result| {
                    Message::SettingsSaved(result.map_err(|error| error.to_string()))
                })
            }
            Message::SettingsSaved(result) => {
                self.pending.settings_saving = false;
                match result {
                    Ok(()) => {
                        self.error = None;
                    }
                    Err(error) => {
                        self.error = Some(ErrorNotice::new("保存设置失败").with_detail(error));
                    }
                }
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
                let Some(target) = self.confirming.clone() else {
                    return Task::none();
                };
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
                        self.error = Some(ErrorNotice::new(
                            "Prefix 变更尚未保存，请先保存游戏后再删除",
                        ));
                        return Task::none();
                    }
                }
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new("数据库不可用"));
                    return Task::none();
                };
                self.pending.deleting = true;
                let running = self.running.clone();
                let default_runner_id = self.settings.default_runner_id;
                Task::perform(
                    service.delete(target.clone(), running, default_runner_id),
                    move |result| Message::DeleteFinished {
                        target: target.clone(),
                        result: result.map_err(|error| error.to_string()),
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
                                if let Some(draft) = &mut self.draft {
                                    if draft.runner_id == Some(id) {
                                        draft.runner_id = None;
                                    }
                                }
                            }
                            DeleteTarget::Prefix { game_id, path } => {
                                if let Some(draft) = &mut self.draft {
                                    if draft.id == game_id {
                                        draft.prefix_path = None;
                                    }
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
                        self.error = Some(ErrorNotice::new("删除失败").with_detail(error));
                    }
                }
                Task::none()
            }
            Message::Launch(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(ErrorNotice::new("请先保存游戏，再启动"));
                    return Task::none();
                }
                if self.running.contains_key(&game_id) {
                    self.error = Some(ErrorNotice::new("该游戏已在运行"));
                    return Task::none();
                }
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(ErrorNotice::new(error));
                        return Task::none();
                    }
                };
                let prefix_plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return Task::none();
                    }
                };
                if self.running.values().any(|path| path == &prefix_plan.path) {
                    self.error = Some(ErrorNotice::new("同一 Prefix 已有游戏在运行，请等待其退出"));
                    return Task::none();
                }

                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new("数据库不可用"));
                    return Task::none();
                };
                let global_env = match crate::draft::env_rows_to_map(&self.settings.global_env_rows)
                {
                    Ok(env) => env,
                    Err(error) => {
                        self.error = Some(
                            ErrorNotice::new("全局环境变量无效").with_detail(error.to_string()),
                        );
                        return Task::none();
                    }
                };

                let request = LaunchRequest {
                    game,
                    runner: prefix_plan.runner.clone(),
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

                self.running.insert(game_id, prefix_plan.path);
                self.starting.insert(game_id);
                launch_task(service, request)
            }
            Message::LaunchEvent(event) => {
                match event {
                    LaunchEvent::Started { game_id } => {
                        self.starting.remove(&game_id);
                    }
                    LaunchEvent::Finished { game_id, outcome } => {
                        self.starting.remove(&game_id);
                        self.running.remove(&game_id);
                        self.stopping.remove(&game_id);
                        let timestamp = chrono::Utc::now().timestamp();
                        if let Some(game) = self.games.iter_mut().find(|game| game.id == game_id) {
                            game.last_played_at = Some(timestamp);
                        }
                        if let Some(draft) = &mut self.draft {
                            if draft.id == game_id {
                                draft.last_played_at = Some(timestamp);
                            }
                        }
                        if !outcome.success && !outcome.stopped {
                            self.error = Some(ErrorNotice::new(format!(
                                "游戏异常退出（退出码 {:?}）",
                                outcome.exit_code
                            )));
                        }
                    }
                    LaunchEvent::Failed { game_id, error } => {
                        self.starting.remove(&game_id);
                        self.running.remove(&game_id);
                        self.stopping.remove(&game_id);
                        self.error = Some(ErrorNotice::new("启动失败").with_detail(error));
                    }
                }
                Task::none()
            }
            Message::Stop(game_id) => {
                if !self.running.contains_key(&game_id) {
                    return Task::none();
                }
                let Some(service) = self.service.clone() else {
                    self.error = Some(ErrorNotice::new("数据库不可用"));
                    return Task::none();
                };
                self.stopping.insert(game_id);
                Task::perform(service.stop_game(game_id), move |result| {
                    Message::StopFinished {
                        game_id,
                        result: result.map_err(|error| error.to_string()),
                    }
                })
            }
            Message::StopFinished { game_id, result } => {
                if let Err(error) = result {
                    self.stopping.remove(&game_id);
                    self.error = Some(ErrorNotice::new("结束游戏失败").with_detail(error));
                }
                Task::none()
            }
            Message::RefreshMonitors => self.refresh_monitors(),
            Message::MonitorsLoaded(result) => {
                match result {
                    Ok(monitors) => self.monitors = monitors,
                    Err(error) => {
                        self.error = Some(ErrorNotice::new("扫描显示器失败").with_detail(error));
                    }
                }
                Task::none()
            }
            Message::OpenLogs(game_id) => {
                self.logs_game = Some(game_id);
                self.logs_content.clear();
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
                        Ok(content) => self.logs_content = content,
                        Err(error) => {
                            self.error = Some(ErrorNotice::new("读取日志失败").with_detail(error));
                        }
                    }
                }
                Task::none()
            }
        }
    }
}

fn launch_task(service: application::AppService, request: LaunchRequest) -> Task<Message> {
    Task::stream(iced::stream::channel(64, move |mut output| async move {
        use iced::futures::SinkExt;

        let (events, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let _ = service.launch_game_with_events(request, events).await;
        });

        while let Some(event) = receiver.recv().await {
            if output.send(Message::LaunchEvent(event)).await.is_err() {
                break;
            }
        }
    }))
}
