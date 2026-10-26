use display::{SessionType, X11Output};
use launcher_core::{Game, Runner};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use storage::Db;

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

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, DisplayConfig, Game};
    use std::collections::BTreeMap;
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
}
