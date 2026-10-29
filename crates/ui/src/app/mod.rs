mod message;
mod update;
mod view;

use crate::i18n::{self, Lang, LanguagePref};
use crate::theme::ThemeMode;
use application::editor::{DraftAction, EnvRow, GameDraft, env_rows_to_map};
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
    SETTING_UI_LANGUAGE, SETTING_UI_THEME,
};
pub use message::{Message, SettingsAction};

pub const LOG_TAIL_LINES: usize = 1000;
pub const COMPONENT_TAIL_LINES: usize = 400;

/// An in-flight winetricks run. The plan is kept so the installed list can be
/// refreshed against the same prefix even if the draft changes meanwhile.
#[derive(Debug, Clone)]
pub struct ComponentJob {
    pub plan: PrefixPlan,
    pub verb: String,
    pub process_group: Option<u32>,
    pub cancelling: bool,
    pub tail: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    GameDetail,
    RunnerManager,
    Settings,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NoticeKind {
    #[default]
    General,
    Form,
    ThemeSave,
    LanguageSave,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorNotice {
    pub kind: NoticeKind,
    pub title: String,
    pub detail: Option<String>,
}

impl ErrorNotice {
    pub fn new(title: impl Into<String>) -> Self {
        Self::with_kind(NoticeKind::General, title)
    }

    pub fn form(title: impl Into<String>) -> Self {
        Self::with_kind(NoticeKind::Form, title)
    }

    pub fn theme_save(title: impl Into<String>) -> Self {
        Self::with_kind(NoticeKind::ThemeSave, title)
    }

    pub fn language_save(title: impl Into<String>) -> Self {
        Self::with_kind(NoticeKind::LanguageSave, title)
    }

    fn with_kind(kind: NoticeKind, title: impl Into<String>) -> Self {
        Self {
            kind,
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
    pub theme: ThemeMode,
    pub language: LanguagePref,
}

fn parse_bool(
    map: &BTreeMap<String, String>,
    key: &str,
    default: bool,
    lang: Lang,
) -> Result<bool, String> {
    match map.get(key).map(String::as_str) {
        None => Ok(default),
        Some("true" | "1") => Ok(true),
        Some("false" | "0") => Ok(false),
        Some(_) => Err(i18n::tr_args(
            lang,
            "settings.error.bad_bool",
            &[("key", key)],
        )),
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
            theme: ThemeMode::Light,
            language: LanguagePref::Auto,
        }
    }

    pub fn from_map(
        map: &BTreeMap<String, String>,
        default_prefix_root: &Path,
    ) -> Result<Self, String> {
        let pref = LanguagePref::from_setting(map.get(SETTING_UI_LANGUAGE).map(String::as_str));
        let lang = pref.resolve();
        let global_env: BTreeMap<String, String> = match map.get(SETTING_GLOBAL_ENV) {
            Some(raw) => serde_json::from_str(raw).map_err(|error| {
                i18n::tr_args(
                    lang,
                    "settings.error.global_env_json",
                    &[("error", &error.to_string())],
                )
            })?,
            None => BTreeMap::new(),
        };

        if global_env.contains_key("WINEDLLOVERRIDES") {
            return Err(i18n::tr(lang, "settings.error.global_env_override").into());
        }
        for key in global_env.keys() {
            validate_env_key(key).map_err(|error| error.to_string())?;
        }

        let default_runner_id =
            match map.get(SETTING_DEFAULT_RUNNER_ID) {
                Some(raw) => Some(Uuid::parse_str(raw).map_err(|_| {
                    i18n::tr(lang, "settings.error.default_runner_uuid").to_string()
                })?),
                None => None,
            };

        let prefix_root = map
            .get(SETTING_PREFIX_ROOT)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_prefix_root.to_path_buf());
        if !prefix_root.is_absolute() {
            return Err(i18n::tr(lang, "settings.error.prefix_root_absolute").into());
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
            enable_esync: parse_bool(map, SETTING_ENABLE_ESYNC, true, lang)?,
            enable_fsync: parse_bool(map, SETTING_ENABLE_FSYNC, true, lang)?,
            enable_mangohud: parse_bool(map, SETTING_ENABLE_MANGOHUD, false, lang)?,
            auto_install_dxvk_nvapi: parse_bool(map, SETTING_AUTO_INSTALL_DXVK_NVAPI, false, lang)?,
            dxvk_nvapi_source: map
                .get(SETTING_DXVK_NVAPI_SOURCE)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            theme: ThemeMode::from_setting(map.get(SETTING_UI_THEME).map(String::as_str)),
            language: pref,
        })
    }

    pub fn to_map(&self) -> Result<BTreeMap<String, Option<String>>, String> {
        let lang = self.language.resolve();
        if !self.prefix_root.is_absolute() {
            return Err(i18n::tr(lang, "settings.error.prefix_root_absolute").into());
        }
        let env = env_rows_to_map(&self.global_env_rows)
            .map_err(|error| crate::i18n::errors::draft_error(lang, &error))?;
        let global_env = serde_json::to_string(&env).map_err(|error| {
            i18n::tr_args(
                lang,
                "settings.error.global_env_serialize",
                &[("error", &error.to_string())],
            )
        })?;

        let mut map = BTreeMap::new();
        map.insert(SETTING_GLOBAL_ENV.to_string(), Some(global_env));
        let prefix_root = self
            .prefix_root
            .to_str()
            .ok_or_else(|| i18n::tr(lang, "settings.error.prefix_root_utf8").to_string())?
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
                        .ok_or_else(|| i18n::tr(lang, "settings.error.dxvk_dir_utf8").to_string())
                })
                .transpose()?,
        );
        map.insert(
            SETTING_UI_THEME.to_string(),
            Some(self.theme.as_setting().to_string()),
        );
        map.insert(
            SETTING_UI_LANGUAGE.to_string(),
            Some(self.language.as_setting().to_string()),
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
    pub theme_saving: bool,
    pub language_saving: bool,
    pub selected_game: Option<Uuid>,
    pub draft: Option<GameDraft>,
    pub draft_revision: u64,
    pub prefix_status: Option<PrefixStatus>,
    pub session: SessionType,
    pub monitors: Vec<MonitorOutput>,
    pub gamescope_path: Option<PathBuf>,
    pub mangohud_path: Option<PathBuf>,
    pub winetricks_path: Option<PathBuf>,
    pub pending: PendingState,
    pub running: HashMap<Uuid, PathBuf>, // game id -> prefix path
    pub starting: HashSet<Uuid>,
    pub stopping: HashSet<Uuid>,
    pub dxvk_dir: Option<PathBuf>,
    pub components: HashMap<Uuid, ComponentJob>,
    pub installed_verbs: HashMap<PathBuf, Vec<String>>,
    pub component_preset: String,
    pub component_verb: String,
    pub logs_game: Option<Uuid>,
    pub logs_content: String,
    pub logs_error: Option<String>,
    pub confirming: Option<DeleteTarget>,
    pub error: Option<ErrorNotice>,
    pub cover_handles: HashMap<Uuid, iced::widget::image::Handle>,
    pub window_width: f32,
    pub lang: Lang,
}

