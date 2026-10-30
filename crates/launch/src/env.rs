use crate::{LaunchError, LaunchRequest};
use display::{SessionType, should_use_gamescope};
use launcher_core::{DllOverride, OverrideMode, RunnerKind};
use prefix::plan_for_game;
use runner_wine::WineContext;
use std::collections::BTreeMap;

const TUNING_VARIABLES: [&str; 8] = [
    "WINEESYNC",
    "WINEFSYNC",
    "PROTON_NO_ESYNC",
    "PROTON_NO_FSYNC",
    "DXVK_ENABLE_NVAPI",
    "DXVK_NVAPI_ALLOW_OTHER_DRIVERS",
    "PROTON_ENABLE_NVAPI",
    "PROTON_DISABLE_NVAPI",
];

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

fn apply_tuning(env: &mut BTreeMap<String, String>, request: &LaunchRequest, kind: RunnerKind) {
    let tuning = &request.tuning;
    match kind {
        RunnerKind::Wine => {
            if tuning.enable_esync {
                env.insert("WINEESYNC".into(), "1".into());
            }
            if tuning.enable_fsync {
                env.insert("WINEFSYNC".into(), "1".into());
            }
            if tuning.enable_dxvk_nvapi {
                env.insert("DXVK_ENABLE_NVAPI".into(), "1".into());
                env.insert("DXVK_NVAPI_ALLOW_OTHER_DRIVERS".into(), "1".into());
            }
        }
        RunnerKind::Proton => {
            if !tuning.enable_esync {
                env.insert("PROTON_NO_ESYNC".into(), "1".into());
            }
            if !tuning.enable_fsync {
                env.insert("PROTON_NO_FSYNC".into(), "1".into());
            }
            if tuning.enable_dxvk_nvapi {
                env.insert("PROTON_ENABLE_NVAPI".into(), "1".into());
                env.insert("DXVK_NVAPI_ALLOW_OTHER_DRIVERS".into(), "1".into());
            } else {
                env.insert("PROTON_DISABLE_NVAPI".into(), "1".into());
            }
        }
    }
}

fn apply_x11_driver(env: &mut BTreeMap<String, String>, kind: RunnerKind) {
    env.remove("WAYLANDDRV_PRIMARY_MONITOR");
    env.remove("PROTON_WAYLAND_MONITOR");
    env.insert("WINE_GRAPHICS_DRIVER".into(), "x11".into());
    if kind == RunnerKind::Proton {
        env.insert("PROTON_ENABLE_WAYLAND".into(), "0".into());
        env.insert("PROTON_USE_WAYLAND".into(), "0".into());
    }
}

