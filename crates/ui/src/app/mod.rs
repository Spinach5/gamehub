mod message;
mod update;
mod view;

use crate::draft::{EnvRow, GameDraft};
use application::{
    AppPaths, AppService, Game, MonitorOutput, PrefixPlan, PrefixStatus, Runner, SessionType,
    validate_env_key,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub use application::{
    AppError as ServiceError, DeleteTarget, LaunchOutcome, SETTING_AUTO_INSTALL_DXVK_NVAPI,
    SETTING_DEFAULT_RUNNER_ID, SETTING_DXVK_NVAPI_SOURCE, SETTING_ENABLE_ESYNC,
    SETTING_ENABLE_FSYNC, SETTING_ENABLE_MANGOHUD, SETTING_GLOBAL_ENV, SETTING_PREFIX_ROOT,
};
pub use message::{Message, SettingsAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    GameDetail,
    RunnerManager,
    Settings,
    Logs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorNotice {
    pub title: String,
    pub detail: Option<String>,
}

impl ErrorNotice {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsState {
    pub global_env_rows: Vec<EnvRow>,
    pub default_runner_id: Option<Uuid>,
    pub prefix_root: PathBuf,
    pub enable_esync: bool,
    pub enable_fsync: bool,
    pub enable_mangohud: bool,
    pub auto_install_dxvk_nvapi: bool,
    pub dxvk_nvapi_source: Option<PathBuf>,
}

fn parse_bool(map: &BTreeMap<String, String>, key: &str, default: bool) -> Result<bool, String> {
    match map.get(key).map(String::as_str) {
        None => Ok(default),
        Some("true" | "1") => Ok(true),
        Some("false" | "0") => Ok(false),
        Some(_) => Err(format!("{key} 必须是 true/false 或 1/0")),
    }
}

impl SettingsState {
    pub fn default_for(default_prefix_root: &Path) -> Self {
        Self {
            global_env_rows: Vec::new(),
            default_runner_id: None,
            prefix_root: default_prefix_root.to_path_buf(),
            enable_esync: true,
            enable_fsync: true,
            enable_mangohud: false,
            auto_install_dxvk_nvapi: false,
            dxvk_nvapi_source: None,
        }
    }

    pub fn from_map(
        map: &BTreeMap<String, String>,
        default_prefix_root: &Path,
    ) -> Result<Self, String> {
        let global_env: BTreeMap<String, String> = match map.get(SETTING_GLOBAL_ENV) {
            Some(raw) => serde_json::from_str(raw)
                .map_err(|error| format!("global_env 不是合法的 JSON 对象: {error}"))?,
            None => BTreeMap::new(),
        };

        if global_env.contains_key("WINEDLLOVERRIDES") {
            return Err("global_env 不能包含 WINEDLLOVERRIDES，请使用结构化 DLL override".into());
        }
        for key in global_env.keys() {
            validate_env_key(key).map_err(|error| error.to_string())?;
        }

        let default_runner_id = match map.get(SETTING_DEFAULT_RUNNER_ID) {
            Some(raw) => Some(
                Uuid::parse_str(raw).map_err(|_| "default_runner_id 不是合法 UUID".to_string())?,
            ),
            None => None,
        };

        let prefix_root = map
            .get(SETTING_PREFIX_ROOT)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_prefix_root.to_path_buf());
        if !prefix_root.is_absolute() {
            return Err("prefix_root 必须是绝对路径".into());
        }

        let global_env_rows = global_env
            .iter()
            .map(|(key, value)| EnvRow {
                key: key.clone(),
                value: value.clone(),
            })
            .collect();

        Ok(Self {
            global_env_rows,
            default_runner_id,
            prefix_root,
            enable_esync: parse_bool(map, SETTING_ENABLE_ESYNC, true)?,
            enable_fsync: parse_bool(map, SETTING_ENABLE_FSYNC, true)?,
            enable_mangohud: parse_bool(map, SETTING_ENABLE_MANGOHUD, false)?,
            auto_install_dxvk_nvapi: parse_bool(map, SETTING_AUTO_INSTALL_DXVK_NVAPI, false)?,
            dxvk_nvapi_source: map
                .get(SETTING_DXVK_NVAPI_SOURCE)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
        })
    }

    pub fn to_map(&self) -> Result<BTreeMap<String, Option<String>>, String> {
        if !self.prefix_root.is_absolute() {
            return Err("prefix_root 必须是绝对路径".into());
        }
        let env = crate::draft::env_rows_to_map(&self.global_env_rows)
            .map_err(|error| error.to_string())?;
        let global_env = serde_json::to_string(&env)
            .map_err(|error| format!("global_env 序列化失败: {error}"))?;

        let mut map = BTreeMap::new();
        map.insert(SETTING_GLOBAL_ENV.to_string(), Some(global_env));
        let prefix_root = self
            .prefix_root
            .to_str()
            .ok_or_else(|| "prefix_root 不是 UTF-8 路径".to_string())?
            .to_string();
        map.insert(SETTING_PREFIX_ROOT.to_string(), Some(prefix_root));
        map.insert(
            SETTING_DEFAULT_RUNNER_ID.to_string(),
            self.default_runner_id.map(|id| id.to_string()),
        );
        map.insert(
            SETTING_ENABLE_ESYNC.to_string(),
            Some(self.enable_esync.to_string()),
        );
        map.insert(
            SETTING_ENABLE_FSYNC.to_string(),
            Some(self.enable_fsync.to_string()),
        );
        map.insert(
            SETTING_ENABLE_MANGOHUD.to_string(),
            Some(self.enable_mangohud.to_string()),
        );
        map.insert(
            SETTING_AUTO_INSTALL_DXVK_NVAPI.to_string(),
            Some(self.auto_install_dxvk_nvapi.to_string()),
        );
        map.insert(
            SETTING_DXVK_NVAPI_SOURCE.to_string(),
            self.dxvk_nvapi_source
                .as_ref()
                .map(|path| {
                    path.to_str()
                        .map(str::to_string)
                        .ok_or_else(|| "DXVK-NVAPI 目录不是 UTF-8 路径".to_string())
                })
                .transpose()?,
        );
        Ok(map)
    }
}

#[derive(Debug, Default)]
pub struct PendingState {
    pub loading: bool,
    pub saving: bool,
    pub prefix: HashSet<Uuid>,
    pub dxvk: HashSet<Uuid>,
    pub detecting_wine: bool,
    pub importing_proton: bool,
    pub deleting: bool,
    pub settings_saving: bool,
}

pub struct App {
    pub paths: AppPaths,
    pub service: Option<AppService>,
    pub fatal: Option<String>,
    pub screen: Screen,
    pub games: Vec<Game>,
    pub runners: Vec<Runner>,
    pub settings: SettingsState,
    pub selected_game: Option<Uuid>,
    pub draft: Option<GameDraft>,
    pub prefix_status: Option<PrefixStatus>,
    pub session: SessionType,
    pub monitors: Vec<MonitorOutput>,
    pub gamescope_path: Option<PathBuf>,
    pub mangohud_path: Option<PathBuf>,
    pub pending: PendingState,
    pub running: HashMap<Uuid, PathBuf>, // game id -> prefix path
    pub starting: HashSet<Uuid>,
    pub stopping: HashSet<Uuid>,
    pub dxvk_dir: Option<PathBuf>,
    pub logs_game: Option<Uuid>,
    pub logs_content: String,
    pub confirming: Option<DeleteTarget>,
    pub error: Option<ErrorNotice>,
}

impl App {
    pub fn new(
        paths: AppPaths,
        service: Result<AppService, ServiceError>,
    ) -> (Self, iced::Task<Message>) {
        let settings = SettingsState::default_for(&paths.default_prefix_root);
        let mut app = Self {
            paths: paths.clone(),
            service: None,
            fatal: None,
            screen: Screen::Library,
            games: Vec::new(),
            runners: Vec::new(),
            settings,
            selected_game: None,
            draft: None,
            prefix_status: None,
            session: SessionType::Unknown,
            monitors: Vec::new(),
            gamescope_path: None,
            mangohud_path: None,
            pending: PendingState {
                loading: true,
                ..PendingState::default()
            },
            running: HashMap::new(),
            starting: HashSet::new(),
            stopping: HashSet::new(),
            dxvk_dir: None,
            logs_game: None,
            logs_content: String::new(),
            confirming: None,
            error: None,
        };

        match service {
            Ok(service) => {
                let task =
                    iced::Task::perform(service.clone().load_initial(), |result| match result {
                        Ok(data) => Message::Loaded(data),
                        Err(error) => Message::LoadFailed(error.to_string()),
                    });
                app.service = Some(service);
                (app, task)
            }
            Err(error) => {
                app.pending.loading = false;
                app.fatal = Some(format!(
                    "无法打开数据库 {}: {error}",
                    app.paths.db_path.display()
                ));
                (app, iced::Task::none())
            }
        }
    }

    fn apply_loaded(&mut self, data: application::LoadedData) {
        self.pending.loading = false;
        self.games = data.games;
        self.runners = data.runners;
        self.session = data.session;
        self.monitors = data.monitors;
        self.gamescope_path = data.gamescope_path;
        self.mangohud_path = data.mangohud_path;

        match SettingsState::from_map(&data.settings_map, &self.paths.default_prefix_root) {
            Ok(settings) => self.settings = settings,
            Err(error) => {
                self.settings = SettingsState::default_for(&self.paths.default_prefix_root);
                self.error = Some(ErrorNotice::new("设置读取失败").with_detail(error));
            }
        }

        if let Some(id) = self
            .settings
            .default_runner_id
            .filter(|id| !self.runners.iter().any(|runner| runner.id == *id))
        {
            self.settings.default_runner_id = None;
            self.error = Some(ErrorNotice::new(
                "默认 Runner 不存在，已重置为未选择（保存设置后生效）",
            ));
            tracing::warn!(%id, "default runner is missing");
        }

        self.sort_games();
    }

    fn save_draft(&mut self) -> iced::Task<Message> {
        let Some(draft) = self.draft.clone() else {
            return iced::Task::none();
        };
        let game = match draft.to_game() {
            Ok(game) => game,
            Err(error) => {
                self.error = Some(ErrorNotice::new("表单校验失败").with_detail(error.to_string()));
                return iced::Task::none();
            }
        };
        let Some(service) = self.service.clone() else {
            self.error = Some(ErrorNotice::new("数据库不可用"));
            return iced::Task::none();
        };

        self.pending.saving = true;
        iced::Task::perform(service.save_game(game, draft.is_new), |result| {
            Message::DraftSaved(result.map_err(|error| error.to_string()))
        })
    }

    fn apply_draft_saved(&mut self, result: Result<Game, String>) -> iced::Task<Message> {
        self.pending.saving = false;
        match result {
            Ok(game) => {
                if let Some(existing) = self
                    .games
                    .iter_mut()
                    .find(|existing| existing.id == game.id)
                {
                    *existing = game.clone();
                } else {
                    self.games.push(game.clone());
                }
                self.sort_games();
                self.selected_game = Some(game.id);
                self.draft = Some(GameDraft::from_game(&game));
                self.error = None;
                return self.refresh_prefix_status(game.id);
            }
            Err(error) => {
                self.error = Some(ErrorNotice::new("保存失败").with_detail(error));
            }
        }
        iced::Task::none()
    }

    fn apply_dialog_result(
        &mut self,
        intent: crate::dialogs::DialogIntent,
        path: Option<PathBuf>,
    ) -> iced::Task<Message> {
        use crate::dialogs::DialogIntent;

        match intent {
            DialogIntent::PickExe => {
                if let Some(path) = path {
                    if let Some(draft) = &mut self.draft {
                        draft.executable_path = Some(path);
                    } else {
                        self.draft = Some(GameDraft::new(path));
                        self.selected_game = None;
                        self.screen = Screen::GameDetail;
                    }
                }
            }
            DialogIntent::PickWorkingDir => {
                if let (Some(draft), Some(path)) = (&mut self.draft, path) {
                    draft.working_dir = Some(path);
                }
            }
            DialogIntent::PickProtonDir => {
                if let Some(path) = path {
                    if let Some(service) = self.service.clone() {
                        self.pending.importing_proton = true;
                        return iced::Task::perform(service.import_proton(path), |result| {
                            Message::ProtonImported(result.map_err(|error| error.to_string()))
                        });
                    }
                }
            }
            DialogIntent::PickPrefixDir => {
                if let (Some(draft), Some(path)) = (&mut self.draft, path) {
                    draft.prefix_path = Some(path);
                    let id = draft.id;
                    self.prefix_status = None;
                    return self.refresh_prefix_status(id);
                }
            }
            DialogIntent::PickDxvkDir => {
                self.dxvk_dir = path;
            }
            DialogIntent::PickDxvkNvapiDir => {
                if let Some(path) = path {
                    self.settings.dxvk_nvapi_source = Some(path);
                }
            }
            DialogIntent::PickPrefixRoot => {
                if let Some(path) = path {
                    self.settings.prefix_root = path;
                    self.prefix_status = None;
                    if let Some(id) = self.selected_game {
                        return self.refresh_prefix_status(id);
                    }
                }
            }
        }
        iced::Task::none()
    }

    fn inherited_env() -> BTreeMap<String, String> {
        std::env::vars_os()
            .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
            .collect()
    }

    fn game_for_actions(&self, game_id: Uuid) -> Result<Game, String> {
        if let Some(draft) = &self.draft {
            if draft.id == game_id {
                return draft.to_game().map_err(|error| error.to_string());
            }
        }
        self.games
            .iter()
            .find(|game| game.id == game_id)
            .cloned()
            .ok_or_else(|| "游戏不存在".to_string())
    }

    fn prefix_plan_for(&self, game: &Game) -> Result<PrefixPlan, ErrorNotice> {
        application::plan_for_game(
            game,
            &self.runners,
            self.settings.default_runner_id,
            &self.settings.prefix_root,
            self.paths.steam_compat_client.clone(),
        )
        .map_err(|error| ErrorNotice::new("Runner 不可用").with_detail(error.to_string()))
    }

    fn refresh_prefix_status(&self, game_id: Uuid) -> iced::Task<Message> {
        let game = match self.game_for_actions(game_id) {
            Ok(game) => game,
            Err(_) => {
                return iced::Task::none();
            }
        };
        let plan = match self.prefix_plan_for(&game) {
            Ok(plan) => plan,
            Err(_) => {
                return iced::Task::none();
            }
        };

        iced::Task::perform(application::prefix_status(plan), move |status| {
            Message::PrefixStatusLoaded { game_id, status }
        })
    }

    fn sort_games(&mut self) {
        self.games.sort_by_key(|game| game.name.to_lowercase());
    }

    pub fn launch_state_label(&self, game_id: Uuid) -> Option<&'static str> {
        if self.starting.contains(&game_id) {
            Some("启动中")
        } else if self.running.contains_key(&game_id) {
            if self.stopping.contains(&game_id) {
                Some("结束中")
            } else {
                Some("游玩")
            }
        } else {
            None
        }
    }

    fn refresh_monitors(&self) -> iced::Task<Message> {
        let Some(service) = self.service.clone() else {
            return iced::Task::none();
        };
        iced::Task::perform(service.list_monitors(), |result| {
            Message::MonitorsLoaded(result.map_err(|error| error.to_string()))
        })
    }

    fn load_logs(&self, game_id: Uuid) -> iced::Task<Message> {
        iced::Task::perform(
            read_log_tail(self.paths.game_log_path(game_id)),
            move |result| Message::LogsLoaded { game_id, result },
        )
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        if self.screen == Screen::Logs {
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::RefreshLogs)
        } else {
            iced::Subscription::none()
        }
    }
}

