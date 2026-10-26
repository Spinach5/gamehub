mod error;
mod mapping;
mod schema;

use std::path::Path;

use mapping::{optional_path_to_string, path_to_string, row_to_game};
use rusqlite::Connection;
use rusqlite::params;
use uuid::Uuid;

pub use error::StorageError;
pub use launcher_core::{AppPaths, Game, Runner};

const GAME_COLUMNS: &str = "id, name, executable_path, working_dir, runner_id, prefix_path, \
                            arch, display_json, env_overrides_json, dll_overrides_json, \
                            args_json, created_at, last_played_at";

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Ok(Self {
            conn: schema::initialize(conn)?,
        })
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        Ok(Self {
            conn: schema::initialize(conn)?,
        })
    }

    pub fn insert_game(&self, game: &Game) -> Result<(), StorageError> {
        let id = game.id.to_string();
        let executable_path = path_to_string(&game.executable_path, "games.executable_path")?;
        let working_dir =
            optional_path_to_string(game.working_dir.as_deref(), "games.working_dir")?;
        let runner_id = game.runner_id.map(|id| id.to_string());
        let prefix_path =
            optional_path_to_string(game.prefix_path.as_deref(), "games.prefix_path")?;
        let display_json = serde_json::to_string(&game.display)?;
        let env_overrides_json = serde_json::to_string(&game.env_overrides)?;
        let dll_overrides_json = serde_json::to_string(&game.dll_overrides)?;
        let args_json = serde_json::to_string(&game.args)?;

        self.conn
            .execute(
                "INSERT INTO games (id, name, executable_path, working_dir, runner_id, prefix_path,
                                    arch, display_json, env_overrides_json, dll_overrides_json,
                                    args_json, created_at, last_played_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    id,
                    game.name,
                    executable_path,
                    working_dir,
                    runner_id,
                    prefix_path,
                    game.arch.as_str(),
                    display_json,
                    env_overrides_json,
                    dll_overrides_json,
                    args_json,
                    game.created_at,
                    game.last_played_at,
                ],
            )
            .map_err(|error| map_game_insert_error(error, game.id))?;
        Ok(())
    }

    pub fn update_game(&self, game: &Game) -> Result<(), StorageError> {
        let executable_path = path_to_string(&game.executable_path, "games.executable_path")?;
        let working_dir =
            optional_path_to_string(game.working_dir.as_deref(), "games.working_dir")?;
        let runner_id = game.runner_id.map(|id| id.to_string());
        let prefix_path =
            optional_path_to_string(game.prefix_path.as_deref(), "games.prefix_path")?;
        let display_json = serde_json::to_string(&game.display)?;
        let env_overrides_json = serde_json::to_string(&game.env_overrides)?;
        let dll_overrides_json = serde_json::to_string(&game.dll_overrides)?;
        let args_json = serde_json::to_string(&game.args)?;

        let changed = self.conn.execute(
            "UPDATE games SET name = ?2, executable_path = ?3, working_dir = ?4, runner_id = ?5,
                              prefix_path = ?6, arch = ?7, display_json = ?8,
                              env_overrides_json = ?9, dll_overrides_json = ?10, args_json = ?11,
                              created_at = ?12, last_played_at = ?13
             WHERE id = ?1",
            params![
                game.id.to_string(),
                game.name,
                executable_path,
                working_dir,
                runner_id,
                prefix_path,
                game.arch.as_str(),
                display_json,
                env_overrides_json,
                dll_overrides_json,
                args_json,
                game.created_at,
                game.last_played_at,
            ],
        )?;

        if changed == 0 {
            return Err(StorageError::GameNotFound(game.id));
        }
        Ok(())
    }

    pub fn delete_game(&self, id: Uuid) -> Result<(), StorageError> {
        let changed = self
            .conn
            .execute("DELETE FROM games WHERE id = ?1", params![id.to_string()])?;
        if changed == 0 {
            return Err(StorageError::GameNotFound(id));
        }
        Ok(())
    }

    pub fn get_game(&self, id: Uuid) -> Result<Option<Game>, StorageError> {
        let sql = format!("SELECT {GAME_COLUMNS} FROM games WHERE id = ?1");
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows = statement.query(params![id.to_string()])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_game(row)?)),
            None => Ok(None),
        }
    }

    pub fn list_games(&self) -> Result<Vec<Game>, StorageError> {
        let sql = format!("SELECT {GAME_COLUMNS} FROM games ORDER BY name COLLATE NOCASE, id");
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows = statement.query([])?;
        let mut games = Vec::new();
        while let Some(row) = rows.next()? {
            games.push(row_to_game(row)?);
        }
        Ok(games)
    }

    pub fn touch_last_played(&self, id: Uuid, timestamp: i64) -> Result<(), StorageError> {
        let changed = self.conn.execute(
            "UPDATE games SET last_played_at = ?2 WHERE id = ?1",
            params![id.to_string(), timestamp],
        )?;
        if changed == 0 {
            return Err(StorageError::GameNotFound(id));
        }
        Ok(())
    }
}

