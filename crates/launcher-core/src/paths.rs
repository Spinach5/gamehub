use crate::AppPathsError;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub data_root: PathBuf,
    pub db_path: PathBuf,
    pub logs_dir: PathBuf,
    pub covers_dir: PathBuf,
    pub default_prefix_root: PathBuf,
    pub steam_compat_client: PathBuf,
}

impl AppPaths {
    pub fn for_root(data_root: PathBuf) -> Self {
        Self {
            db_path: data_root.join("gamehub.db"),
            logs_dir: data_root.join("logs"),
            covers_dir: data_root.join("covers"),
            default_prefix_root: data_root.join("prefixes"),
            steam_compat_client: data_root.join("steam-compat-client"),
            data_root,
        }
    }

    pub fn from_values(
        xdg_data_home: Option<&Path>,
        home: Option<&Path>,
    ) -> Result<Self, AppPathsError> {
        let xdg = xdg_data_home.filter(|p| !p.as_os_str().is_empty());
        let root = match (xdg, home) {
            (Some(xdg), _) => xdg.to_path_buf(),
            (None, Some(home)) => home.join(".local/share"),
            (None, None) => return Err(AppPathsError::NoDataDir),
        };
        Ok(Self::for_root(root.join("gamehub")))
    }

    pub fn discover() -> Result<Self, AppPathsError> {
        let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
        let home = std::env::var_os("HOME").map(PathBuf::from);
        Self::from_values(xdg.as_deref(), home.as_deref())
    }

    pub fn game_log_path(&self, game_id: uuid::Uuid) -> PathBuf {
        self.logs_dir.join("games").join(format!("{game_id}.log"))
    }

    pub fn component_log_path(&self, game_id: uuid::Uuid) -> PathBuf {
        self.logs_dir
            .join("winetricks")
            .join(format!("{game_id}.log"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn xdg_data_home_wins() {
        let paths =
            AppPaths::from_values(Some(Path::new("/xdg")), Some(Path::new("/home/me"))).unwrap();
        assert_eq!(paths.data_root, PathBuf::from("/xdg/gamehub"));
        assert_eq!(paths.db_path, PathBuf::from("/xdg/gamehub/gamehub.db"));
    }

    #[test]
    fn home_fallback_is_used_when_xdg_is_empty() {
        let paths =
            AppPaths::from_values(Some(Path::new("")), Some(Path::new("/home/me"))).unwrap();
        assert_eq!(
            paths.data_root,
            PathBuf::from("/home/me/.local/share/gamehub")
        );
    }

    #[test]
    fn missing_everything_is_an_error() {
        assert_eq!(
            AppPaths::from_values(None, None),
            Err(AppPathsError::NoDataDir)
        );
    }

    #[test]
    fn derived_paths_are_stable() {
        let paths = AppPaths::for_root(PathBuf::from("/data/gamehub"));
        assert_eq!(paths.logs_dir, PathBuf::from("/data/gamehub/logs"));
        assert_eq!(paths.covers_dir, PathBuf::from("/data/gamehub/covers"));
        assert_eq!(
            paths.default_prefix_root,
            PathBuf::from("/data/gamehub/prefixes")
        );
        assert_eq!(
            paths.steam_compat_client,
            PathBuf::from("/data/gamehub/steam-compat-client")
        );
    }

    #[test]
    fn game_log_path_is_under_logs_games() {
        let paths = AppPaths::for_root(PathBuf::from("/data/gamehub"));
        assert_eq!(
            paths.game_log_path(uuid::Uuid::nil()),
            PathBuf::from("/data/gamehub/logs/games/00000000-0000-0000-0000-000000000000.log")
        );
    }

    #[test]
    fn component_log_path_is_under_logs_winetricks() {
        let paths = AppPaths::for_root(PathBuf::from("/data/gamehub"));
        assert_eq!(
            paths.component_log_path(uuid::Uuid::nil()),
            PathBuf::from("/data/gamehub/logs/winetricks/00000000-0000-0000-0000-000000000000.log")
        );
    }
}
