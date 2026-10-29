pub mod editor;

mod components;
mod data;
mod delete;
mod error;
mod games;
mod launching;
mod prefixes;
mod runners;
mod service;
mod settings;

pub use components::{
    ComponentEvent, ComponentOutcome, cancel_component, install_component, installed_components,
};
pub use data::LoadedData;
pub use delete::DeleteTarget;
pub use error::AppError;
pub use launching::{
    LaunchEvent, LaunchOutcome, LaunchRequest, LaunchTuning, validate_dll_name, validate_env_key,
};
pub use prefixes::{
    DxvkSource, PrefixPlan, PrefixStatus, create_prefix, install_dxvk, plan_for_game, prefix_status,
};
pub use service::AppService;
pub use settings::{
    SETTING_AUTO_INSTALL_DXVK_NVAPI, SETTING_DEFAULT_RUNNER_ID, SETTING_DXVK_NVAPI_SOURCE,
    SETTING_ENABLE_ESYNC, SETTING_ENABLE_FSYNC, SETTING_ENABLE_MANGOHUD, SETTING_GLOBAL_ENV,
    SETTING_PREFIX_ROOT, SETTING_UI_FONT, SETTING_UI_LANGUAGE, SETTING_UI_THEME,
};

pub use display::{MonitorOutput, SessionType, should_use_gamescope};
pub use launcher_core::{
    AppPaths, Arch, DisplayBackend, DisplayConfig, DllOverride, Game, GamescopeMode, OverrideMode,
    Runner, RunnerKind, init_logging,
};
pub use storage::ensure_app_dirs;
