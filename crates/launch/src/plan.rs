use display::SessionType;
use launcher_core::{Game, Runner};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub game: Game,
    pub runner: Runner,
    pub session: SessionType,
    pub global_env: BTreeMap<String, String>,
    pub inherited_env: BTreeMap<String, String>,
    pub steam_compat_client: PathBuf,
    pub default_prefix_root: PathBuf,
    pub gamescope_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}
