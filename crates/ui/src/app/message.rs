use super::{DeleteTarget, PrefixStatus, Screen};
use crate::theme::ThemeMode;
use application::Platform;
use application::editor::{DraftAction, EnvAction};
use application::{
    DllScanError, DllScanOutcome, Game, LaunchEvent, LoadedData, OverrideMode, Runner,
};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    SelectGame(uuid::Uuid),
    NewGame,
    BeginNewGame(Platform),
    CancelNewGame,
    Loaded(LoadedData),
    LoadFailed(String),
    Draft(DraftAction),
    SaveDraft,
    DraftSaved {
        game_id: uuid::Uuid,
        revision: u64,
        cover_changed: bool,
        result: Result<Game, String>,
    },
    CoverLoaded {
        game_id: Uuid,
        path: PathBuf,
        bytes: Option<Vec<u8>>,
    },
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
    SetDefaultRunner(Option<uuid::Uuid>),
    DefaultRunnerSaved {
        id: Option<uuid::Uuid>,
        result: Result<(), String>,
    },
    CreatePrefix(uuid::Uuid),
    PrefixCreated {
        game_id: uuid::Uuid,
        result: Result<PathBuf, String>,
    },
    InstallDxvk(uuid::Uuid),
    DxvkInstalled {
        game_id: uuid::Uuid,
        result: Result<(), String>,
    },
    ComponentPresetChanged(String),
    ComponentVerbChanged(String),
    InstallComponent {
        game_id: Uuid,
        verb: String,
    },
    ComponentEvent {
        game_id: Uuid,
        event: application::ComponentEvent,
    },
    ComponentFinished {
        game_id: Uuid,
        outcome: application::ComponentOutcome,
    },
    ComponentFailed {
        game_id: Uuid,
        error: String,
    },
    CancelComponent(Uuid),
    ComponentCancelFinished {
        game_id: Uuid,
        result: Result<(), String>,
    },
    ComponentsLoaded {
        prefix: PathBuf,
        verbs: Vec<String>,
    },
    RefreshPrefixStatus(uuid::Uuid),
    PrefixStatusLoaded {
        game_id: uuid::Uuid,
        status: PrefixStatus,
    },
    Settings(SettingsAction),
    SaveSettings,
    SettingsSaved(Result<(), String>),
    ThemeSaved {
        mode: crate::theme::ThemeMode,
        result: Result<(), String>,
    },
    LanguageSaved {
        pref: crate::i18n::LanguagePref,
        result: Result<(), String>,
    },
    FontPanelToggled,
    FontSearchChanged(String),
    FontCjkOnlyToggled(bool),
    FontSelected(Option<String>),
    FontsLoaded(Result<Vec<crate::fonts::FontEntry>, String>),
    ScanFonts,
    FontSaved {
        name: Option<String>,
        result: Result<(), String>,
    },
    WindowResized(f32),
    ConfirmDelete(DeleteTarget),
    CancelDelete,
    DeleteConfirmed,
    DeleteFinished {
        target: DeleteTarget,
        result: Result<(), String>,
    },
    Launch(uuid::Uuid),
    LaunchEvent(LaunchEvent),
    LaunchFailed {
        game_id: Uuid,
        error: String,
    },
    Stop(uuid::Uuid),
    StopFinished {
        game_id: uuid::Uuid,
        result: Result<(), String>,
    },
    RefreshMonitors,
    MonitorsLoaded(Result<Vec<application::MonitorOutput>, String>),
    OpenLogs(uuid::Uuid),
    RefreshLogs,
    LogsLoaded {
        game_id: uuid::Uuid,
        result: Result<String, String>,
    },
    OpenDllScan,
    CloseDllScan,
    DllScanPickDir,
    DllScanQueryChanged(String),
    DllScanToggle(String),
    DllScanSelectAll,
    DllScanInvert,
    DllScanClear,
    DllScanSortToggled,
    DllScanModeChanged(OverrideMode),
    DllScanRescan,
    DllScanCancelScan,
    DllScanFinished {
        seq: u64,
        result: Result<DllScanOutcome, DllScanError>,
    },
    DllScanConfirm,
    DllScanDirSaved(Result<(), String>),
    DismissSuccess,
    OpenHomepage,
    HomepageOpened(Result<(), String>),
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    Env(EnvAction),
    DefaultRunner(Option<uuid::Uuid>),
    PrefixRoot(PathBuf),
    EnableEsync(bool),
    EnableFsync(bool),
    EnableMangoHud(bool),
    AutoInstallDxvkNvapi(bool),
    DxvkNvapiSource(Option<PathBuf>),
    Theme(ThemeMode),
    Language(crate::i18n::LanguagePref),
}
