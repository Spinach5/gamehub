use crate::app::{DeleteTarget, LaunchOutcome};
use display::{SessionType, X11Output};
use launcher_core::{Game, Runner};
use prefix::{DxvkSource, PrefixPlan, PrefixStatus};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use storage::Db;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LoadedData {
    pub games: Vec<Game>,
    pub runners: Vec<Runner>,
    pub settings_map: BTreeMap<String, String>,
    pub session: SessionType,
    pub x11_outputs: Vec<X11Output>,
    pub gamescope_path: Option<PathBuf>,
}

pub async fn load_initial(db: Arc<Mutex<Db>>) -> Result<LoadedData, String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let games = db.list_games().map_err(|error| error.to_string())?;
        let runners = db.list_runners().map_err(|error| error.to_string())?;
        let settings_map = db.get_all_settings().map_err(|error| error.to_string())?;

        Ok(LoadedData {
            games,
            runners,
            settings_map,
            session: display::detect_session(),
            x11_outputs: display::list_x11_outputs(),
            gamescope_path: display::gamescope_path(),
        })
    })
    .await
    .map_err(|error| format!("加载任务失败: {error}"))?
}

pub async fn save_game(db: Arc<Mutex<Db>>, game: Game, is_new: bool) -> Result<Game, String> {
    tokio::task::spawn_blocking(move || {
        if !game.executable_path.is_file() {
            return Err(format!(
                "游戏可执行文件不存在: {}",
                game.executable_path.display()
            ));
        }
        if let Some(working_dir) = &game.working_dir {
            if !working_dir.is_dir() {
                return Err(format!("工作目录不存在: {}", working_dir.display()));
            }
        }

        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        if is_new {
            db.insert_game(&game).map_err(|error| error.to_string())?;
        } else {
            db.update_game(&game).map_err(|error| error.to_string())?;
        }
        Ok(game)
    })
    .await
    .map_err(|error| format!("保存任务失败: {error}"))?
}

pub async fn prefix_status(plan: PrefixPlan) -> PrefixStatus {
    tokio::task::spawn_blocking(move || prefix::status(&plan))
        .await
        .unwrap_or(PrefixStatus::NotInitialized)
}

pub async fn create_prefix(plan: PrefixPlan, dxvk: Option<DxvkSource>) -> Result<PathBuf, String> {
    plan.initialize().await.map_err(|error| error.to_string())?;
    if let Some(source) = dxvk {
        prefix::install_dxvk(&plan, &source)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(plan.path)
}

pub async fn install_dxvk(plan: PrefixPlan, source: DxvkSource) -> Result<(), String> {
    prefix::install_dxvk(&plan, &source)
        .await
        .map_err(|error| error.to_string())
}

pub async fn detect_and_persist(db: Arc<Mutex<Db>>) -> Result<Vec<Runner>, String> {
    let found = runner_wine::detect_system_wine().await;
    persist_runners(db, found).await
}

pub async fn persist_runners(
    db: Arc<Mutex<Db>>,
    runners: Vec<Runner>,
) -> Result<Vec<Runner>, String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        for mut runner in runners {
            let existing = db
                .list_runners()
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|existing| existing.binary_path == runner.binary_path);

            if let Some(existing) = existing {
                runner.id = existing.id;
                db.update_runner(&runner)
                    .map_err(|error| error.to_string())?;
            } else {
                match db.insert_runner(&runner) {
                    Ok(()) | Err(storage::StorageError::DuplicateRunner { .. }) => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        db.list_runners().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Runner 探测任务失败: {error}"))?
}

pub async fn import_proton(db: Arc<Mutex<Db>>, dir: PathBuf) -> Result<Runner, String> {
    tokio::task::spawn_blocking(move || {
        let runner = runner_wine::validate_proton_dir(&dir).map_err(|error| error.to_string())?;
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.insert_runner(&runner)
            .map_err(|error| error.to_string())?;
        Ok(runner)
    })
    .await
    .map_err(|error| format!("Proton 导入任务失败: {error}"))?
}

pub async fn save_default_runner(db: Arc<Mutex<Db>>, id: Option<Uuid>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        match id {
            Some(id) => db
                .set_setting(crate::app::SETTING_DEFAULT_RUNNER_ID, &id.to_string())
                .map_err(|error| error.to_string()),
            None => db
                .delete_setting(crate::app::SETTING_DEFAULT_RUNNER_ID)
                .map_err(|error| error.to_string()),
        }
    })
    .await
    .map_err(|error| format!("默认 Runner 保存任务失败: {error}"))?
}