fn map_game_insert_error(error: rusqlite::Error, id: Uuid) -> StorageError {
    match &error {
        rusqlite::Error::SqliteFailure(code, message)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            if message.as_deref().is_some_and(|m| m.contains("games.id")) {
                return StorageError::GameAlreadyExists(id);
            }
            StorageError::Sqlite(error)
        }
        _ => StorageError::Sqlite(error),
    }
}

pub fn ensure_app_dirs(paths: &AppPaths) -> Result<(), StorageError> {
    for dir in [
        &paths.data_root,
        &paths.logs_dir,
        &paths.default_prefix_root,
        &paths.steam_compat_client,
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, DisplayConfig, DllOverride, GamescopeMode, OverrideMode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn sample_game(name: &str) -> Game {
        Game {
            id: Uuid::new_v4(),
            name: name.into(),
            executable_path: PathBuf::from("/games/Test/game.exe"),
            working_dir: Some(PathBuf::from("/games/Test")),
            runner_id: Some(Uuid::new_v4()),
            prefix_path: Some(PathBuf::from("/data/prefixes/test")),
            arch: Arch::Win64,
            display: DisplayConfig {
                gamescope: GamescopeMode::Forced,
                resolution: Some((2560, 1440)),
                ..DisplayConfig::default()
            },
            env_overrides: BTreeMap::from([("DXVK_HUD".into(), "fps".into())]),
            dll_overrides: vec![DllOverride {
                dll_name: "d3d11".into(),
                mode: OverrideMode::NativeBuiltin,
            }],
            args: vec!["--windowed".into()],
            created_at: 1_700_000_000,
            last_played_at: Some(1_700_000_100),
        }
    }

    #[test]
    fn game_round_trips_all_fields() {
        let db = Db::open_in_memory().unwrap();
        let game = sample_game("赛博朋克 2077");
        db.insert_game(&game).unwrap();
        assert_eq!(db.get_game(game.id).unwrap(), Some(game.clone()));
        assert_eq!(db.list_games().unwrap(), vec![game]);
    }

    #[test]
    fn games_are_listed_case_insensitively_by_name() {
        let db = Db::open_in_memory().unwrap();
        db.insert_game(&sample_game("zeta")).unwrap();
        db.insert_game(&sample_game("Alpha")).unwrap();
        let names: Vec<String> = db
            .list_games()
            .unwrap()
            .into_iter()
            .map(|game| game.name)
            .collect();
        assert_eq!(names, vec!["Alpha", "zeta"]);
    }

    #[test]
    fn update_touch_and_delete_game() {
        let db = Db::open_in_memory().unwrap();
        let mut game = sample_game("Test");
        db.insert_game(&game).unwrap();

        game.name = "Renamed".into();
        db.update_game(&game).unwrap();
        assert_eq!(db.get_game(game.id).unwrap().unwrap().name, "Renamed");

        db.touch_last_played(game.id, 1_800_000_000).unwrap();
        assert_eq!(
            db.get_game(game.id).unwrap().unwrap().last_played_at,
            Some(1_800_000_000)
        );

        db.delete_game(game.id).unwrap();
        assert_eq!(db.get_game(game.id).unwrap(), None);
        assert!(matches!(
            db.delete_game(game.id),
            Err(StorageError::GameNotFound(_))
        ));
    }

    #[test]
    fn duplicate_game_id_is_rejected() {
        let db = Db::open_in_memory().unwrap();
        let game = sample_game("Test");
        db.insert_game(&game).unwrap();
        assert!(matches!(
            db.insert_game(&game),
            Err(StorageError::GameAlreadyExists(_))
        ));
    }

    #[test]
    fn missing_game_updates_fail() {
        let db = Db::open_in_memory().unwrap();
        let game = sample_game("Missing");
        assert!(matches!(
            db.update_game(&game),
            Err(StorageError::GameNotFound(_))
        ));
        assert!(matches!(
            db.touch_last_played(game.id, 1),
            Err(StorageError::GameNotFound(_))
        ));
    }

    #[test]
    fn in_memory_database_has_v1_schema() {
        let db = Db::open_in_memory().unwrap();
        let version: i64 = db
            .conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);

        let tables: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('games', 'runners', 'settings')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 3);
    }

    #[test]
    fn open_creates_missing_parent_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested/data/gamehub.db");
        let db = Db::open(&path).unwrap();
        drop(db);
        assert!(path.exists());
    }

    #[test]
    fn newer_schema_is_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("gamehub.db");
        {
            let db = Db::open(&path).unwrap();
            db.conn
                .execute("UPDATE schema_version SET version = 2", [])
                .unwrap();
        }

        let result = Db::open(&path);
        match result {
            Err(StorageError::UnsupportedSchema {
                found: 2,
                supported: 1,
            }) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected unsupported schema error"),
        }
    }

    #[test]
    fn ensure_app_dirs_creates_all_paths() {
        let dir = tempdir().unwrap();
        let paths = AppPaths::for_root(dir.path().join("data"));
        ensure_app_dirs(&paths).unwrap();
        assert!(paths.data_root.is_dir());
        assert!(paths.logs_dir.is_dir());
        assert!(paths.default_prefix_root.is_dir());
        assert!(paths.steam_compat_client.is_dir());
    }
}
