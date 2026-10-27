use application::{
    AppService, DeleteTarget, Game, LaunchEvent, LaunchRequest, LaunchTuning, Runner,
    SETTING_DEFAULT_RUNNER_ID, SETTING_GLOBAL_ENV, SETTING_PREFIX_ROOT,
};
use launcher_core::{Arch, DisplayConfig, RunnerKind, RunnerSource};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use uuid::Uuid;

static EXEC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn write_executable(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

/// Creates an initialized prefix, a fake runner script, and a matching request.
fn launch_fixture(kind: RunnerKind, body: &str) -> (tempfile::TempDir, Game, LaunchRequest) {
    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();

    let prefix = if kind == RunnerKind::Proton {
        let prefix = dir.path().join("compat");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "#arch=win64\n").unwrap();
        prefix
    } else {
        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();
        prefix
    };

    let binary = dir.path().join(if kind == RunnerKind::Proton {
        "proton"
    } else {
        "wine"
    });
    write_executable(&binary, body);

    let game = Game {
        id: Uuid::new_v4(),
        name: "Test".into(),
        executable_path: exe,
        working_dir: Some(dir.path().to_path_buf()),
        runner_id: None,
        prefix_path: Some(prefix),
        arch: Arch::Win64,
        display: DisplayConfig::default(),
        env_overrides: BTreeMap::new(),
        dll_overrides: Vec::new(),
        args: Vec::new(),
        created_at: 1,
        last_played_at: None,
    };
    let runner = Runner {
        id: Uuid::new_v4(),
        kind,
        name: "fake".into(),
        binary_path: binary,
        root_dir: dir.path().to_path_buf(),
        arch: Arch::Win64,
        source: RunnerSource::System,
        verified: true,
        version_string: None,
    };
    let request = LaunchRequest {
        game: game.clone(),
        runner,
        session: application::SessionType::X11,
        global_env: BTreeMap::new(),
        inherited_env: BTreeMap::new(),
        steam_compat_client: dir.path().join("steam-client"),
        default_prefix_root: dir.path().join("prefixes"),
        gamescope_path: None,
        tuning: LaunchTuning::default(),
        log_path: None,
        dxvk_nvapi_source: None,
    };
    (dir, game, request)
}

fn drain_events(rx: &mut tokio::sync::mpsc::UnboundedReceiver<LaunchEvent>) -> Vec<LaunchEvent> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

async fn read_until(path: &Path, needle: &str) -> String {
    for _ in 0..200 {
        if let Ok(text) = fs::read_to_string(path) {
            if text.contains(needle) {
                return text;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("{path:?} never contained {needle:?}");
}

fn game(path: PathBuf) -> Game {
    Game {
        id: Uuid::new_v4(),
        name: "Test".into(),
        executable_path: path,
        working_dir: None,
        runner_id: None,
        prefix_path: None,
        arch: Arch::Win64,
        display: DisplayConfig::default(),
        env_overrides: BTreeMap::new(),
        dll_overrides: Vec::new(),
        args: Vec::new(),
        created_at: 1,
        last_played_at: None,
    }
}

fn runner_value(path: PathBuf) -> Runner {
    Runner {
        id: Uuid::new_v4(),
        kind: RunnerKind::Wine,
        name: "Wine".into(),
        binary_path: path,
        root_dir: PathBuf::from("/usr/bin"),
        arch: Arch::Win64,
        source: RunnerSource::System,
        verified: true,
        version_string: None,
    }
}

#[tokio::test]
async fn save_game_inserts_then_updates() {
    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();
    let service = AppService::open_in_memory().unwrap();

    let mut value = game(exe);
    let saved = service
        .clone()
        .save_game(value.clone(), true)
        .await
        .unwrap();
    assert_eq!(saved, value);

    value.name = "Renamed".into();
    service
        .clone()
        .save_game(value.clone(), false)
        .await
        .unwrap();
    let data = service.load_initial().await.unwrap();
    assert_eq!(data.games[0].name, "Renamed");
}

#[tokio::test]
async fn save_game_rejects_missing_executable() {
    let dir = tempdir().unwrap();
    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path().join("missing.exe"));
    assert!(service.save_game(value, true).await.is_err());
}

#[tokio::test]
async fn saved_game_survives_reopen() {
    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();
    let db_path = dir.path().join("gamehub.db");

    let value = game(exe);
    {
        let service = AppService::open(&db_path).unwrap();
        service
            .clone()
            .save_game(value.clone(), true)
            .await
            .unwrap();
    }

    let service = AppService::open(&db_path).unwrap();
    let data = service.load_initial().await.unwrap();
    assert_eq!(data.games, vec![value]);
}

#[tokio::test]
async fn load_initial_reads_database_state() {
    let service = AppService::open_in_memory().unwrap();
    service
        .clone()
        .save_settings(BTreeMap::from([(
            SETTING_PREFIX_ROOT.to_string(),
            Some("/tmp/prefixes".to_string()),
        )]))
        .await
        .unwrap();

    let data = service.load_initial().await.unwrap();
    assert_eq!(
        data.settings_map.get(SETTING_PREFIX_ROOT),
        Some(&"/tmp/prefixes".to_string())
    );
}

#[tokio::test]
async fn persist_runners_updates_existing_binary_path() {
    let service = AppService::open_in_memory().unwrap();
    let runner = runner_value(PathBuf::from("/usr/bin/wine"));
    let stored = service
        .clone()
        .persist_runners(vec![runner.clone()])
        .await
        .unwrap();
    assert_eq!(stored.len(), 1);

    let mut duplicate = Runner {
        id: Uuid::new_v4(),
        ..runner
    };
    duplicate.version_string = Some("wine-10.0".into());
    let stored = service.persist_runners(vec![duplicate]).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].version_string.as_deref(), Some("wine-10.0"));
}