pub async fn save_settings(
    db: Arc<Mutex<Db>>,
    values: BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        for (key, value) in values {
            match value {
                Some(value) => db
                    .set_setting(&key, &value)
                    .map_err(|error| error.to_string())?,
                None => db.delete_setting(&key).map_err(|error| error.to_string())?,
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("设置保存任务失败: {error}"))?
}

pub async fn launch_game(
    db: Arc<Mutex<Db>>,
    request: launch::LaunchRequest,
) -> Result<LaunchOutcome, String> {
    let game_id = request.game.id;
    let plan_request = request.clone();

    let plan = tokio::task::spawn_blocking(move || launch::build_plan(&plan_request))
        .await
        .map_err(|error| format!("启动计划任务失败: {error}"))?
        .map_err(|error| error.to_string())?;

    let mut child = launch::spawn_plan(&plan)
        .await
        .map_err(|error| error.to_string())?;

    let wait_result = child.wait().await;
    let timestamp = chrono::Utc::now().timestamp();

    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(db) = db.lock() {
            let _ = db.touch_last_played(game_id, timestamp);
        }
    })
    .await;

    let status = wait_result.map_err(|error| format!("等待游戏进程失败: {error}"))?;
    Ok(LaunchOutcome {
        exit_code: status.code(),
        success: status.success(),
    })
}

