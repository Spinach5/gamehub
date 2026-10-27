use crate::env::{validate_dll_name, validate_env_key};
use crate::{LaunchError, build_env};
use display::{SessionType, build_gamescope_args, should_use_gamescope};
use launcher_core::{Game, GamescopeMode, Runner};
use prefix::{PrefixStatus, plan_for_game, status};
use runner_wine::{WineContext, verify_runner};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct LaunchTuning {
    pub enable_esync: bool,
    pub enable_fsync: bool,
    pub enable_mangohud: bool,
    pub mangohud_path: Option<PathBuf>,
    pub enable_dxvk_nvapi: bool,
}

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
    pub tuning: LaunchTuning,
    pub log_path: Option<PathBuf>,
    pub dxvk_nvapi_source: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

pub fn validate(request: &LaunchRequest) -> Result<(), LaunchError> {
    if !request.game.executable_path.is_file() {
        return Err(LaunchError::ExecutableMissing(
            request.game.executable_path.clone(),
        ));
    }

    verify_runner(&request.runner)?;

    if let Some(working_dir) = request
        .game
        .working_dir
        .as_ref()
        .filter(|dir| !dir.is_dir())
    {
        return Err(LaunchError::WorkingDirMissing(working_dir.clone()));
    }

    let prefix_plan = plan_for_game(
        &request.game,
        request.runner.clone(),
        &request.default_prefix_root,
        request.steam_compat_client.clone(),
    );
    match status(&prefix_plan) {
        PrefixStatus::Initialized => {}
        PrefixStatus::ArchMismatch { existing } => {
            return Err(LaunchError::PrefixArchMismatch {
                expected: prefix_plan.arch,
                existing,
            });
        }
        PrefixStatus::Missing | PrefixStatus::NotInitialized => {
            return Err(LaunchError::PrefixNotInitialized(prefix_plan.path));
        }
    }

    if request.game.display.gamescope == GamescopeMode::Forced && request.gamescope_path.is_none() {
        return Err(LaunchError::GamescopeUnavailable);
    }

    if request.tuning.enable_mangohud && request.tuning.mangohud_path.is_none() {
        return Err(LaunchError::MangohudUnavailable);
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
    for entry in &request.game.dll_overrides {
        validate_dll_name(&entry.dll_name)?;
    }

    Ok(())
}

pub fn build_plan(request: &LaunchRequest) -> Result<LaunchPlan, LaunchError> {
    validate(request)?;

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

    let (mut program, mut args) =
        context.build_command(&request.game.executable_path, &request.game.args);

    let gamescope_available = request.gamescope_path.is_some();
    if should_use_gamescope(&request.game.display, request.session, gamescope_available) {
        let gamescope = request
            .gamescope_path
            .clone()
            .ok_or(LaunchError::GamescopeUnavailable)?;
        let mut gamescope_args = build_gamescope_args(
            &request.game.display,
            request.session,
            gamescope_available,
            request.tuning.enable_mangohud,
        );
        gamescope_args.push(program.to_string_lossy().into_owned());
        gamescope_args.extend(args);
        program = gamescope;
        args = gamescope_args;
    } else if request.tuning.enable_mangohud {
        let mangohud = request
            .tuning
            .mangohud_path
            .clone()
            .ok_or(LaunchError::MangohudUnavailable)?;
        let mut mangohud_args = vec![
            "--dlsym".to_string(),
            program.to_string_lossy().into_owned(),
        ];
        mangohud_args.extend(args);
        program = mangohud;
        args = mangohud_args;
    }

    let env = build_env(request)?;
    let cwd = request.game.working_dir.clone().or_else(|| {
        request
            .game
            .executable_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
    });

    Ok(LaunchPlan {
        program,
        args,
        env,
        cwd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, DisplayConfig, GamescopeMode, RunnerKind, RunnerSource};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn request(dir: &tempfile::TempDir) -> LaunchRequest {
        let wine = dir.path().join("wine");
        write_executable(&wine, "#!/bin/sh\nexit 0\n");
        let executable = dir.path().join("game.exe");
        fs::write(&executable, b"MZ").unwrap();

        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        LaunchRequest {
            game: Game {
                id: uuid::Uuid::new_v4(),
                name: "test".into(),
                executable_path: executable.clone(),
                working_dir: Some(dir.path().to_path_buf()),
                runner_id: None,
                prefix_path: Some(prefix),
                arch: Arch::Win64,
                display: DisplayConfig::default(),
                env_overrides: BTreeMap::new(),
                dll_overrides: Vec::new(),
                args: vec!["-skip".into()],
                created_at: 0,
                last_played_at: None,
            },
            runner: Runner {
                id: uuid::Uuid::new_v4(),
                kind: RunnerKind::Wine,
                name: "Wine".into(),
                binary_path: wine,
                root_dir: dir.path().to_path_buf(),
                arch: Arch::Win64,
                source: RunnerSource::System,
                verified: true,
                version_string: None,
            },
            session: SessionType::X11,
            global_env: BTreeMap::new(),
            inherited_env: BTreeMap::new(),
            steam_compat_client: dir.path().join("steam-client"),
            default_prefix_root: dir.path().join("prefixes"),
            gamescope_path: None,
            tuning: crate::LaunchTuning::default(),
            log_path: None,
            dxvk_nvapi_source: None,
        }
    }

    #[test]
    fn missing_executable_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.executable_path = dir.path().join("missing.exe");
        assert!(matches!(
            validate(&request),
            Err(LaunchError::ExecutableMissing(_))
        ));
    }

    #[test]
    fn missing_runner_binary_is_rejected() {
        let dir = tempdir().unwrap();
        let request = request(&dir);
        fs::remove_file(&request.runner.binary_path).unwrap();
        assert!(matches!(
            validate(&request),
            Err(LaunchError::Runner(
                runner_wine::RunnerError::MissingBinary(_)
            ))
        ));
    }

    #[test]
    fn missing_working_directory_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.working_dir = Some(dir.path().join("missing"));
        assert!(matches!(
            validate(&request),
            Err(LaunchError::WorkingDirMissing(_))
        ));
    }

    #[test]
    fn uninitialized_prefix_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        let prefix = dir.path().join("empty-prefix");
        fs::create_dir(&prefix).unwrap();
        request.game.prefix_path = Some(prefix.clone());
        assert!(matches!(
            validate(&request),
            Err(LaunchError::PrefixNotInitialized(path)) if path == prefix
        ));
    }

    #[test]
    fn arch_mismatch_is_rejected() {
        let dir = tempdir().unwrap();
        let request = request(&dir);
        fs::write(
            request
                .game
                .prefix_path
                .as_ref()
                .unwrap()
                .join("system.reg"),
            "#arch=win32\n",
        )
        .unwrap();
        assert!(matches!(
            validate(&request),
            Err(LaunchError::PrefixArchMismatch {
                existing: Some(Arch::Win32),
                ..
            })
        ));
    }

    #[test]
    fn forced_gamescope_without_binary_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.display.gamescope = GamescopeMode::Forced;
        request.gamescope_path = None;
        assert!(matches!(
            validate(&request),
            Err(LaunchError::GamescopeUnavailable)
        ));
    }

    #[test]
    fn manual_dll_override_is_rejected_before_spawn() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.env_overrides = BTreeMap::from([("WINEDLLOVERRIDES".into(), "x=n".into())]);
        assert!(matches!(
            validate(&request),
            Err(LaunchError::DllOverrideConflict)
        ));
    }

    #[test]
    fn proton_game_uses_win64_prefix_even_for_win32_game() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.runner.kind = RunnerKind::Proton;
        request.runner.binary_path = dir.path().join("proton");
        write_executable(&request.runner.binary_path, "#!/bin/sh\nexit 0\n");
        request.game.prefix_path = Some(dir.path().join("compat"));
        fs::create_dir_all(dir.path().join("compat/pfx")).unwrap();
        fs::write(dir.path().join("compat/pfx/system.reg"), "#arch=win64\n").unwrap();
        request.game.arch = Arch::Win32;

        validate(&request).unwrap();
    }

    #[test]
    fn plan_without_gamescope_uses_runner_command() {
        let dir = tempdir().unwrap();
        let request = request(&dir);
        let wine = request.runner.binary_path.clone();
        let exe = request.game.executable_path.clone();
        let working_dir = request.game.working_dir.clone();

        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.program, wine);
        assert_eq!(plan.args[0], exe.to_string_lossy().into_owned());
        assert_eq!(plan.args[1], "-skip");
        assert_eq!(plan.cwd, working_dir);
    }

    #[test]
    fn plan_with_gamescope_wraps_runner_command() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        let gamescope = dir.path().join("gamescope");
        write_executable(&gamescope, "#!/bin/sh\nexit 0\n");
        request.gamescope_path = Some(gamescope.clone());
        request.session = SessionType::Wayland;
        request.game.display = DisplayConfig {
            resolution: Some((2560, 1440)),
            fullscreen: true,
            gamescope: GamescopeMode::Forced,
            ..DisplayConfig::default()
        };

        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.program, gamescope);
        assert!(
            plan.args
                .windows(2)
                .any(|pair| pair[0] == "--backend" && pair[1] == "wayland")
        );
        let separator = plan.args.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(
            plan.args[separator + 1],
            request.runner.binary_path.to_string_lossy().into_owned()
        );
        assert_eq!(
            plan.args[separator + 2],
            request.game.executable_path.to_string_lossy().into_owned()
        );
    }

    #[test]
    fn cwd_falls_back_to_executable_parent() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.working_dir = None;
        let plan = build_plan(&request).unwrap();
        assert_eq!(
            plan.cwd,
            request.game.executable_path.parent().map(Path::to_path_buf)
        );
    }

    #[test]
    fn x11_monitor_env_is_present_in_plan() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.display.monitor = Some("eDP-1".into());
        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.env["__GL_SYNC_DISPLAY_DEVICE"], "eDP-1");
    }

    #[test]
    fn mangohud_wraps_command_when_not_using_gamescope() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        let mangohud = dir.path().join("mangohud");
        write_executable(&mangohud, "#!/bin/sh\nexit 0\n");
        request.tuning = crate::LaunchTuning {
            enable_mangohud: true,
            mangohud_path: Some(mangohud.clone()),
            ..crate::LaunchTuning::default()
        };

        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.program, mangohud);
        assert_eq!(plan.args[0], "--dlsym");
        assert_eq!(
            plan.args[1],
            request.runner.binary_path.to_string_lossy().into_owned()
        );
        assert_eq!(
            plan.args[2],
            request.game.executable_path.to_string_lossy().into_owned()
        );
    }

    #[test]
    fn mangohud_with_gamescope_uses_mangoapp_instead_of_wrapper() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        let gamescope = dir.path().join("gamescope");
        write_executable(&gamescope, "#!/bin/sh\nexit 0\n");
        let mangohud = dir.path().join("mangohud");
        write_executable(&mangohud, "#!/bin/sh\nexit 0\n");
        request.gamescope_path = Some(gamescope.clone());
        request.session = SessionType::Wayland;
        request.game.display = DisplayConfig {
            gamescope: GamescopeMode::Forced,
            ..DisplayConfig::default()
        };
        request.tuning = crate::LaunchTuning {
            enable_mangohud: true,
            mangohud_path: Some(mangohud),
            ..crate::LaunchTuning::default()
        };

        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.program, gamescope);
        let separator = plan.args.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(plan.args[separator - 1], "--mangoapp");
        assert_eq!(
            plan.args[separator + 1],
            request.runner.binary_path.to_string_lossy().into_owned()
        );
    }

    #[test]
    fn mangohud_enabled_without_binary_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.tuning = crate::LaunchTuning {
            enable_mangohud: true,
            mangohud_path: None,
            ..crate::LaunchTuning::default()
        };
        assert!(matches!(
            validate(&request),
            Err(LaunchError::MangohudUnavailable)
        ));
        assert!(matches!(
            build_plan(&request),
            Err(LaunchError::MangohudUnavailable)
        ));
    }
}