async fn read_log_tail(path: PathBuf) -> Result<String, String> {
    match tokio::fs::read_to_string(&path).await {
        Ok(text) => Ok(tail_lines(&text, 1000)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut tail = lines[start..].join("\n");
    if !tail.is_empty() && text.ends_with('\n') {
        tail.push('\n');
    }
    tail
}

#[cfg(test)]
mod tests {
    use super::*;
    use application::LaunchEvent;

    fn settings_from(pairs: &[(&str, &str)]) -> Result<SettingsState, String> {
        let map = pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        SettingsState::from_map(&map, Path::new("/tmp/prefixes"))
    }

    fn app() -> App {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-ui-test"));
        let (app, _) = App::new(paths, Err(ServiceError::DatabaseLocked));
        app
    }

    #[test]
    fn settings_default_to_esync_and_fsync_enabled() {
        let settings = settings_from(&[]).unwrap();
        assert!(settings.enable_esync);
        assert!(settings.enable_fsync);
        assert!(!settings.enable_mangohud);
        assert!(!settings.auto_install_dxvk_nvapi);
        assert_eq!(settings.dxvk_nvapi_source, None);
    }

    #[test]
    fn settings_parse_boolean_spellings() {
        let settings = settings_from(&[
            (SETTING_ENABLE_ESYNC, "0"),
            (SETTING_ENABLE_FSYNC, "false"),
            (SETTING_ENABLE_MANGOHUD, "true"),
            (SETTING_AUTO_INSTALL_DXVK_NVAPI, "1"),
        ])
        .unwrap();
        assert!(!settings.enable_esync);
        assert!(!settings.enable_fsync);
        assert!(settings.enable_mangohud);
        assert!(settings.auto_install_dxvk_nvapi);
    }

    #[test]
    fn invalid_boolean_is_rejected() {
        assert!(settings_from(&[(SETTING_ENABLE_ESYNC, "yes")]).is_err());
    }

    #[test]
    fn settings_round_trip_preserves_tuning_fields() {
        let mut settings = SettingsState::default_for(Path::new("/tmp/prefixes"));
        settings.enable_esync = false;
        settings.enable_mangohud = true;
        settings.auto_install_dxvk_nvapi = true;
        settings.dxvk_nvapi_source = Some(PathBuf::from("/opt/nvapi"));

        let stored: BTreeMap<String, String> = settings
            .to_map()
            .unwrap()
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .collect();
        let parsed = SettingsState::from_map(&stored, Path::new("/tmp/prefixes")).unwrap();
        assert_eq!(parsed, settings);
    }

    #[test]
    fn launch_state_moves_from_starting_to_playing() {
        let mut app = app();
        let id = Uuid::new_v4();
        app.running.insert(id, PathBuf::from("/tmp/prefix"));
        app.starting.insert(id);
        assert_eq!(app.launch_state_label(id), Some("启动中"));

        let _ = app.update(Message::LaunchEvent(LaunchEvent::Started { game_id: id }));
        assert_eq!(app.launch_state_label(id), Some("游玩"));

        let _ = app.update(Message::LaunchEvent(LaunchEvent::Finished {
            game_id: id,
            outcome: LaunchOutcome {
                exit_code: Some(0),
                success: true,
                stopped: false,
            },
        }));
        assert_eq!(app.launch_state_label(id), None);
        assert!(!app.running.contains_key(&id));
        assert!(!app.starting.contains(&id));
        assert!(!app.stopping.contains(&id));
    }

    #[test]
    fn stopping_a_running_game_is_labelled() {
        let mut app = app();
        let id = Uuid::new_v4();
        app.running.insert(id, PathBuf::from("/tmp/prefix"));
        app.stopping.insert(id);

        assert_eq!(app.launch_state_label(id), Some("结束中"));

        let _ = app.update(Message::LaunchEvent(LaunchEvent::Finished {
            game_id: id,
            outcome: LaunchOutcome {
                exit_code: None,
                success: false,
                stopped: true,
            },
        }));
        assert_eq!(app.launch_state_label(id), None);
        assert!(app.error.is_none());
    }

    #[test]
    fn failed_launch_clears_running_state_and_reports_error() {
        let mut app = app();
        let id = Uuid::new_v4();
        app.running.insert(id, PathBuf::from("/tmp/prefix"));
        app.starting.insert(id);

        let _ = app.update(Message::LaunchEvent(LaunchEvent::Failed {
            game_id: id,
            error: "boom".into(),
        }));

        assert!(!app.starting.contains(&id));
        assert!(!app.running.contains_key(&id));
        assert_eq!(
            app.error.as_ref().and_then(|error| error.detail.as_deref()),
            Some("boom")
        );
    }

    #[test]
    fn tail_lines_keeps_only_the_requested_suffix() {
        assert_eq!(tail_lines("a\nb\nc\n", 2), "b\nc\n");
        assert_eq!(tail_lines("a\nb", 5), "a\nb");
        assert_eq!(tail_lines("", 10), "");
        assert_eq!(tail_lines("a\nb\nc", 0), "");
    }
}
