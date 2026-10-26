use crate::draft::{DraftAction, EnvRow, GameDraft};
use crate::tasks::{self, LoadedData};
use display::{SessionType, X11Output};
use launcher_core::{AppPaths, Game, Runner};
use prefix::{PrefixPlan, PrefixStatus};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use storage::{Db, StorageError};
use uuid::Uuid;

pub const SETTING_GLOBAL_ENV: &str = "global_env";
pub const SETTING_DEFAULT_RUNNER_ID: &str = "default_runner_id";
pub const SETTING_PREFIX_ROOT: &str = "prefix_root";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    GameDetail,
    RunnerManager,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Game(Uuid),
    Runner(Uuid),
    Prefix { game_id: Uuid, path: PathBuf },
}

impl DeleteTarget {
    pub fn label(&self) -> String {
        match self {
            Self::Game(id) => format!("游戏 {id}"),
            Self::Runner(id) => format!("Runner {id}"),
            Self::Prefix { path, .. } => format!("Prefix {}", path.display()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    Env(crate::draft::EnvAction),
    DefaultRunner(Option<Uuid>),
    PrefixRoot(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    pub title: String,
    pub detail: Option<String>,
}

impl AppError {
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
            launch::validate_env_key(key).map_err(|error| error.to_string())?;
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
    pub db: Option<Arc<Mutex<Db>>>,
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
    pub error: Option<AppError>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    SelectGame(Uuid),
    NewGame,
    Loaded(LoadedData),
    LoadFailed(String),
    Draft(DraftAction),
    SaveDraft,
    DraftSaved(Result<Game, String>),
    Pick(crate::dialogs::DialogIntent),
    DialogResult {
        intent: crate::dialogs::DialogIntent,
        path: Option<PathBuf>,
    },
    DismissError,
    Error(String),
    DetectSystemWine,
    WineDetected(Result<Vec<Runner>, String>),
    ImportProton,
    ProtonImported(Result<Runner, String>),
    SetDefaultRunner(Option<Uuid>),
    DefaultRunnerSaved {
        id: Option<Uuid>,
        result: Result<(), String>,
    },
    CreatePrefix(Uuid),
    PrefixCreated {
        game_id: Uuid,
        result: Result<PathBuf, String>,
    },
    InstallDxvk(Uuid),
    DxvkInstalled {
        game_id: Uuid,
        result: Result<(), String>,
    },
    RefreshPrefixStatus(Uuid),
    PrefixStatusLoaded {
        game_id: Uuid,
        status: PrefixStatus,
    },
    Settings(SettingsAction),
    SaveSettings,
    SettingsSaved(Result<(), String>),
    ConfirmDelete(DeleteTarget),
    CancelDelete,
    DeleteConfirmed,
    DeleteFinished {
        target: DeleteTarget,
        result: Result<(), String>,
    },
    Launch(Uuid),
    LaunchFinished {
        game_id: Uuid,
        result: Result<LaunchOutcome, String>,
    },
}

impl App {
    pub fn new(paths: AppPaths, db: Result<Db, StorageError>) -> (Self, iced::Task<Message>) {
        let settings = SettingsState::default_for(&paths.default_prefix_root);
        let mut app = Self {
            paths: paths.clone(),
            db: None,
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

        match db {
            Ok(db) => {
                let db = Arc::new(Mutex::new(db));
                app.db = Some(db.clone());
                let task = iced::Task::perform(tasks::load_initial(db), |result| match result {
                    Ok(data) => Message::Loaded(data),
                    Err(error) => Message::LoadFailed(error),
                });
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

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Navigate(screen) => {
                self.screen = screen;
                iced::Task::none()
            }
            Message::SelectGame(id) => {
                self.selected_game = Some(id);
                self.draft = self
                    .games
                    .iter()
                    .find(|game| game.id == id)
                    .map(GameDraft::from_game);
                self.prefix_status = None;
                self.screen = Screen::GameDetail;
                self.refresh_prefix_status(id)
            }
            Message::NewGame => {
                self.draft = None;
                self.selected_game = None;
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickExe)
            }
            Message::Loaded(data) => {
                self.apply_loaded(data);
                iced::Task::none()
            }
            Message::LoadFailed(error) => {
                self.pending.loading = false;
                self.error = Some(AppError::new("加载数据失败").with_detail(error));
                iced::Task::none()
            }
            Message::Draft(action) => {
                let affects_prefix = matches!(
                    &action,
                    DraftAction::Runner(_)
                        | DraftAction::Prefix(_)
                        | DraftAction::ClearPrefix
                        | DraftAction::Arch(_)
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
                    _ => iced::Task::none(),
                }
            }
            Message::SaveDraft => self.save_draft(),
            Message::DraftSaved(result) => self.apply_draft_saved(result),
            Message::Pick(intent) => crate::dialogs::pick(intent),
            Message::DialogResult { intent, path } => self.apply_dialog_result(intent, path),
            Message::DismissError => {
                self.error = None;
                iced::Task::none()
            }
            Message::Error(error) => {
                self.error = Some(AppError::new(error));
                iced::Task::none()
            }
            Message::DetectSystemWine => {
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.detecting_wine = true;
                iced::Task::perform(tasks::detect_and_persist(db), Message::WineDetected)
            }
            Message::WineDetected(result) => {
                self.pending.detecting_wine = false;
                match result {
                    Ok(runners) => {
                        if runners.is_empty() {
                            self.error =
                                Some(AppError::new("未找到系统 Wine，请确认 wine 在 PATH 中"));
                        }
                        self.runners = runners;
                        self.runners
                            .sort_by_key(|runner| runner.name.to_lowercase());
                    }
                    Err(error) => {
                        self.error = Some(AppError::new("检测 Wine 失败").with_detail(error));
                    }
                }
                iced::Task::none()
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
                        self.error = Some(AppError::new("导入 Proton 失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::SetDefaultRunner(id) => {
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.settings_saving = true;
                iced::Task::perform(tasks::save_default_runner(db, id), move |result| {
                    Message::DefaultRunnerSaved { id, result }
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
                        self.error = Some(AppError::new("保存默认 Runner 失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::CreatePrefix(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(AppError::new("请先保存游戏，再创建 Prefix"));
                    return iced::Task::none();
                }
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(AppError::new(error));
                        return iced::Task::none();
                    }
                };
                let plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return iced::Task::none();
                    }
                };
                let dxvk = self
                    .dxvk_dir
                    .clone()
                    .map(|root| prefix::DxvkSource { root });
                self.pending.prefix.insert(game_id);
                iced::Task::perform(tasks::create_prefix(plan, dxvk), move |result| {
                    Message::PrefixCreated { game_id, result }
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
                        self.error = Some(AppError::new("创建 Prefix 失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::InstallDxvk(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(AppError::new("请先保存游戏，再安装 DXVK"));
                    return iced::Task::none();
                }
                let Some(dxvk_dir) = self.dxvk_dir.clone() else {
                    self.error = Some(AppError::new("请先选择 DXVK 目录"));
                    return iced::Task::none();
                };
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(AppError::new(error));
                        return iced::Task::none();
                    }
                };
                let plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return iced::Task::none();
                    }
                };
                self.pending.dxvk.insert(game_id);
                iced::Task::perform(
                    tasks::install_dxvk(plan, prefix::DxvkSource { root: dxvk_dir }),
                    move |result| Message::DxvkInstalled { game_id, result },
                )
            }
            Message::DxvkInstalled { game_id, result } => {
                self.pending.dxvk.remove(&game_id);
                if let Err(error) = result {
                    self.error = Some(AppError::new("安装 DXVK 失败").with_detail(error));
                }
                iced::Task::none()
            }
            Message::RefreshPrefixStatus(game_id) => self.refresh_prefix_status(game_id),
            Message::PrefixStatusLoaded { game_id, status } => {
                if self.selected_game == Some(game_id) {
                    self.prefix_status = Some(status);
                }
                iced::Task::none()
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
                }
                match (affects_prefix, self.selected_game) {
                    (true, Some(id)) => {
                        self.prefix_status = None;
                        self.refresh_prefix_status(id)
                    }
                    _ => iced::Task::none(),
                }
            }
            Message::SaveSettings => {
                let values = match self.settings.to_map() {
                    Ok(values) => values,
                    Err(error) => {
                        self.error = Some(AppError::new("设置校验失败").with_detail(error));
                        return iced::Task::none();
                    }
                };
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.settings_saving = true;
                iced::Task::perform(tasks::save_settings(db, values), Message::SettingsSaved)
            }
            Message::SettingsSaved(result) => {
                self.pending.settings_saving = false;
                match result {
                    Ok(()) => {
                        self.error = None;
                    }
                    Err(error) => {
                        self.error = Some(AppError::new("保存设置失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::ConfirmDelete(target) => {
                self.confirming = Some(target);
                iced::Task::none()
            }
            Message::CancelDelete => {
                self.confirming = None;
                iced::Task::none()
            }
            Message::DeleteConfirmed => {
                let Some(target) = self.confirming.clone() else {
                    return iced::Task::none();
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
                        self.error =
                            Some(AppError::new("Prefix 变更尚未保存，请先保存游戏后再删除"));
                        return iced::Task::none();
                    }
                }
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.deleting = true;
                let running = self.running.clone();
                let default_runner_id = self.settings.default_runner_id;
                iced::Task::perform(
                    tasks::delete_target(db, target.clone(), running, default_runner_id),
                    move |result| Message::DeleteFinished {
                        target: target.clone(),
                        result,
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
                        self.error = Some(AppError::new("删除失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::Launch(_) | Message::LaunchFinished { .. } => iced::Task::none(),
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        if let Some(fatal) = &self.fatal {
            return crate::widgets::fatal(fatal);
        }

        let screen: iced::Element<'_, Message> = match self.screen {
            Screen::Library => crate::views::library::view(self),
            Screen::GameDetail => crate::views::game_detail::view(self),
            Screen::RunnerManager => crate::views::runners::view(self),
            Screen::Settings => crate::views::settings::view(self),
        };

        let mut content = match &self.error {
            Some(error) => iced::widget::column![crate::widgets::error_banner(error), screen]
                .spacing(8)
                .padding(8)
                .into(),
            None => screen,
        };

        if let Some(target) = &self.confirming {
            content = iced::widget::column![crate::widgets::confirm_bar(target), content]
                .spacing(8)
                .padding(8)
                .into();
        }

        content
    }

    fn apply_loaded(&mut self, data: LoadedData) {
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
                self.error = Some(AppError::new("设置读取失败").with_detail(error));
            }
        }

        if let Some(id) = self
            .settings
            .default_runner_id
            .filter(|id| !self.runners.iter().any(|runner| runner.id == *id))
        {
            self.settings.default_runner_id = None;
            self.error = Some(AppError::new(
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
                self.error = Some(AppError::new("表单校验失败").with_detail(error.to_string()));
                return iced::Task::none();
            }
        };
        let Some(db) = self.db.clone() else {
            self.error = Some(AppError::new("数据库不可用"));
            return iced::Task::none();
        };

        self.pending.saving = true;
        iced::Task::perform(
            tasks::save_game(db, game, draft.is_new),
            Message::DraftSaved,
        )
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
                self.error = Some(AppError::new("保存失败").with_detail(error));
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
                    if let Some(db) = self.db.clone() {
                        self.pending.importing_proton = true;
                        return iced::Task::perform(
                            tasks::import_proton(db, path),
                            Message::ProtonImported,
                        );
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

    fn prefix_plan_for(&self, game: &Game) -> Result<PrefixPlan, AppError> {
        let runner =
            launch::resolve_runner(game, &self.runners, self.settings.default_runner_id)
                .map_err(|error| AppError::new("Runner 不可用").with_detail(error.to_string()))?;

        Ok(prefix::plan_for_game(
            game,
            runner,
            &self.settings.prefix_root,
            self.paths.steam_compat_client.clone(),
        ))
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

        iced::Task::perform(tasks::prefix_status(plan), move |status| {
            Message::PrefixStatusLoaded { game_id, status }
        })
    }

    fn sort_games(&mut self) {
        self.games.sort_by_key(|game| game.name.to_lowercase());
    }
}
