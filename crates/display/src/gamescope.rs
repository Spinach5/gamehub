use launcher_core::{DisplayConfig, GamescopeMode};
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::SessionType;

fn is_executable_file(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

pub fn find_in_path(program: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    let path_env = path_env?;
    for dir in std::env::split_paths(path_env) {
        let candidate = dir.join(program);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn gamescope_path() -> Option<PathBuf> {
    static GAMESCOPE: OnceLock<Option<PathBuf>> = OnceLock::new();
    GAMESCOPE
        .get_or_init(|| {
            let path_env = std::env::var_os("PATH");
            find_in_path("gamescope", path_env.as_deref())
        })
        .clone()
}

pub fn should_use_gamescope(config: &DisplayConfig, session: SessionType, available: bool) -> bool {
    match config.gamescope {
        GamescopeMode::Off => false,
        GamescopeMode::Auto => session == SessionType::Wayland && available,
        GamescopeMode::Forced => true,
    }
}

pub fn build_gamescope_args(
    config: &DisplayConfig,
    session: SessionType,
    available: bool,
) -> Vec<String> {
    if !should_use_gamescope(config, session, available) {
        return Vec::new();
    }

    let mut args = Vec::new();
    if let Some((width, height)) = config.resolution {
        args.push("-W".into());
        args.push(width.to_string());
        args.push("-H".into());
        args.push(height.to_string());
    }
    if let Some(rate) = config.refresh_rate {
        args.push("-r".into());
        args.push(rate.to_string());
    }
    if config.fullscreen {
        args.push("-f".into());
    }
    if session == SessionType::Wayland {
        if let Some(monitor) = &config.monitor {
            args.push("-O".into());
            args.push(monitor.clone());
        }
    }
    match session {
        SessionType::Wayland => {
            args.push("--backend".into());
            args.push("wayland".into());
        }
        SessionType::X11 => {
            args.push("--backend".into());
            args.push("x11".into());
        }
        SessionType::Unknown => {}
    }
    args.push("--".into());
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn write_executable(path: &std::path::Path) {
        fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn finds_executable_on_path() {
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        let gamescope = second.path().join("gamescope");
        write_executable(&gamescope);

        let path_env = std::env::join_paths([first.path(), second.path()]).unwrap();
        assert_eq!(
            find_in_path("gamescope", Some(path_env.as_os_str())),
            Some(gamescope)
        );
    }

    #[test]
    fn ignores_non_executable_and_missing() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("gamescope");
        fs::write(&file, "not executable").unwrap();

        let path_env = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(find_in_path("gamescope", Some(path_env.as_os_str())), None);
        assert_eq!(find_in_path("missing", Some(path_env.as_os_str())), None);
        assert_eq!(find_in_path("gamescope", None), None);
    }

    #[test]
    fn returns_absolute_paths_from_path_entries() {
        let dir = tempdir().unwrap();
        let gamescope = dir.path().join("gamescope");
        write_executable(&gamescope);
        let path_env = std::env::join_paths([dir.path()]).unwrap();
        let found = find_in_path("gamescope", Some(path_env.as_os_str())).unwrap();
        assert_eq!(found, gamescope);
    }
}
