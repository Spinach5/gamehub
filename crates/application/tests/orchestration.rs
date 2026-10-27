use application::{
    AppService, DeleteTarget, Game, LaunchRequest, Runner, SETTING_DEFAULT_RUNNER_ID,
    SETTING_GLOBAL_ENV, SETTING_PREFIX_ROOT,
};
use launcher_core::{Arch, DisplayConfig, RunnerKind, RunnerSource};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use uuid::Uuid;

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
    };

    let service = AppService::open_in_memory().unwrap();
    service.clone().save_game(game.clone(), true).await.unwrap();

    let outcome = service.clone().launch_game(request).await.unwrap();
    assert_eq!(outcome.exit_code, Some(7));
    assert!(!outcome.success);
    let stored = service.load_initial().await.unwrap().games;
    assert!(stored[0].last_played_at.is_some());
}