pub async fn delete_target(
    db: Arc<Mutex<Db>>,
    target: DeleteTarget,
    running: HashMap<Uuid, PathBuf>,
    default_runner_id: Option<Uuid>,
) -> Result<(), String> {
    match target {
        DeleteTarget::Runner(id) => tokio::task::spawn_blocking(move || {
            if default_runner_id == Some(id) {
                return Err("该 Runner 是全局默认，请先更换默认 Runner".to_string());
            }
            let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
            let count = db
                .count_games_using_runner(id)
                .map_err(|error| error.to_string())?;
            if count > 0 {
                return Err(format!("仍有 {count} 个游戏使用该 Runner"));
            }
            db.delete_runner(id).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| format!("删除任务失败: {error}"))?,
        DeleteTarget::Prefix { game_id, path } => {
            if running.contains_key(&game_id)
                || running.values().any(|running_path| running_path == &path)
            {
                return Err("游戏正在运行，不能删除 Prefix".into());
            }

            let path_for_db = path.clone();
            tokio::task::spawn_blocking(move || {
                let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
                let count = db
                    .count_games_using_prefix(&path_for_db)
                    .map_err(|error| error.to_string())?;
                if count > 1 {
                    return Err("该 Prefix 被多个游戏共享，不能删除".to_string());
                }
                if count == 1 {
                    let mut game = db
                        .get_game(game_id)
                        .map_err(|error| error.to_string())?
                        .ok_or_else(|| "游戏不存在".to_string())?;
                    if game.prefix_path.as_deref() != Some(path_for_db.as_path()) {
                        return Err("该 Prefix 被其他游戏使用".to_string());
                    }
                    game.prefix_path = None;
                    db.update_game(&game).map_err(|error| error.to_string())?;
                }
                Ok::<(), String>(())
            })
            .await
            .map_err(|error| format!("删除任务失败: {error}"))??;

            prefix::delete_prefix(&path)
                .await
                .map_err(|error| error.to_string())
        }
        DeleteTarget::Game(id) => {
            if running.contains_key(&id) {
                return Err("游戏正在运行，不能删除".into());
            }
            tokio::task::spawn_blocking(move || {
                let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
                db.delete_game(id).map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| format!("删除任务失败: {error}"))?
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, DisplayConfig, Game, RunnerKind, RunnerSource};
    use std::collections::{BTreeMap, HashMap};
    use std::fs;
    use std::sync::{Arc, Mutex};
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

    #[tokio::test]
    async fn save_game_inserts_then_updates() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));

        let mut value = game(exe);
        let saved = save_game(db.clone(), value.clone(), true).await.unwrap();
        assert_eq!(saved, value);

        value.name = "Renamed".into();
        save_game(db.clone(), value.clone(), false).await.unwrap();
        let stored = db.lock().unwrap().get_game(value.id).unwrap().unwrap();
        assert_eq!(stored.name, "Renamed");
    }

    #[tokio::test]
    async fn save_game_rejects_missing_executable() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let value = game(dir.path().join("missing.exe"));
        assert!(save_game(db, value, true).await.is_err());
    }

    #[tokio::test]
    async fn saved_game_survives_reopen() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let db_path = dir.path().join("gamehub.db");

        let value = game(exe);
        {
            let db = Arc::new(Mutex::new(Db::open(&db_path).unwrap()));
            save_game(db, value.clone(), true).await.unwrap();
        }

        let db = Arc::new(Mutex::new(Db::open(&db_path).unwrap()));
        let data = load_initial(db).await.unwrap();
        assert_eq!(data.games, vec![value]);
    }

    #[tokio::test]
    async fn load_initial_reads_database_state() {
        let db = Db::open_in_memory().unwrap();
        db.set_setting(crate::app::SETTING_PREFIX_ROOT, "/tmp/prefixes")
            .unwrap();
        let db = Arc::new(Mutex::new(db));

        let data = load_initial(db).await.unwrap();
        assert_eq!(
            data.settings_map.get(crate::app::SETTING_PREFIX_ROOT),
            Some(&"/tmp/prefixes".to_string())
        );
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
    async fn persist_runners_updates_existing_binary_path() {
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let runner = runner_value(PathBuf::from("/usr/bin/wine"));
        let stored = persist_runners(db.clone(), vec![runner.clone()])
            .await
            .unwrap();
        assert_eq!(stored.len(), 1);

        let mut duplicate = Runner {
            id: Uuid::new_v4(),
            ..runner
        };
        duplicate.version_string = Some("wine-10.0".into());
        let stored = persist_runners(db, vec![duplicate]).await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].version_string.as_deref(), Some("wine-10.0"));
    }

    #[tokio::test]
    async fn settings_round_trip_through_tasks() {
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let values = BTreeMap::from([
            (
                crate::app::SETTING_GLOBAL_ENV.to_string(),
                Some(r#"{"DXVK_HUD":"fps"}"#.to_string()),
            ),
            (
                crate::app::SETTING_PREFIX_ROOT.to_string(),
                Some("/tmp/prefixes".to_string()),
            ),
            (crate::app::SETTING_DEFAULT_RUNNER_ID.to_string(), None),
        ]);
        save_settings(db.clone(), values).await.unwrap();

        let stored = db.lock().unwrap().get_all_settings().unwrap();
        assert_eq!(
            stored[crate::app::SETTING_GLOBAL_ENV],
            r#"{"DXVK_HUD":"fps"}"#
        );
        assert_eq!(stored[crate::app::SETTING_PREFIX_ROOT], "/tmp/prefixes");
        assert!(!stored.contains_key(crate::app::SETTING_DEFAULT_RUNNER_ID));
    }

    #[tokio::test]
    async fn deleting_single_use_prefix_clears_game_reference_and_directory() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        let db = Db::open_in_memory().unwrap();
        let mut value = game(exe);
        value.prefix_path = Some(prefix.clone());
        db.insert_game(&value).unwrap();
        let db = Arc::new(Mutex::new(db));

        delete_target(
            db.clone(),
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
        assert_eq!(
            db.lock()
                .unwrap()
                .get_game(value.id)
                .unwrap()
                .unwrap()
                .prefix_path,
            None
        );
    }

    #[tokio::test]
    async fn deleting_running_prefix_is_rejected() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        let db = Db::open_in_memory().unwrap();
        let mut value = game(exe);
        value.prefix_path = Some(prefix.clone());
        db.insert_game(&value).unwrap();
        let db = Arc::new(Mutex::new(db));

        let result = delete_target(
            db.clone(),
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
        assert!(
            db.lock()
                .unwrap()
                .get_game(value.id)
                .unwrap()
                .unwrap()
                .prefix_path
                .is_some()
        );
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

        let db = Db::open_in_memory().unwrap();
        let mut a = game(exe_a);
        a.prefix_path = Some(prefix.clone());
        let mut b = game(exe_b);
        b.prefix_path = Some(prefix.clone());
        db.insert_game(&a).unwrap();
        db.insert_game(&b).unwrap();
        let db = Arc::new(Mutex::new(db));

        let result = delete_target(
            db,
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
        let db = Db::open_in_memory().unwrap();
        let runner = runner_value(dir.path().join("wine"));
        let default_only = runner_value(dir.path().join("wine-default"));
        db.insert_runner(&runner).unwrap();
        db.insert_runner(&default_only).unwrap();
        let mut value = game(exe);
        value.runner_id = Some(runner.id);
        db.insert_game(&value).unwrap();
        let db = Arc::new(Mutex::new(db));

        assert!(
            delete_target(
                db.clone(),
                DeleteTarget::Runner(runner.id),
                HashMap::new(),
                None,
            )
            .await
            .is_err()
        );

        assert!(
            delete_target(
                db,
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
        let db = Db::open_in_memory().unwrap();
        let value = game(exe);
        db.insert_game(&value).unwrap();
        let db = Arc::new(Mutex::new(db));

        let mut running = HashMap::new();
        running.insert(value.id, PathBuf::from("/tmp/prefix"));
        assert!(
            delete_target(db.clone(), DeleteTarget::Game(value.id), running, None,)
                .await
                .is_err()
        );

        delete_target(
            db.clone(),
            DeleteTarget::Game(value.id),
            HashMap::new(),
            None,
        )
        .await
        .unwrap();
        assert!(db.lock().unwrap().get_game(value.id).unwrap().is_none());
    }

    #[tokio::test]
    async fn launch_runs_fake_wine_and_touches_last_played() {
        use display::SessionType;
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
        let request = launch::LaunchRequest {
            game: game.clone(),
            runner,
            session: SessionType::X11,
            global_env: BTreeMap::new(),
            inherited_env: BTreeMap::new(),
            steam_compat_client: dir.path().join("steam-client"),
            default_prefix_root: dir.path().join("prefixes"),
            gamescope_path: None,
        };

        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        db.lock().unwrap().insert_game(&game).unwrap();

        let outcome = launch_game(db.clone(), request).await.unwrap();
        assert_eq!(outcome.exit_code, Some(7));
        assert!(!outcome.success);
        let stored = db.lock().unwrap().get_game(game.id).unwrap().unwrap();
        assert!(stored.last_played_at.is_some());
    }
}