impl App {
    pub fn new(
        paths: AppPaths,
        service: Result<AppService, ServiceError>,
    ) -> (Self, iced::Task<Message>) {
        let settings = SettingsState::default_for(&paths.default_prefix_root);
        let lang = settings.language.resolve();
        let mut app = Self {
            paths: paths.clone(),
            service: None,
            fatal: None,
            screen: Screen::Library,
            games: Vec::new(),
            runners: Vec::new(),
            settings,
            theme_saving: false,
            language_saving: false,
            selected_game: None,
            draft: None,
            draft_revision: 0,
            prefix_status: None,
            session: SessionType::Unknown,
            monitors: Vec::new(),
            gamescope_path: None,
            mangohud_path: None,
            winetricks_path: None,
            pending: PendingState {
                loading: true,
                ..PendingState::default()
            },
            running: HashMap::new(),
            starting: HashSet::new(),
            stopping: HashSet::new(),
            dxvk_dir: None,
            components: HashMap::new(),
            installed_verbs: HashMap::new(),
            component_preset: "vcrun2019".to_string(),
            component_verb: String::new(),
            logs_game: None,
            logs_content: String::new(),
            logs_error: None,
            confirming: None,
            error: None,
            cover_handles: HashMap::new(),
            window_width: 1180.0,
            lang,
        };

        match service {
            Ok(service) => {
                let task =
                    iced::Task::perform(
                        service.clone().load_initial(),
                        move |result| match result {
                            Ok(data) => Message::Loaded(data),
                            Err(error) => {
                                Message::LoadFailed(crate::i18n::errors::app_error(lang, &error))
                            }
                        },
                    );
                app.service = Some(service);
                (app, task)
            }
            Err(error) => {
                app.pending.loading = false;
                app.fatal = Some(i18n::tr_args(
                    lang,
                    "app.fatal.database",
                    &[
                        ("path", &app.paths.db_path.display().to_string()),
                        ("error", &crate::i18n::errors::app_error(lang, &error)),
                    ],
                ));
                (app, iced::Task::none())
            }
        }
    }

