mod message;
mod update;
mod view;

use crate::draft::{EnvRow, GameDraft};
use application::{
    AppPaths, AppService, Game, PrefixPlan, PrefixStatus, Runner, SessionType, X11Output,
    validate_env_key,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub use application::{
    AppError as ServiceError, DeleteTarget, LaunchOutcome, SETTING_DEFAULT_RUNNER_ID,
    SETTING_GLOBAL_ENV, SETTING_PREFIX_ROOT,
};
pub use message::{Message, SettingsAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    GameDetail,
    RunnerManager,
    Settings,
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
}

impl SettingsState {
    pub fn default_for(default_prefix_root: &Path) -> Self {
        Self {
            global_env_rows: Vec::new(),
            default_runner_id: None,
            prefix_root: default_prefix_root.to_path_buf(),
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
    pub x11_outputs: Vec<X11Output>,
    pub gamescope_path: Option<PathBuf>,
    pub pending: PendingState,
    pub running: HashMap<Uuid, PathBuf>, // game id -> prefix path
    pub dxvk_dir: Option<PathBuf>,
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
            x11_outputs: Vec::new(),
            gamescope_path: None,
            pending: PendingState {
                loading: true,
                ..PendingState::default()
            },
            running: HashMap::new(),
            dxvk_dir: None,
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
        self.x11_outputs = data.x11_outputs;
        self.gamescope_path = data.gamescope_path;

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
}
