use crate::draft::{DraftAction, EnvRow, GameDraft};
use crate::tasks::{self, LoadedData};
use display::{SessionType, X11Output};
use launcher_core::{AppPaths, Game, Runner};
use prefix::PrefixStatus;
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
    pub running: HashMap<Uuid, PathBuf>,
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
                iced::Task::none()
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
                if let Some(draft) = &mut self.draft {
                    draft.apply(action);
                }
                iced::Task::none()
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

        match &self.error {
            Some(error) => iced::widget::column![crate::widgets::error_banner(error), screen]
                .spacing(8)
                .padding(8)
                .into(),
            None => screen,
        }
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
        }
        iced::Task::none()
    }

    fn sort_games(&mut self) {
        self.games.sort_by_key(|game| game.name.to_lowercase());
    }
}