    fn apply_loaded(&mut self, data: application::LoadedData) -> iced::Task<Message> {
        self.pending.loading = false;
        self.games = data.games;
        self.runners = data.runners;
        self.session = data.session;
        self.monitors = data.monitors;
        self.gamescope_path = data.gamescope_path;
        self.mangohud_path = data.mangohud_path;
        self.winetricks_path = data.winetricks_path;

        match SettingsState::from_map(&data.settings_map, &self.paths.default_prefix_root) {
            Ok(settings) => {
                self.lang = settings.language.resolve();
                self.settings = settings;
            }
            Err(error) => {
                let language = LanguagePref::from_setting(
                    data.settings_map
                        .get(SETTING_UI_LANGUAGE)
                        .map(String::as_str),
                );
                self.settings = SettingsState::default_for(&self.paths.default_prefix_root);
                self.settings.language = language;
                self.lang = language.resolve();
                self.error =
                    Some(ErrorNotice::form(self.tr("app.error.settings_load")).with_detail(error));
            }
        }

        if let Some(id) = self
            .settings
            .default_runner_id
            .filter(|id| !self.runners.iter().any(|runner| runner.id == *id))
        {
            self.settings.default_runner_id = None;
            self.error = Some(ErrorNotice::form(
                self.tr("app.warning.default_runner_missing"),
            ));
            tracing::warn!(%id, "default runner is missing");
        }

        self.sort_games();

        let tasks: Vec<_> = self
            .games
            .iter()
            .map(|game| Self::cover_task(game.id, game.cover_path.clone()))
            .collect();
        iced::Task::batch(tasks)
    }

    /// Reads a cover off the UI thread and reports back through `CoverLoaded`.
    fn cover_task(game_id: Uuid, path: Option<PathBuf>) -> iced::Task<Message> {
        let Some(path) = path else {
            return iced::Task::none();
        };
        let loaded_path = path.clone();
        iced::Task::perform(Self::read_cover_bytes(path), move |bytes| {
            Message::CoverLoaded {
                game_id,
                path: loaded_path.clone(),
                bytes,
            }
        })
    }

    async fn read_cover_bytes(path: PathBuf) -> Option<Vec<u8>> {
        tokio::fs::read(path).await.ok()
    }

    fn touch_draft(&mut self) {
        self.draft_revision = self.draft_revision.wrapping_add(1);
    }

    fn save_draft(&mut self) -> iced::Task<Message> {
        if self.pending.saving || self.pending.deleting {
            return iced::Task::none();
        }
        let Some(draft) = self.draft.clone() else {
            return iced::Task::none();
        };
        // Compare against the persisted game: `draft` may already reflect a
        // cleared cover, so comparing it with the derived game would miss it.
        let persisted_cover = self
            .games
            .iter()
            .find(|existing| existing.id == draft.id)
            .and_then(|existing| existing.cover_path.clone());
        let cover_changed = draft.cover_source.is_some() || draft.cover_path != persisted_cover;
        let game = match draft.to_game() {
            Ok(game) => game,
            Err(error) => {
                self.error = Some(
                    ErrorNotice::form(self.tr("app.error.form_validation"))
                        .with_detail(crate::i18n::errors::draft_error(self.lang, &error)),
                );
                return iced::Task::none();
            }
        };
        let Some(service) = self.service.clone() else {
            self.error = Some(ErrorNotice::new(self.tr("app.error.database_unavailable")));
            return iced::Task::none();
        };

        self.pending.saving = true;
        let game_id = game.id;
        let revision = self.draft_revision;
        let cover_source = draft.cover_source.clone();
        let covers_dir = self.paths.covers_dir.clone();
        let lang = self.lang;
        iced::Task::perform(
            service.save_game(game, draft.is_new, cover_source, covers_dir),
            move |result| Message::DraftSaved {
                game_id,
                revision,
                cover_changed,
                result: result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
            },
        )
    }

