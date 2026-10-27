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

fn mangohud_path_from(path_env: Option<&OsStr>) -> Option<PathBuf> {
    find_in_path("mangohud", path_env)
}

pub fn mangohud_path() -> Option<PathBuf> {
    static MANGOHUD: OnceLock<Option<PathBuf>> = OnceLock::new();
    MANGOHUD
        .get_or_init(|| {
            let path_env = std::env::var_os("PATH");
            mangohud_path_from(path_env.as_deref())
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
    mangoapp: bool,
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
    if mangoapp {
        args.push("--mangoapp".into());
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

    use launcher_core::{DisplayBackend, GamescopeMode};

    fn config(gamescope: GamescopeMode) -> DisplayConfig {
        DisplayConfig {
            backend: DisplayBackend::Auto,
            monitor: Some("eDP-1".into()),
            resolution: Some((2560, 1440)),
            refresh_rate: Some(144),
            fullscreen: true,
            gamescope,
        }
    }

    #[test]
    fn off_never_uses_gamescope() {
        let config = config(GamescopeMode::Off);
        assert!(!should_use_gamescope(&config, SessionType::Wayland, true));
        assert!(build_gamescope_args(&config, SessionType::Wayland, true, false).is_empty());
    }

    #[test]
    fn auto_requires_wayland_and_availability() {
        let config = config(GamescopeMode::Auto);
        assert!(should_use_gamescope(&config, SessionType::Wayland, true));
        assert!(!should_use_gamescope(&config, SessionType::Wayland, false));
        assert!(!should_use_gamescope(&config, SessionType::X11, true));
        assert!(!should_use_gamescope(&config, SessionType::Unknown, true));
    }

    #[test]
    fn forced_generates_arguments_even_when_unavailable() {
        let config = config(GamescopeMode::Forced);
        assert!(should_use_gamescope(&config, SessionType::X11, false));
        assert_eq!(
            build_gamescope_args(&config, SessionType::X11, false, false),
            vec![
                "-W",
                "2560",
                "-H",
                "1440",
                "-r",
                "144",
                "-f",
                "--backend",
                "x11",
                "--"
            ]
        );
    }

    #[test]
    fn wayland_arguments_are_exact_and_include_output() {
        let config = config(GamescopeMode::Auto);
        assert_eq!(
            build_gamescope_args(&config, SessionType::Wayland, true, false),
            vec![
                "-W",
                "2560",
                "-H",
                "1440",
                "-r",
                "144",
                "-f",
                "-O",
                "eDP-1",
                "--backend",
                "wayland",
                "--"
            ]
        );
    }

    #[test]
    fn empty_options_only_produce_backend_and_separator() {
        let config = DisplayConfig {
            backend: DisplayBackend::Auto,
            monitor: None,
            resolution: None,
            refresh_rate: None,
            fullscreen: false,
            gamescope: GamescopeMode::Auto,
        };
        assert_eq!(
            build_gamescope_args(&config, SessionType::Wayland, true, false),
            vec!["--backend", "wayland", "--"]
        );
    }

    #[test]
    fn x11_does_not_pass_output_flag() {
        let config = config(GamescopeMode::Forced);
        let args = build_gamescope_args(&config, SessionType::X11, true, false);
        assert!(!args.contains(&"-O".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("--"));
    }

    #[test]
    fn unknown_session_omits_backend_flag() {
        let config = config(GamescopeMode::Forced);
        assert_eq!(
            build_gamescope_args(&config, SessionType::Unknown, false, false),
            vec!["-W", "2560", "-H", "1440", "-r", "144", "-f", "--"]
        );
    }

    #[test]
    fn mangoapp_flag_is_inserted_before_separator() {
        let config = config(GamescopeMode::Auto);
        let args = build_gamescope_args(&config, SessionType::Wayland, true, true);
        assert_eq!(args[args.len() - 2], "--mangoapp");
        assert_eq!(args.last().map(String::as_str), Some("--"));
    }

    #[test]
    fn mangoapp_is_not_present_when_disabled() {
        let config = config(GamescopeMode::Auto);
        let args = build_gamescope_args(&config, SessionType::Wayland, true, false);
        assert!(!args.contains(&"--mangoapp".to_string()));
    }

    #[test]
    fn mango_hud_path_is_resolved_from_injected_path() {
        let dir = tempdir().unwrap();
        let mangohud = dir.path().join("mangohud");
        write_executable(&mangohud);
        let path_env = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(
            mangohud_path_from(Some(path_env.as_os_str())),
            Some(mangohud)
        );
        assert_eq!(mangohud_path_from(None), None);
    }
}