pub fn build_env(request: &LaunchRequest) -> Result<BTreeMap<String, String>, LaunchError> {
    let mut env = request.inherited_env.clone();
    env.remove("WINEDLLOVERRIDES");
    for key in TUNING_VARIABLES {
        env.remove(key);
    }

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

    // Native games run outside Wine, so none of the Wine/Proton environment
    // applies; only display selection remains meaningful.
    if let Some(runner) = &request.runner {
        let prefix_plan = plan_for_game(
            &request.game,
            runner.clone(),
            &request.default_prefix_root,
            request.steam_compat_client.clone(),
        );
        let context = WineContext {
            runner: runner.clone(),
            prefix: prefix_plan.path,
            arch: prefix_plan.arch,
            steam_compat_client: request.steam_compat_client.clone(),
        };
        env.extend(context.base_env());
        apply_tuning(&mut env, request, runner.kind);
    }
    env.extend(request.global_env.clone());
    env.extend(request.game.env_overrides.clone());

    let wine_runner = request.runner.as_ref().map(|runner| runner.kind);
    let gamescope_available = request.gamescope_path.is_some();
    let uses_gamescope =
        should_use_gamescope(&request.game.display, request.session, gamescope_available);
    if !uses_gamescope {
        if let Some(monitor) = request
            .game
            .display
            .monitor
            .as_deref()
            .map(str::trim)
            .filter(|monitor| !monitor.is_empty())
        {
            match request.session {
                SessionType::X11 => {
                    env.insert("__GL_SYNC_DISPLAY_DEVICE".into(), monitor.to_string());
                    if let Some(kind) = wine_runner {
                        apply_x11_driver(&mut env, kind);
                    }
                }
                SessionType::Wayland if env.contains_key("DISPLAY") => {
                    // The selected monitor is applied by making it the XRandR
                    // primary, which only affects Wine's X11 driver. Native
                    // OpenGL games still honour the GL sync variable.
                    env.insert("__GL_SYNC_DISPLAY_DEVICE".into(), monitor.to_string());
                    if let Some(kind) = wine_runner {
                        apply_x11_driver(&mut env, kind);
                    }
                }
                SessionType::Wayland => {
                    if let Some(kind) = wine_runner {
                        env.remove("WINE_GRAPHICS_DRIVER");
                        env.insert("WAYLANDDRV_PRIMARY_MONITOR".into(), monitor.to_string());
                        if kind == RunnerKind::Proton {
                            env.insert("PROTON_ENABLE_WAYLAND".into(), "1".into());
                            env.insert("PROTON_USE_WAYLAND".into(), "1".into());
                            env.insert("PROTON_WAYLAND_MONITOR".into(), monitor.to_string());
                        }
                    }
                }
                SessionType::Unknown if env.contains_key("DISPLAY") => {
                    env.insert("__GL_SYNC_DISPLAY_DEVICE".into(), monitor.to_string());
                    if let Some(kind) = wine_runner {
                        apply_x11_driver(&mut env, kind);
                    }
                }
                SessionType::Unknown => {}
            }
        }
    }

    if !request.game.platform.is_native() && !request.game.dll_overrides.is_empty() {
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
        Arch, DisplayConfig, Game, GamescopeMode, Platform, Runner, RunnerKind, RunnerSource,
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
            cover_path: None,
            platform: Platform::Windows,
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            env_overrides: BTreeMap::new(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
            playtime_seconds: 0,
        }
    }

    fn request() -> LaunchRequest {
        LaunchRequest {
            game: game(),
            runner: Some(runner()),
            session: SessionType::X11,
            global_env: BTreeMap::new(),
            inherited_env: BTreeMap::new(),
            steam_compat_client: PathBuf::from("/data/steam-compat-client"),
            default_prefix_root: PathBuf::from("/data/prefixes"),
            gamescope_path: None,
            tuning: crate::LaunchTuning::default(),
            log_path: None,
            dxvk_nvapi_source: None,
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
    fn wayland_monitor_targets_wine_wayland_driver() {
        let mut request = request();
        request.session = SessionType::Wayland;
        request.game.display.monitor = Some("eDP-1".into());

        let env = build_env(&request).unwrap();
        assert_eq!(env["WAYLANDDRV_PRIMARY_MONITOR"], "eDP-1");
        assert!(!env.contains_key("__GL_SYNC_DISPLAY_DEVICE"));
        assert!(!env.contains_key("PROTON_ENABLE_WAYLAND"));
        assert!(!env.contains_key("PROTON_WAYLAND_MONITOR"));
    }

    #[test]
    fn wayland_with_xwayland_uses_x11_for_monitor_selection() {
        let mut request = request();
        request.session = SessionType::Wayland;
        request.inherited_env = BTreeMap::from([("DISPLAY".into(), ":0".into())]);
        request.game.display.monitor = Some("HDMI-1".into());

        let env = build_env(&request).unwrap();
        assert_eq!(env["WINE_GRAPHICS_DRIVER"], "x11");
        assert_eq!(env["__GL_SYNC_DISPLAY_DEVICE"], "HDMI-1");
        assert!(!env.contains_key("WAYLANDDRV_PRIMARY_MONITOR"));
        assert!(!env.contains_key("PROTON_WAYLAND_MONITOR"));
        assert!(!env.contains_key("PROTON_ENABLE_WAYLAND"));
    }

    #[test]
    fn proton_with_xwayland_disables_the_wayland_driver() {
        let mut request = request();
        request.session = SessionType::Wayland;
        request.inherited_env = BTreeMap::from([("DISPLAY".into(), ":0".into())]);
        let runner = request.runner.as_mut().unwrap();
        runner.kind = RunnerKind::Proton;
        runner.binary_path = PathBuf::from("/opt/proton/proton");
        runner.root_dir = PathBuf::from("/opt/proton");
        request.game.prefix_path = Some(PathBuf::from("/data/compat"));
        request.game.display.monitor = Some("HDMI-1".into());

        let env = build_env(&request).unwrap();
        assert_eq!(env["PROTON_ENABLE_WAYLAND"], "0");
        assert_eq!(env["PROTON_USE_WAYLAND"], "0");
        assert_eq!(env["WINE_GRAPHICS_DRIVER"], "x11");
        assert!(!env.contains_key("PROTON_WAYLAND_MONITOR"));
        assert!(!env.contains_key("WAYLANDDRV_PRIMARY_MONITOR"));
    }

    #[test]
    fn wayland_monitor_enables_proton_wayland_driver_without_xwayland() {
        let mut request = request();
        request.session = SessionType::Wayland;
        let runner = request.runner.as_mut().unwrap();
        runner.kind = RunnerKind::Proton;
        runner.binary_path = PathBuf::from("/opt/proton/proton");
        runner.root_dir = PathBuf::from("/opt/proton");
        request.game.prefix_path = Some(PathBuf::from("/data/compat"));
        request.game.display.monitor = Some("HDMI-1".into());

        let env = build_env(&request).unwrap();
        assert_eq!(env["PROTON_ENABLE_WAYLAND"], "1");
        assert_eq!(env["PROTON_USE_WAYLAND"], "1");
        assert_eq!(env["PROTON_WAYLAND_MONITOR"], "HDMI-1");
        assert_eq!(env["WAYLANDDRV_PRIMARY_MONITOR"], "HDMI-1");
        assert!(!env.contains_key("__GL_SYNC_DISPLAY_DEVICE"));
        assert!(!env.contains_key("WINE_GRAPHICS_DRIVER"));
    }

    #[test]
    fn wayland_without_selected_monitor_keeps_driver_defaults() {
        let mut request = request();
        request.session = SessionType::Wayland;
        let runner = request.runner.as_mut().unwrap();
        runner.kind = RunnerKind::Proton;
        runner.binary_path = PathBuf::from("/opt/proton/proton");
        runner.root_dir = PathBuf::from("/opt/proton");
        request.game.prefix_path = Some(PathBuf::from("/data/compat"));

        let env = build_env(&request).unwrap();
        assert!(!env.contains_key("PROTON_ENABLE_WAYLAND"));
        assert!(!env.contains_key("PROTON_WAYLAND_MONITOR"));
        assert!(!env.contains_key("WAYLANDDRV_PRIMARY_MONITOR"));
        assert!(!env.contains_key("WINE_GRAPHICS_DRIVER"));
    }

    #[test]
    fn gamescope_uses_output_flag_instead_of_monitor_env() {
        let mut request = request();
        request.session = SessionType::Wayland;
        let runner = request.runner.as_mut().unwrap();
        runner.kind = RunnerKind::Proton;
        runner.binary_path = PathBuf::from("/opt/proton/proton");
        runner.root_dir = PathBuf::from("/opt/proton");
        request.game.prefix_path = Some(PathBuf::from("/data/compat"));
        request.game.display.monitor = Some("HDMI-1".into());
        request.game.display.gamescope = GamescopeMode::Forced;
        request.gamescope_path = Some(PathBuf::from("/usr/bin/gamescope"));

        let env = build_env(&request).unwrap();
        assert!(!env.contains_key("PROTON_ENABLE_WAYLAND"));
        assert!(!env.contains_key("PROTON_WAYLAND_MONITOR"));
        assert!(!env.contains_key("WAYLANDDRV_PRIMARY_MONITOR"));
    }

    #[test]
    fn proton_environment_uses_compat_variables() {
        let mut request = request();
        let runner = request.runner.as_mut().unwrap();
        runner.kind = RunnerKind::Proton;
        runner.binary_path = PathBuf::from("/opt/proton/proton");
        runner.root_dir = PathBuf::from("/opt/proton");
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

    fn tuning() -> crate::LaunchTuning {
        crate::LaunchTuning {
            enable_esync: true,
            enable_fsync: true,
            ..crate::LaunchTuning::default()
        }
    }

    #[test]
    fn wine_esync_and_fsync_are_injected_when_enabled() {
        let mut request = request();
        request.tuning = tuning();
        let env = build_env(&request).unwrap();
        assert_eq!(env["WINEESYNC"], "1");
        assert_eq!(env["WINEFSYNC"], "1");
    }

    #[test]
    fn wine_esync_and_fsync_are_absent_when_disabled() {
        let request = request();
        let env = build_env(&request).unwrap();
        assert!(!env.contains_key("WINEESYNC"));
        assert!(!env.contains_key("WINEFSYNC"));
    }

    #[test]
    fn proton_disables_esync_and_fsync_only_when_turned_off() {
        let mut request = request();
        request.runner.as_mut().unwrap().kind = RunnerKind::Proton;
        request.tuning = tuning();

        let env = build_env(&request).unwrap();
        assert!(!env.contains_key("PROTON_NO_ESYNC"));
        assert!(!env.contains_key("PROTON_NO_FSYNC"));

        request.tuning.enable_esync = false;
        request.tuning.enable_fsync = false;
        let env = build_env(&request).unwrap();
        assert_eq!(env["PROTON_NO_ESYNC"], "1");
        assert_eq!(env["PROTON_NO_FSYNC"], "1");
        assert!(!env.contains_key("WINEESYNC"));
        assert!(!env.contains_key("WINEFSYNC"));
    }

    #[test]
    fn wine_nvapi_environment_is_injected_when_enabled() {
        let mut request = request();
        request.tuning = crate::LaunchTuning {
            enable_dxvk_nvapi: true,
            ..crate::LaunchTuning::default()
        };
        let env = build_env(&request).unwrap();
        assert_eq!(env["DXVK_ENABLE_NVAPI"], "1");
        assert_eq!(env["DXVK_NVAPI_ALLOW_OTHER_DRIVERS"], "1");
    }

    #[test]
    fn proton_nvapi_environment_toggles_between_enable_and_disable() {
        let mut request = request();
        request.runner.as_mut().unwrap().kind = RunnerKind::Proton;
        request.tuning = crate::LaunchTuning {
            enable_dxvk_nvapi: true,
            ..crate::LaunchTuning::default()
        };

        let env = build_env(&request).unwrap();
        assert_eq!(env["PROTON_ENABLE_NVAPI"], "1");
        assert_eq!(env["DXVK_NVAPI_ALLOW_OTHER_DRIVERS"], "1");
        assert!(!env.contains_key("PROTON_DISABLE_NVAPI"));

        request.tuning.enable_dxvk_nvapi = false;
        let env = build_env(&request).unwrap();
        assert_eq!(env["PROTON_DISABLE_NVAPI"], "1");
        assert!(!env.contains_key("PROTON_ENABLE_NVAPI"));
    }

    #[test]
    fn inherited_tuning_variables_are_dropped() {
        let mut request = request();
        request.inherited_env = BTreeMap::from([
            ("WINEESYNC".into(), "1".into()),
            ("WINEFSYNC".into(), "1".into()),
            ("PROTON_NO_ESYNC".into(), "1".into()),
            ("PROTON_NO_FSYNC".into(), "1".into()),
            ("DXVK_ENABLE_NVAPI".into(), "1".into()),
            ("DXVK_NVAPI_ALLOW_OTHER_DRIVERS".into(), "1".into()),
            ("PROTON_ENABLE_NVAPI".into(), "1".into()),
            ("PROTON_DISABLE_NVAPI".into(), "1".into()),
        ]);

        let env = build_env(&request).unwrap();
        for key in [
            "WINEESYNC",
            "WINEFSYNC",
            "PROTON_NO_ESYNC",
            "PROTON_NO_FSYNC",
            "DXVK_ENABLE_NVAPI",
            "DXVK_NVAPI_ALLOW_OTHER_DRIVERS",
            "PROTON_ENABLE_NVAPI",
            "PROTON_DISABLE_NVAPI",
        ] {
            assert!(!env.contains_key(key), "{key} should have been dropped");
        }
    }

    #[test]
    fn game_environment_overrides_tuning_defaults() {
        let mut request = request();
        request.tuning = tuning();
        request.game.env_overrides = BTreeMap::from([("WINEESYNC".into(), "0".into())]);
        let env = build_env(&request).unwrap();
        assert_eq!(env["WINEESYNC"], "0");
        assert_eq!(env["WINEFSYNC"], "1");
    }

    fn native_request() -> LaunchRequest {
        let mut request = request();
        request.game.platform = Platform::Linux;
        request.runner = None;
        request
    }

    #[test]
    fn native_games_skip_wine_environment() {
        let mut request = native_request();
        request.tuning = crate::LaunchTuning {
            enable_esync: true,
            enable_fsync: true,
            enable_dxvk_nvapi: true,
            ..crate::LaunchTuning::default()
        };
        request.game.dll_overrides = vec![DllOverride {
            dll_name: "d3d11".into(),
            mode: OverrideMode::Native,
        }];

        let env = build_env(&request).unwrap();
        for key in [
            "WINEPREFIX",
            "WINEARCH",
            "WINEESYNC",
            "WINEFSYNC",
            "DXVK_ENABLE_NVAPI",
            "WINEDLLOVERRIDES",
        ] {
            assert!(
                !env.contains_key(key),
                "{key} should be absent for native games"
            );
        }
    }

    #[test]
    fn native_x11_monitor_still_sets_gl_display_device() {
        let mut request = native_request();
        request.game.display.monitor = Some("eDP-1".into());

        let env = build_env(&request).unwrap();
        assert_eq!(env["__GL_SYNC_DISPLAY_DEVICE"], "eDP-1");
        assert!(!env.contains_key("WINE_GRAPHICS_DRIVER"));
    }
}