    fn apply_draft_saved(
        &mut self,
        game_id: Uuid,
        revision: u64,
        cover_changed: bool,
        result: Result<Game, String>,
    ) -> iced::Task<Message> {
        self.pending.saving = false;
        match result {
            Ok(game) => {
                // A save can race a running session: keep the session-accounted
                // fields monotonic instead of taking the draft's older values.
                let (merged_last_played, merged_playtime) = if let Some(existing) = self
                    .games
                    .iter_mut()
                    .find(|existing| existing.id == game.id)
                {
                    let latest_played = existing.last_played_at.max(game.last_played_at);
                    let latest_playtime = existing.playtime_seconds.max(game.playtime_seconds);
                    *existing = game.clone();
                    existing.last_played_at = latest_played;
                    existing.playtime_seconds = latest_playtime;
                    (latest_played, latest_playtime)
                } else {
                    self.games.push(game.clone());
                    (game.last_played_at, game.playtime_seconds)
                };
                let mut cover_load = iced::Task::none();
                if cover_changed {
                    if game.cover_path.is_none() {
                        self.cover_handles.remove(&game.id);
                    }
                    cover_load = Self::cover_task(game.id, game.cover_path.clone());
                } else if !self.cover_handles.contains_key(&game.id) {
                    cover_load = Self::cover_task(game.id, game.cover_path.clone());
                }
                self.sort_games();
                let current = self.draft.as_ref().is_some_and(|draft| draft.id == game_id)
                    && self.draft_revision == revision;
                if current {
                    self.selected_game = Some(game.id);
                    let mut draft = GameDraft::from_game(&game);
                    draft.last_played_at = merged_last_played;
                    draft.playtime_seconds = merged_playtime;
                    self.draft = Some(draft);
                    self.touch_draft();
                    self.error = None;
                    return iced::Task::batch([cover_load, self.refresh_prefix_status(game.id)]);
                }
                if self.selected_game == Some(game.id) {
                    return iced::Task::batch([cover_load, self.refresh_prefix_status(game.id)]);
                }
                cover_load
            }
            Err(error) => {
                let current = self.draft.as_ref().is_some_and(|draft| draft.id == game_id)
                    && self.draft_revision == revision;
                if current {
                    self.error =
                        Some(ErrorNotice::new(self.tr("app.error.save")).with_detail(error));
                }
                iced::Task::none()
            }
        }
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
                        let mut draft = GameDraft::new(path);
                        if draft.name.is_empty() {
                            draft.name = self.tr("draft.default_name").to_string();
                        }
                        self.draft = Some(draft);
                        self.selected_game = None;
                        self.screen = Screen::GameDetail;
                    }
                    self.touch_draft();
                }
            }
            DialogIntent::PickWorkingDir => {
                if let (Some(draft), Some(path)) = (&mut self.draft, path) {
                    draft.working_dir = Some(path);
                    self.touch_draft();
                }
            }
            DialogIntent::PickProtonDir => {
                if let Some(path) = path {
                    if let Some(service) = self.service.clone() {
                        self.pending.importing_proton = true;
                        let lang = self.lang;
                        return iced::Task::perform(service.import_proton(path), move |result| {
                            Message::ProtonImported(
                                result
                                    .map_err(|error| crate::i18n::errors::app_error(lang, &error)),
                            )
                        });
                    }
                }
                self.pending.importing_proton = false;
            }
            DialogIntent::PickPrefixDir => {
                if let (Some(draft), Some(path)) = (&mut self.draft, path) {
                    draft.prefix_path = Some(path);
                    let id = draft.id;
                    self.touch_draft();
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
            DialogIntent::PickCover => {
                if let (Some(draft), Some(path)) = (&mut self.draft, path) {
                    draft.apply(DraftAction::CoverSource(path));
                    self.touch_draft();
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
                return draft
                    .to_game()
                    .map_err(|error| crate::i18n::errors::draft_error(self.lang, &error));
            }
        }
        self.games
            .iter()
            .find(|game| game.id == game_id)
            .cloned()
            .ok_or_else(|| self.tr("errors.game_not_found").to_string())
    }

    fn prefix_plan_for(&self, game: &Game) -> Result<PrefixPlan, ErrorNotice> {
        application::plan_for_game(
            game,
            &self.runners,
            self.settings.default_runner_id,
            &self.settings.prefix_root,
            self.paths.steam_compat_client.clone(),
        )
        .map_err(|error| {
            ErrorNotice::new(self.tr("launch.runner_unavailable"))
                .with_detail(crate::i18n::errors::app_error(self.lang, &error))
        })
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

    fn refresh_installed_verbs(&self, game_id: Uuid) -> iced::Task<Message> {
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
        let prefix = plan.path.clone();

        iced::Task::perform(application::installed_components(plan), move |verbs| {
            Message::ComponentsLoaded {
                prefix: prefix.clone(),
                verbs,
            }
        })
    }

    fn sort_games(&mut self) {
        self.games.sort_by_key(|game| game.name.to_lowercase());
    }

    pub fn theme(&self) -> iced::Theme {
        self.settings.theme.theme()
    }

    pub fn tr(&self, key: &'static str) -> &'static str {
        i18n::tr(self.lang, key)
    }

    pub fn tr_args(&self, key: &'static str, args: &[(&str, &str)]) -> String {
        i18n::tr_args(self.lang, key, args)
    }

    pub fn tr_plural(&self, one: &'static str, other: &'static str, n: u64) -> &'static str {
        i18n::tr_plural(self.lang, one, other, n)
    }

    pub fn tr_plural_args(
        &self,
        one: &'static str,
        other: &'static str,
        n: u64,
        args: &[(&str, &str)],
    ) -> String {
        i18n::tr_plural_args(self.lang, one, other, n, args)
    }

    pub fn launch_state_label(&self, game_id: Uuid) -> Option<&'static str> {
        if self.starting.contains(&game_id) {
            Some(self.tr("launch.state.starting"))
        } else if self.running.contains_key(&game_id) {
            if self.stopping.contains(&game_id) {
                Some(self.tr("launch.state.stopping"))
            } else {
                Some(self.tr("launch.state.running"))
            }
        } else {
            None
        }
    }

    fn refresh_monitors(&self) -> iced::Task<Message> {
        let Some(service) = self.service.clone() else {
            return iced::Task::none();
        };
        let lang = self.lang;
        iced::Task::perform(service.list_monitors(), move |result| {
            Message::MonitorsLoaded(
                result.map_err(|error| crate::i18n::errors::app_error(lang, &error)),
            )
        })
    }

    fn load_logs(&self, game_id: Uuid) -> iced::Task<Message> {
        iced::Task::perform(
            read_log_tail(self.paths.game_log_path(game_id)),
            move |result| Message::LogsLoaded { game_id, result },
        )
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        let resized =
            iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size.width));

        if self.screen == Screen::Logs {
            iced::Subscription::batch([
                resized,
                iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::RefreshLogs),
            ])
        } else {
            resized
        }
    }
}

