use crate::{LaunchError, LaunchRequest};
use display::{SessionType, should_use_gamescope};
use launcher_core::{DllOverride, OverrideMode};
use prefix::plan_for_game;
use runner_wine::WineContext;
use std::collections::BTreeMap;

fn mode_value(mode: OverrideMode) -> &'static str {
    match mode {
        OverrideMode::Disabled => "disabled",
        OverrideMode::Native => "n",
        OverrideMode::Builtin => "b",
        OverrideMode::NativeBuiltin => "n,b",
    }
}

pub fn build_dll_overrides(overrides: &[DllOverride]) -> String {
    overrides
        .iter()
        .filter_map(|entry| {
            let name = entry.dll_name.trim();
            if name.is_empty() {
                None
            } else {
                Some(format!("{name}={}", mode_value(entry.mode)))
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}

pub fn validate_env_key(key: &str) -> Result<(), LaunchError> {
    let mut chars = key.chars();
    let first = chars
        .next()
        .ok_or_else(|| LaunchError::InvalidEnvKey(key.to_string()))?;
    if first != '_' && !first.is_ascii_alphabetic() {
        return Err(LaunchError::InvalidEnvKey(key.to_string()));
    }
    if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        return Err(LaunchError::InvalidEnvKey(key.to_string()));
    }
    Ok(())
}

pub fn validate_dll_name(name: &str) -> Result<(), LaunchError> {
    let name = name.trim();
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_whitespace)
        || name.to_ascii_lowercase().ends_with(".dll")
    {
        return Err(LaunchError::InvalidDllName(name.to_string()));
    }
    Ok(())
}

pub fn build_env(request: &LaunchRequest) -> Result<BTreeMap<String, String>, LaunchError> {
    let prefix_plan = plan_for_game(
        &request.game,
        request.runner.clone(),
        &request.default_prefix_root,
        request.steam_compat_client.clone(),
    );
    let context = WineContext {
        runner: request.runner.clone(),
        prefix: prefix_plan.path,
        arch: prefix_plan.arch,
        steam_compat_client: request.steam_compat_client.clone(),
    };

    let mut env = request.inherited_env.clone();
    env.remove("WINEDLLOVERRIDES");

    if request.global_env.contains_key("WINEDLLOVERRIDES")
        || request.game.env_overrides.contains_key("WINEDLLOVERRIDES")
    {
        return Err(LaunchError::DllOverrideConflict);
    }

    for key in request
        .global_env
        .keys()
        .chain(request.game.env_overrides.keys())
    {
        validate_env_key(key)?;
    }

    env.extend(context.base_env());
    env.extend(request.global_env.clone());
    env.extend(request.game.env_overrides.clone());

    let gamescope_available = request.gamescope_path.is_some();
    let uses_gamescope =
        should_use_gamescope(&request.game.display, request.session, gamescope_available);
    if request.session == SessionType::X11 && !uses_gamescope {
        if let Some(monitor) = request
            .game
            .display
            .monitor
            .as_deref()
            .map(str::trim)
            .filter(|monitor| !monitor.is_empty())
        {
            env.insert("__GL_SYNC_DISPLAY_DEVICE".into(), monitor.to_string());
        }
    }

    if !request.game.dll_overrides.is_empty() {
        for entry in &request.game.dll_overrides {
            validate_dll_name(&entry.dll_name)?;
        }
        env.insert(
            "WINEDLLOVERRIDES".into(),
            build_dll_overrides(&request.game.dll_overrides),
        );
    }

    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{DllOverride, OverrideMode};

    #[test]
    fn all_modes_map_to_wine_syntax() {
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "a".into(),
                mode: OverrideMode::Disabled,
            }]),
            "a=disabled"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "b".into(),
                mode: OverrideMode::Native,
            }]),
            "b=n"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "c".into(),
                mode: OverrideMode::Builtin,
            }]),
            "c=b"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "d".into(),
                mode: OverrideMode::NativeBuiltin,
            }]),
            "d=n,b"
        );
    }

    #[test]
    fn multiple_overrides_are_semicolon_separated_in_input_order() {
        let overrides = vec![
            DllOverride {
                dll_name: "winmm".into(),
                mode: OverrideMode::NativeBuiltin,
            },
            DllOverride {
                dll_name: "steam_api64".into(),
                mode: OverrideMode::Native,
            },
        ];
        assert_eq!(build_dll_overrides(&overrides), "winmm=n,b;steam_api64=n");
    }

    #[test]
    fn empty_names_are_skipped() {
        let overrides = vec![
            DllOverride {
                dll_name: "  ".into(),
                mode: OverrideMode::Native,
            },
            DllOverride {
                dll_name: " winmm ".into(),
                mode: OverrideMode::Builtin,
            },
        ];
        assert_eq!(build_dll_overrides(&overrides), "winmm=b");
    }

    use crate::LaunchRequest;
    use display::SessionType;
    use launcher_core::{
        Arch, DisplayConfig, Game, GamescopeMode, Runner, RunnerKind, RunnerSource,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn runner() -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: "Wine".into(),
            binary_path: PathBuf::from("/usr/bin/wine"),
            root_dir: PathBuf::from("/usr/bin"),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    fn game() -> Game {
        Game {
            id: Uuid::new_v4(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: Some(PathBuf::from("/data/prefix")),
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            env_overrides: BTreeMap::new(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        }
    }

    fn request() -> LaunchRequest {
        LaunchRequest {
            game: game(),
            runner: runner(),
            session: SessionType::X11,
            global_env: BTreeMap::new(),
            inherited_env: BTreeMap::new(),
            steam_compat_client: PathBuf::from("/data/steam-compat-client"),
            default_prefix_root: PathBuf::from("/data/prefixes"),
            gamescope_path: None,
        }
    }

    #[test]
    fn environment_precedence_is_game_over_global_over_inherited_over_base() {
        let mut request = request();
        request.inherited_env = BTreeMap::from([
            ("A".into(), "inherited".into()),
            ("HOME".into(), "/home/test".into()),
            ("WINEDLLOVERRIDES".into(), "legacy".into()),
        ]);
        request.global_env =
            BTreeMap::from([("A".into(), "global".into()), ("B".into(), "global".into())]);
        request.game.env_overrides = BTreeMap::from([
            ("A".into(), "game".into()),
            ("C".into(), "game".into()),
            ("WINEDEBUG".into(), "-fixme".into()),
        ]);

        let env = build_env(&request).unwrap();
        assert_eq!(env["A"], "game");
        assert_eq!(env["B"], "global");
        assert_eq!(env["C"], "game");
        assert_eq!(env["HOME"], "/home/test");
        assert_eq!(env["WINEPREFIX"], "/data/prefix");
        assert_eq!(env["WINEARCH"], "win64");
        assert_eq!(env["WINEDEBUG"], "-fixme");
        assert!(!env.contains_key("WINEDLLOVERRIDES"));
    }

    #[test]
    fn manual_winedlloverrides_is_rejected_even_if_inherited() {
        let mut request = request();
        request.inherited_env = BTreeMap::from([("WINEDLLOVERRIDES".into(), "x=n".into())]);
        request.game.env_overrides = BTreeMap::from([("WINEDLLOVERRIDES".into(), "y=n".into())]);
        assert!(matches!(
            build_env(&request),
            Err(LaunchError::DllOverrideConflict)
        ));
    }

    #[test]
    fn global_winedlloverrides_is_rejected() {
        let mut request = request();
        request.global_env = BTreeMap::from([("WINEDLLOVERRIDES".into(), "x=n".into())]);
        assert!(matches!(
            build_env(&request),
            Err(LaunchError::DllOverrideConflict)
        ));
    }

    #[test]
    fn structured_overrides_replace_the_conflicting_variable() {
        let mut request = request();
        request.game.dll_overrides = vec![
            DllOverride {
                dll_name: "d3d11".into(),
                mode: launcher_core::OverrideMode::NativeBuiltin,
            },
            DllOverride {
                dll_name: "winmm".into(),
                mode: launcher_core::OverrideMode::Native,
            },
        ];
        let env = build_env(&request).unwrap();
        assert_eq!(env["WINEDLLOVERRIDES"], "d3d11=n,b;winmm=n");
    }

    #[test]
    fn x11_monitor_is_injected_only_without_gamescope() {
        let mut request = request();
        request.game.display.monitor = Some("eDP-1".into());
        assert_eq!(
            build_env(&request).unwrap()["__GL_SYNC_DISPLAY_DEVICE"],
            "eDP-1"
        );

        request.gamescope_path = Some(PathBuf::from("/usr/bin/gamescope"));
        request.game.display.gamescope = GamescopeMode::Forced;
        assert!(
            !build_env(&request)
                .unwrap()
                .contains_key("__GL_SYNC_DISPLAY_DEVICE")
        );
    }

    #[test]
    fn wayland_never_injects_x11_display_variable() {
        let mut request = request();
        request.session = SessionType::Wayland;
        request.game.display.monitor = Some("eDP-1".into());
        assert!(
            !build_env(&request)
                .unwrap()
                .contains_key("__GL_SYNC_DISPLAY_DEVICE")
        );
    }

    #[test]
    fn proton_environment_uses_compat_variables() {
        let mut request = request();
        request.runner.kind = RunnerKind::Proton;
        request.runner.binary_path = PathBuf::from("/opt/proton/proton");
        request.runner.root_dir = PathBuf::from("/opt/proton");
        request.game.prefix_path = Some(PathBuf::from("/data/compat"));
        request.game.arch = Arch::Win32;

        let env = build_env(&request).unwrap();
        assert_eq!(env["STEAM_COMPAT_DATA_PATH"], "/data/compat");
        assert_eq!(
            env["STEAM_COMPAT_CLIENT_INSTALL_PATH"],
            "/data/steam-compat-client"
        );
        assert_eq!(env["PROTONPATH"], "/opt/proton");
        assert!(!env.contains_key("WINEPREFIX"));
    }

    #[test]
    fn invalid_env_keys_are_rejected() {
        let mut request = request();
        request.game.env_overrides = BTreeMap::from([("BAD KEY".into(), "x".into())]);
        assert!(matches!(
            build_env(&request),
            Err(LaunchError::InvalidEnvKey(_))
        ));
    }

    #[test]
    fn invalid_dll_names_are_rejected() {
        for name in ["", " ", "d3d11.dll", "../evil", "bad name"] {
            let mut request = request();
            request.game.dll_overrides = vec![DllOverride {
                dll_name: name.into(),
                mode: launcher_core::OverrideMode::Native,
            }];
            assert!(
                matches!(build_env(&request), Err(LaunchError::InvalidDllName(_))),
                "expected {name:?} to be rejected"
            );
        }
    }
}
