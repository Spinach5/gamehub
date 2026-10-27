mod data;
mod delete;
mod error;
mod games;
mod launching;
mod prefixes;
mod runners;
mod service;
mod settings;

pub use data::LoadedData;
pub use delete::DeleteTarget;
pub use error::AppError;
pub use launching::{LaunchOutcome, LaunchRequest, validate_dll_name, validate_env_key};
pub use prefixes::{
    DxvkSource, PrefixPlan, PrefixStatus, create_prefix, install_dxvk, plan_for_game, prefix_status,
};
pub use service::AppService;
pub use settings::{SETTING_DEFAULT_RUNNER_ID, SETTING_GLOBAL_ENV, SETTING_PREFIX_ROOT};

pub use display::{SessionType, X11Output, should_use_gamescope};
pub use launcher_core::{AppPaths, Game, Runner};
pub use storage::ensure_app_dirs;
