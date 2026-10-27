use super::{DeleteTarget, PrefixStatus, Screen};
use crate::draft::DraftAction;
use application::{Game, LaunchEvent, LoadedData, Runner};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    SelectGame(uuid::Uuid),
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
    RefreshPrefixStatus(uuid::Uuid),
    PrefixStatusLoaded {
        game_id: uuid::Uuid,
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
    Launch(uuid::Uuid),
    LaunchEvent(LaunchEvent),
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
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    Env(crate::draft::EnvAction),
    DefaultRunner(Option<uuid::Uuid>),
    PrefixRoot(PathBuf),
    EnableEsync(bool),
    EnableFsync(bool),
    EnableMangoHud(bool),
    AutoInstallDxvkNvapi(bool),
    DxvkNvapiSource(Option<PathBuf>),
}