async fn read_log_tail(path: PathBuf) -> Result<String, String> {
    match tokio::fs::read_to_string(&path).await {
        Ok(text) => Ok(tail_lines(&text, LOG_TAIL_LINES)),
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
        let (mut app, _) = App::new(paths, Err(ServiceError::DatabaseLocked));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::ZH_CN);
        app.lang = &crate::i18n::ZH_CN;
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
    fn settings_default_theme_is_light() {
        let settings = settings_from(&[]).unwrap();
        assert_eq!(settings.theme, ThemeMode::Light);
    }

    #[test]
    fn settings_parse_theme_with_fallback() {
        assert_eq!(
            settings_from(&[(SETTING_UI_THEME, "dark")]).unwrap().theme,
            ThemeMode::Dark
        );
        assert_eq!(
            settings_from(&[(SETTING_UI_THEME, "nope")]).unwrap().theme,
            ThemeMode::Light
        );
    }

    #[test]
    fn settings_default_language_is_auto() {
        let settings = settings_from(&[]).unwrap();
        assert_eq!(settings.language, crate::i18n::LanguagePref::Auto);
    }

    #[test]
    fn settings_parse_language_with_fallback() {
        assert_eq!(
            settings_from(&[(SETTING_UI_LANGUAGE, "en-US")])
                .unwrap()
                .language,
            crate::i18n::LanguagePref::Fixed(&crate::i18n::EN_US)
        );
        assert_eq!(
            settings_from(&[(SETTING_UI_LANGUAGE, "nope")])
                .unwrap()
                .language,
            crate::i18n::LanguagePref::Auto
        );
    }

    #[test]
    fn settings_errors_use_selected_language() {
        let error = settings_from(&[
            (SETTING_UI_LANGUAGE, "en-US"),
            (SETTING_ENABLE_ESYNC, "yes"),
        ])
        .unwrap_err();
        assert!(error.contains("must be true/false"), "unexpected: {error}");
    }

    #[test]
    fn apply_loaded_keeps_stored_language_when_settings_fail() {
        let mut app = app();
        let data = application::LoadedData {
            games: Vec::new(),
            runners: Vec::new(),
            settings_map: BTreeMap::from([
                (SETTING_UI_LANGUAGE.to_string(), "en-US".to_string()),
                (SETTING_ENABLE_ESYNC.to_string(), "yes".to_string()),
            ]),
            session: application::SessionType::Unknown,
            monitors: Vec::new(),
            gamescope_path: None,
            mangohud_path: None,
            winetricks_path: None,
        };

        let _ = app.apply_loaded(data);

        assert_eq!(app.lang, &crate::i18n::EN_US);
        assert_eq!(
            app.settings.language,
            crate::i18n::LanguagePref::Fixed(&crate::i18n::EN_US)
        );
        let detail = app
            .error
            .expect("settings load failure should surface a notice")
            .detail
            .expect("notice should carry the parser error");
        assert!(
            detail.contains("must be true/false"),
            "unexpected: {detail}"
        );
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
        settings.theme = ThemeMode::Dark;

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
    fn settings_round_trip_preserves_language() {
        let mut settings = SettingsState::default_for(Path::new("/tmp/prefixes"));
        settings.language = crate::i18n::LanguagePref::Fixed(&crate::i18n::EN_US);

        let stored: BTreeMap<String, String> = settings
            .to_map()
            .unwrap()
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .collect();
        assert_eq!(stored[SETTING_UI_LANGUAGE], "en-US");
        assert_eq!(
            SettingsState::from_map(&stored, Path::new("/tmp/prefixes"))
                .unwrap()
                .language,
            settings.language
        );
    }

    #[test]
    fn launch_state_moves_from_starting_to_playing() {
        let mut app = app();
        let game = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"))
            .to_game()
            .unwrap();
        let id = game.id;
        app.games.push(game);
        app.running.insert(id, PathBuf::from("/tmp/prefix"));
        app.starting.insert(id);
        assert_eq!(app.launch_state_label(id), Some("启动中"));

        let _ = app.update(Message::LaunchEvent(LaunchEvent::Started {
            game_id: id,
            started_at: 1_700_000_000,
        }));
        assert_eq!(app.launch_state_label(id), Some("游玩"));
        assert_eq!(app.games[0].last_played_at, Some(1_700_000_000));

        let _ = app.update(Message::LaunchEvent(LaunchEvent::Finished {
            game_id: id,
            outcome: LaunchOutcome {
                exit_code: Some(0),
                success: true,
                stopped: false,
                session_seconds: 90,
            },
        }));
        assert_eq!(app.launch_state_label(id), None);
        assert!(!app.running.contains_key(&id));
        assert!(!app.starting.contains(&id));
        assert!(!app.stopping.contains(&id));
        assert_eq!(app.games[0].playtime_seconds, 90);
    }

    #[test]
    fn saved_draft_does_not_lower_session_totals() {
        let mut app = app();
        let mut game = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"))
            .to_game()
            .unwrap();
        game.playtime_seconds = 600;
        game.last_played_at = Some(1_700_000_600);
        app.games.push(game.clone());

        let mut stale = game.clone();
        stale.playtime_seconds = 120;
        stale.last_played_at = Some(1_700_000_120);
        let _ = app.apply_draft_saved(game.id, 0, false, Ok(stale));

        assert_eq!(app.games[0].playtime_seconds, 600);
        assert_eq!(app.games[0].last_played_at, Some(1_700_000_600));
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
                session_seconds: 0,
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
    fn logs_error_clears_stale_content() {
        let mut app = app();
        let id = Uuid::new_v4();
        app.logs_game = Some(id);
        app.logs_content = "old line".into();
        let _ = app.update(Message::LogsLoaded {
            game_id: id,
            result: Err("boom".into()),
        });
        assert!(app.logs_content.is_empty());
        assert_eq!(app.logs_error.as_deref(), Some("boom"));
    }

    #[test]
    fn logs_success_clears_error() {
        let mut app = app();
        let id = Uuid::new_v4();
        app.logs_game = Some(id);
        app.logs_error = Some("boom".into());
        let _ = app.update(Message::LogsLoaded {
            game_id: id,
            result: Ok("new line".into()),
        });
        assert_eq!(app.logs_content, "new line");
        assert!(app.logs_error.is_none());
    }

    #[test]
    fn tail_lines_keeps_only_the_requested_suffix() {
        assert_eq!(tail_lines("a\nb\nc\n", 2), "b\nc\n");
        assert_eq!(tail_lines("a\nb", 5), "a\nb");
        assert_eq!(tail_lines("", 10), "");
        assert_eq!(tail_lines("a\nb\nc", 0), "");
    }

    #[test]
    fn stale_draft_save_keeps_current_draft() {
        let mut app = app();
        let id = Uuid::new_v4();

        let mut draft = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"));
        draft.id = id;
        draft.name = "Edited".into();
        app.draft = Some(draft);
        app.selected_game = Some(id);
        app.draft_revision = 7;

        let mut saved = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"));
        saved.id = id;
        saved.name = "Saved".into();
        let game = saved.to_game().unwrap();

        // Completion from a save started at revision 6 must not replace the
        // draft the user has edited since (revision 7).
        let _ = app.apply_draft_saved(id, 6, false, Ok(game));

        assert_eq!(
            app.draft.as_ref().map(|draft| draft.name.as_str()),
            Some("Edited")
        );
        assert_eq!(app.draft_revision, 7);
        assert_eq!(app.selected_game, Some(id));
        assert_eq!(app.games.len(), 1);
        assert_eq!(app.games[0].name, "Saved");

        // A completion matching the current revision does replace the draft.
        let revision = app.draft_revision;
        let mut saved = app.games[0].clone();
        saved.name = "Saved again".into();
        let _ = app.apply_draft_saved(id, revision, false, Ok(saved));

        assert_eq!(
            app.draft.as_ref().map(|draft| draft.name.as_str()),
            Some("Saved again")
        );
        assert_eq!(app.draft_revision, revision.wrapping_add(1));
    }

    #[test]
    fn cleared_cover_drops_cached_handle() {
        let mut app = app();
        let id = Uuid::new_v4();

        let mut draft = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"));
        draft.id = id;
        draft.name = "Before".into();
        app.draft = Some(draft);
        app.selected_game = Some(id);
        app.draft_revision = 3;
        app.cover_handles
            .insert(id, iced::widget::image::Handle::from_bytes(vec![1, 2, 3]));

        let mut saved = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"));
        saved.id = id;
        saved.name = "After".into();
        saved.cover_path = None;
        let game = saved.to_game().unwrap();

        let _ = app.apply_draft_saved(id, 3, true, Ok(game));

        assert_eq!(
            app.draft.as_ref().map(|draft| draft.name.as_str()),
            Some("After")
        );
        assert_eq!(app.selected_game, Some(id));
        assert!(!app.cover_handles.contains_key(&id));
    }

    #[test]
    fn cover_loaded_ignores_stale_paths_and_installs_fresh_bytes() {
        let mut app = app();
        let id = Uuid::new_v4();
        let mut game = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"))
            .to_game()
            .unwrap();
        game.id = id;
        game.cover_path = Some(PathBuf::from("/tmp/gamehub-ui-test/cover.png"));
        app.games.push(game);

        let _ = app.update(Message::CoverLoaded {
            game_id: id,
            path: PathBuf::from("/tmp/gamehub-ui-test/replaced.png"),
            bytes: Some(vec![1, 2, 3]),
        });
        assert!(!app.cover_handles.contains_key(&id));

        let _ = app.update(Message::CoverLoaded {
            game_id: id,
            path: PathBuf::from("/tmp/gamehub-ui-test/cover.png"),
            bytes: Some(vec![1, 2, 3]),
        });
        assert!(app.cover_handles.contains_key(&id));
    }

    /// A 3x4 RGB PNG; `save_game` validates the format before importing it.
    const COVER_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 3, 0, 0, 0, 4, 8, 2,
        0, 0, 0, 196, 79, 18, 80, 0, 0, 0, 42, 73, 68, 65, 84, 120, 218, 13, 198, 49, 1, 0, 48, 12,
        195, 48, 195, 9, 136, 130, 8, 156, 158, 6, 85, 128, 155, 46, 1, 6, 139, 48, 102, 236, 252,
        173, 89, 187, 127, 103, 206, 158, 15, 199, 0, 12, 25, 54, 222, 103, 1, 0, 0, 0, 0, 73, 69,
        78, 68, 174, 66, 96, 130,
    ];

    /// Drives a task to completion the way the iced runtime would, without a
    /// window: every action it emits must be a plain message output.
    async fn run_task(task: iced::Task<Message>) -> Vec<Message> {
        use iced_runtime::futures::futures::StreamExt;

        let mut messages = Vec::new();
        if let Some(stream) = iced_runtime::task::into_stream(task) {
            let mut stream = std::pin::pin!(stream);
            while let Some(action) = stream.next().await {
                match action {
                    iced_runtime::Action::Output(message) => messages.push(message),
                    _ => panic!("task emitted a runtime action that needs a live iced runtime"),
                }
            }
        }
        messages
    }

    #[tokio::test]
    async fn saved_cover_bytes_travel_through_the_task_stream() {
        let root = std::env::temp_dir().join(format!("gamehub-cover-{}", Uuid::new_v4()));
        let paths = AppPaths::for_root(root.clone());
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(paths.covers_dir.as_path()).unwrap();
        let exe = root.join("game.exe");
        std::fs::write(&exe, b"stub").unwrap();
        let source = root.join("night.png");
        std::fs::write(&source, COVER_PNG).unwrap();

        let service = AppService::open_in_memory().unwrap();
        let (mut app, _) = App::new(paths.clone(), Ok(service));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::ZH_CN);
        app.lang = &crate::i18n::ZH_CN;

        let mut draft = GameDraft::new(exe);
        draft.name = "Cover Test".into();
        draft.cover_source = Some(source);
        app.draft = Some(draft);

        let saved = run_task(app.save_draft()).await;
        let [saved] = saved.as_slice() else {
            panic!(
                "expected exactly one DraftSaved message, got {}",
                saved.len()
            );
        };
        let Message::DraftSaved {
            game_id,
            revision,
            cover_changed,
            result,
        } = saved.clone()
        else {
            panic!("expected DraftSaved, got {saved:?}");
        };
        assert!(cover_changed);
        let game = result.expect("save succeeds");
        let stored = paths.covers_dir.join(format!("{game_id}.png"));
        assert_eq!(game.cover_path.as_deref(), Some(stored.as_path()));

        let messages =
            run_task(app.apply_draft_saved(game_id, revision, cover_changed, Ok(game))).await;
        let loaded = messages
            .into_iter()
            .find_map(|message| match message {
                Message::CoverLoaded {
                    game_id,
                    path,
                    bytes,
                } => Some((game_id, path, bytes)),
                _ => None,
            })
            .expect("a cover-loaded message follows the save");
        assert_eq!(loaded.0, game_id);
        assert_eq!(loaded.1, stored);
        assert_eq!(loaded.2.as_deref(), Some(COVER_PNG));
        assert!(!app.cover_handles.contains_key(&game_id));

        let _ = app.update(Message::CoverLoaded {
            game_id: loaded.0,
            path: loaded.1,
            bytes: loaded.2,
        });
        assert!(app.cover_handles.contains_key(&game_id));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn initial_load_streams_cover_bytes_for_stored_games() {
        let root = std::env::temp_dir().join(format!("gamehub-cover-load-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let cover = root.join("stored.png");
        std::fs::write(&cover, COVER_PNG).unwrap();

        let mut app = app();
        let mut game = GameDraft::new(PathBuf::from("/tmp/gamehub-ui-test/game.exe"))
            .to_game()
            .unwrap();
        game.cover_path = Some(cover.clone());
        let id = game.id;

        let messages = run_task(app.apply_loaded(application::LoadedData {
            games: vec![game],
            runners: Vec::new(),
            settings_map: BTreeMap::new(),
            session: application::SessionType::Unknown,
            monitors: Vec::new(),
            gamescope_path: None,
            mangohud_path: None,
            winetricks_path: None,
        }))
        .await;

        let loaded = messages
            .into_iter()
            .find_map(|message| match message {
                Message::CoverLoaded {
                    game_id,
                    path,
                    bytes,
                } => Some((game_id, path, bytes)),
                _ => None,
            })
            .expect("startup loading reads stored covers through the task stream");
        assert_eq!(loaded.0, id);
        assert_eq!(loaded.1, cover);
        assert_eq!(loaded.2.as_deref(), Some(COVER_PNG));
        assert!(app.cover_handles.is_empty());

        let _ = app.update(Message::CoverLoaded {
            game_id: loaded.0,
            path: loaded.1,
            bytes: loaded.2,
        });
        assert!(app.cover_handles.contains_key(&id));

        let _ = std::fs::remove_dir_all(&root);
    }
}