#[tokio::test]
async fn settings_round_trip_through_service() {
    let service = AppService::open_in_memory().unwrap();
    let values = BTreeMap::from([
        (
            SETTING_GLOBAL_ENV.to_string(),
            Some(r#"{"DXVK_HUD":"fps"}"#.to_string()),
        ),
        (
            SETTING_PREFIX_ROOT.to_string(),
            Some("/tmp/prefixes".to_string()),
        ),
        (SETTING_DEFAULT_RUNNER_ID.to_string(), None),
    ]);
    service.clone().save_settings(values).await.unwrap();

    let stored = service.load_initial().await.unwrap().settings_map;
    assert_eq!(stored[SETTING_GLOBAL_ENV], r#"{"DXVK_HUD":"fps"}"#);
    assert_eq!(stored[SETTING_PREFIX_ROOT], "/tmp/prefixes");
    assert!(!stored.contains_key(SETTING_DEFAULT_RUNNER_ID));
}

#[tokio::test]
async fn deleting_single_use_prefix_clears_game_reference_and_directory() {
    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();
    let prefix = dir.path().join("prefix");
    fs::create_dir(&prefix).unwrap();
    fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

    let service = AppService::open_in_memory().unwrap();
    let mut value = game(exe);
    value.prefix_path = Some(prefix.clone());
    service
        .clone()
        .save_game(value.clone(), true)
        .await
        .unwrap();

    service
        .clone()
        .delete(
            DeleteTarget::Prefix {
                game_id: value.id,
                path: prefix.clone(),
            },
            HashMap::new(),
            None,
        )
        .await
        .unwrap();

    assert!(!prefix.exists());
    let data = service.load_initial().await.unwrap();
    assert_eq!(data.games[0].prefix_path, None);
}

#[tokio::test]
async fn deleting_running_prefix_is_rejected() {
    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();
    let prefix = dir.path().join("prefix");
    fs::create_dir(&prefix).unwrap();
    fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

    let service = AppService::open_in_memory().unwrap();
    let mut value = game(exe);
    value.prefix_path = Some(prefix.clone());
    service
        .clone()
        .save_game(value.clone(), true)
        .await
        .unwrap();

    let result = service
        .clone()
        .delete(
            DeleteTarget::Prefix {
                game_id: value.id,
                path: prefix.clone(),
            },
            HashMap::from([(value.id, prefix.clone())]),
            None,
        )
        .await;

    assert!(result.is_err());
    assert!(prefix.exists());
    let data = service.load_initial().await.unwrap();
    assert!(data.games[0].prefix_path.is_some());
}

#[tokio::test]
async fn deleting_shared_prefix_is_rejected() {
    let dir = tempdir().unwrap();
    let exe_a = dir.path().join("a.exe");
    let exe_b = dir.path().join("b.exe");
    fs::write(&exe_a, b"MZ").unwrap();
    fs::write(&exe_b, b"MZ").unwrap();
    let prefix = dir.path().join("shared");
    fs::create_dir(&prefix).unwrap();
    fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

    let service = AppService::open_in_memory().unwrap();
    let mut a = game(exe_a);
    a.prefix_path = Some(prefix.clone());
    let mut b = game(exe_b);
    b.prefix_path = Some(prefix.clone());
    service.clone().save_game(a.clone(), true).await.unwrap();
    service.clone().save_game(b, true).await.unwrap();

    let result = service
        .delete(
            DeleteTarget::Prefix {
                game_id: a.id,
                path: prefix.clone(),
            },
            HashMap::new(),
            None,
        )
        .await;
    assert!(result.is_err());
    assert!(prefix.exists());
}

#[tokio::test]
async fn deleting_referenced_or_default_runner_is_rejected() {
    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();
    let service = AppService::open_in_memory().unwrap();
    let runner = runner_value(dir.path().join("wine"));
    let default_only = runner_value(dir.path().join("wine-default"));
    service
        .clone()
        .persist_runners(vec![runner.clone(), default_only.clone()])
        .await
        .unwrap();
    let mut value = game(exe);
    value.runner_id = Some(runner.id);
    service.clone().save_game(value, true).await.unwrap();

    assert!(
        service
            .clone()
            .delete(DeleteTarget::Runner(runner.id), HashMap::new(), None,)
            .await
            .is_err()
    );

    assert!(
        service
            .delete(
                DeleteTarget::Runner(default_only.id),
                HashMap::new(),
                Some(default_only.id),
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn deleting_game_removes_row_and_rejects_running() {
    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();
    let service = AppService::open_in_memory().unwrap();
    let value = game(exe);
    service
        .clone()
        .save_game(value.clone(), true)
        .await
        .unwrap();

    let mut running = HashMap::new();
    running.insert(value.id, PathBuf::from("/tmp/prefix"));
    assert!(
        service
            .clone()
            .delete(DeleteTarget::Game(value.id), running, None)
            .await
            .is_err()
    );

    service
        .clone()
        .delete(DeleteTarget::Game(value.id), HashMap::new(), None)
        .await
        .unwrap();
    assert!(service.load_initial().await.unwrap().games.is_empty());
}

#[tokio::test]
async fn launch_runs_fake_wine_and_touches_last_played() {
    use application::SessionType;
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let exe = dir.path().join("game.exe");
    fs::write(&exe, b"MZ").unwrap();

    let prefix = dir.path().join("prefix");
    fs::create_dir(&prefix).unwrap();
    fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

    let wine = dir.path().join("wine");
    fs::write(&wine, "#!/bin/sh\nexit 7\n").unwrap();
    let mut permissions = fs::metadata(&wine).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&wine, permissions).unwrap();

    let game = Game {
        id: Uuid::new_v4(),
        name: "Test".into(),
        executable_path: exe,
        working_dir: Some(dir.path().to_path_buf()),
        runner_id: None,
        prefix_path: Some(prefix),
        arch: Arch::Win64,
        display: DisplayConfig::default(),
        env_overrides: BTreeMap::new(),
        dll_overrides: Vec::new(),
        args: Vec::new(),
        created_at: 1,
        last_played_at: None,
    };
    let runner = Runner {
        id: Uuid::new_v4(),
        kind: RunnerKind::Wine,
        name: "fake".into(),
        binary_path: wine,
        root_dir: dir.path().to_path_buf(),
        arch: Arch::Win64,
        source: RunnerSource::System,
        verified: true,
        version_string: None,
    };
    let request = LaunchRequest {
        game: game.clone(),
        runner,
        session: SessionType::X11,
        global_env: BTreeMap::new(),
        inherited_env: BTreeMap::new(),
        steam_compat_client: dir.path().join("steam-client"),
        default_prefix_root: dir.path().join("prefixes"),
        gamescope_path: None,
        tuning: LaunchTuning::default(),
        log_path: None,
        dxvk_nvapi_source: None,
    };

    let service = AppService::open_in_memory().unwrap();
    service.clone().save_game(game.clone(), true).await.unwrap();

    let outcome = service.clone().launch_game(request).await.unwrap();
    assert_eq!(outcome.exit_code, Some(7));
    assert!(!outcome.success);
    let stored = service.load_initial().await.unwrap().games;
    assert!(stored[0].last_played_at.is_some());
}

#[tokio::test]
async fn launch_emits_started_then_finished() {
    let _guard = EXEC_LOCK.lock().await;
    let (_dir, game, request) = launch_fixture(RunnerKind::Wine, "#!/bin/sh\nexit 0\n");
    let service = AppService::open_in_memory().unwrap();
    service.clone().save_game(game.clone(), true).await.unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let outcome = service
        .clone()
        .launch_game_with_events(request, tx)
        .await
        .unwrap();
    assert!(outcome.success);

    let events = drain_events(&mut rx);
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(events[0], LaunchEvent::Started { game_id } if game_id == game.id));
    match &events[1] {
        LaunchEvent::Finished { game_id, outcome } => {
            assert_eq!(*game_id, game.id);
            assert!(outcome.success);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn stopping_a_running_game_terminates_it() {
    let _guard = EXEC_LOCK.lock().await;
    let (_dir, game, request) = launch_fixture(RunnerKind::Wine, "#!/bin/sh\nsleep 30\n");
    let service = AppService::open_in_memory().unwrap();
    service.clone().save_game(game.clone(), true).await.unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let launch = tokio::spawn(service.clone().launch_game_with_events(request, tx));

    let mut started = false;
    while let Some(event) = rx.recv().await {
        if matches!(event, LaunchEvent::Started { game_id } if game_id == game.id) {
            started = true;
            break;
        }
    }
    assert!(started, "launch never reported Started");

    service.clone().stop_game(game.id).await.unwrap();
    let outcome = launch.await.unwrap().unwrap();
    assert!(outcome.stopped);
    assert!(!outcome.success);

    let mut finished = None;
    while let Some(event) = rx.recv().await {
        if let LaunchEvent::Finished { outcome, .. } = event {
            finished = Some(outcome);
        }
    }
    assert!(finished.is_some_and(|outcome| outcome.stopped));
}

fn process_exists(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

async fn read_detached_pid(path: &Path) -> u32 {
    for _ in 0..200 {
        if let Ok(text) = fs::read_to_string(path) {
            if let Ok(pid) = text.trim().parse() {
                return pid;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("{path:?} never contained a pid");
}

#[tokio::test]
async fn stopping_kills_processes_detached_from_the_launcher_group() {
    let _guard = EXEC_LOCK.lock().await;
    let (dir, game, request) = launch_fixture(
        RunnerKind::Wine,
        "#!/bin/sh\nsetsid sleep 30 &\necho $! > detached.pid\nwait\n",
    );
    let pidfile = dir.path().join("detached.pid");
    write_executable(
        &dir.path().join("wineserver"),
        &format!(
            "#!/bin/sh\npid=$(cat '{}' 2>/dev/null)\nif [ -n \"$pid\" ]; then kill \"$pid\" 2>/dev/null; fi\nexit 0\n",
            pidfile.display()
        ),
    );

    let service = AppService::open_in_memory().unwrap();
    service.clone().save_game(game.clone(), true).await.unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let launch = tokio::spawn(service.clone().launch_game_with_events(request, tx));
    let started = loop {
        match rx.recv().await {
            Some(LaunchEvent::Started { game_id }) if game_id == game.id => break true,
            Some(_) => {}
            None => break false,
        }
    };
    assert!(started, "launch never reported Started");

    let detached_pid = read_detached_pid(&pidfile).await;
    assert!(process_exists(detached_pid));

    service.clone().stop_game(game.id).await.unwrap();
    let outcome = launch.await.unwrap().unwrap();
    assert!(outcome.stopped);

    for _ in 0..200 {
        if !process_exists(detached_pid) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("detached process {detached_pid} is still running");
}

#[tokio::test]
async fn failed_start_emits_only_failed() {
    let (_dir, mut game, mut request) = launch_fixture(RunnerKind::Wine, "#!/bin/sh\nexit 0\n");
    game.executable_path = game.executable_path.with_file_name("missing.exe");
    request.game = game.clone();

    let service = AppService::open_in_memory().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    assert!(
        service
            .clone()
            .launch_game_with_events(request, tx)
            .await
            .is_err()
    );

    let events = drain_events(&mut rx);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(matches!(
        &events[0],
        LaunchEvent::Failed { game_id, error }
            if *game_id == game.id && error.contains("executable")
    ));
}

#[tokio::test]
async fn wine_nvapi_without_source_fails_before_spawn() {
    let _guard = EXEC_LOCK.lock().await;
    let (dir, _game, mut request) =
        launch_fixture(RunnerKind::Wine, "#!/bin/sh\ntouch wine-ran.txt\nexit 0\n");
    request.tuning = LaunchTuning {
        enable_dxvk_nvapi: true,
        ..LaunchTuning::default()
    };

    let service = AppService::open_in_memory().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    assert!(
        service
            .clone()
            .launch_game_with_events(request, tx)
            .await
            .is_err()
    );

    assert!(!dir.path().join("wine-ran.txt").exists());
    let events = drain_events(&mut rx);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(matches!(
        &events[0],
        LaunchEvent::Failed { error, .. } if error.contains("DXVK-NVAPI")
    ));
}

#[tokio::test]
async fn wine_nvapi_invalid_source_aborts_launch() {
    let _guard = EXEC_LOCK.lock().await;
    let (dir, _game, mut request) =
        launch_fixture(RunnerKind::Wine, "#!/bin/sh\ntouch wine-ran.txt\nexit 0\n");
    let source = dir.path().join("nvapi");
    fs::create_dir_all(source.join("x64")).unwrap();
    request.tuning = LaunchTuning {
        enable_dxvk_nvapi: true,
        ..LaunchTuning::default()
    };
    request.dxvk_nvapi_source = Some(source);

    let service = AppService::open_in_memory().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    assert!(
        service
            .clone()
            .launch_game_with_events(request, tx)
            .await
            .is_err()
    );

    assert!(!dir.path().join("wine-ran.txt").exists());
    let events = drain_events(&mut rx);
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(matches!(
        &events[0],
        LaunchEvent::Failed { error, .. } if error.contains("nvapi64.dll")
    ));
}

#[tokio::test]
async fn proton_injects_nvapi_env_without_copying_dlls() {
    let _guard = EXEC_LOCK.lock().await;
    let (dir, _game, mut request) = launch_fixture(
        RunnerKind::Proton,
        "#!/bin/sh\nprintf '%s' \"$PROTON_ENABLE_NVAPI\" > proton-env.txt\nexit 0\n",
    );
    request.tuning = LaunchTuning {
        enable_dxvk_nvapi: true,
        ..LaunchTuning::default()
    };

    let service = AppService::open_in_memory().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let outcome = service
        .clone()
        .launch_game_with_events(request, tx)
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(
        fs::read_to_string(dir.path().join("proton-env.txt")).unwrap(),
        "1"
    );
    assert!(!dir.path().join("compat/.gamehub-dxvk-nvapi").exists());
    assert_eq!(drain_events(&mut rx).len(), 2);
}

#[tokio::test]
async fn launch_writes_log_to_request_path() {
    let _guard = EXEC_LOCK.lock().await;
    let (dir, game, mut request) = launch_fixture(
        RunnerKind::Wine,
        "#!/bin/sh\necho hello-from-game\nexit 0\n",
    );
    let log = dir
        .path()
        .join("logs/games")
        .join(format!("{}.log", game.id));
    request.log_path = Some(log.clone());

    let service = AppService::open_in_memory().unwrap();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    service
        .clone()
        .launch_game_with_events(request, tx)
        .await
        .unwrap();

    let text = read_until(&log, "hello-from-game").await;
    assert!(text.contains("Running command:"), "{text}");
}
