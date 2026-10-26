mod error;
mod mapping;
mod schema;

use std::collections::BTreeMap;
use std::path::Path;

use mapping::{optional_path_to_string, path_to_string, row_to_game, row_to_runner};
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use rusqlite::params;
use uuid::Uuid;

pub use error::StorageError;
pub use launcher_core::{AppPaths, Game, Runner};

const GAME_COLUMNS: &str = "id, name, executable_path, working_dir, runner_id, prefix_path, \
                            arch, display_json, env_overrides_json, dll_overrides_json, \
                            args_json, created_at, last_played_at";

const RUNNER_COLUMNS: &str =
    "id, kind, name, binary_path, root_dir, arch, source_json, verified, version_string";

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

    pub fn insert_runner(&self, runner: &Runner) -> Result<(), StorageError> {
        let binary_path = path_to_string(&runner.binary_path, "runners.binary_path")?;
        let root_dir = path_to_string(&runner.root_dir, "runners.root_dir")?;
        let source_json = serde_json::to_string(&runner.source)?;

        self.conn
            .execute(
                "INSERT INTO runners (id, kind, name, binary_path, root_dir, arch, source_json,
                                      verified, version_string)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    runner.id.to_string(),
                    runner.kind.as_str(),
                    runner.name,
                    binary_path,
                    root_dir,
                    runner.arch.as_str(),
                    source_json,
                    if runner.verified { 1_i64 } else { 0_i64 },
                    runner.version_string,
                ],
            )
            .map_err(|error| map_runner_error(error, runner))?;
        Ok(())
    }

    pub fn update_runner(&self, runner: &Runner) -> Result<(), StorageError> {
        let binary_path = path_to_string(&runner.binary_path, "runners.binary_path")?;
        let root_dir = path_to_string(&runner.root_dir, "runners.root_dir")?;
        let source_json = serde_json::to_string(&runner.source)?;

        let changed = self
            .conn
            .execute(
                "UPDATE runners SET kind = ?2, name = ?3, binary_path = ?4, root_dir = ?5,
                                    arch = ?6, source_json = ?7, verified = ?8, version_string = ?9
                 WHERE id = ?1",
                params![
                    runner.id.to_string(),
                    runner.kind.as_str(),
                    runner.name,
                    binary_path,
                    root_dir,
                    runner.arch.as_str(),
                    source_json,
                    if runner.verified { 1_i64 } else { 0_i64 },
                    runner.version_string,
                ],
            )
            .map_err(|error| map_runner_error(error, runner))?;

        if changed == 0 {
            return Err(StorageError::RunnerNotFound(runner.id));
        }
        Ok(())
    }

    pub fn delete_runner(&self, id: Uuid) -> Result<(), StorageError> {
        let changed = self
            .conn
            .execute("DELETE FROM runners WHERE id = ?1", params![id.to_string()])?;
        if changed == 0 {
            return Err(StorageError::RunnerNotFound(id));
        }
        Ok(())
    }

    pub fn get_runner(&self, id: Uuid) -> Result<Option<Runner>, StorageError> {
        let sql = format!("SELECT {RUNNER_COLUMNS} FROM runners WHERE id = ?1");
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows = statement.query(params![id.to_string()])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_runner(row)?)),
            None => Ok(None),
        }
    }

    pub fn list_runners(&self) -> Result<Vec<Runner>, StorageError> {
        let sql = format!("SELECT {RUNNER_COLUMNS} FROM runners ORDER BY name COLLATE NOCASE, id");
        let mut statement = self.conn.prepare(&sql)?;
        let mut rows = statement.query([])?;
        let mut runners = Vec::new();
        while let Some(row) = rows.next()? {
            runners.push(row_to_runner(row)?);
        }
        Ok(runners)
    }

    pub fn count_games_using_runner(&self, runner_id: Uuid) -> Result<u64, StorageError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM games WHERE runner_id = ?1",
            params![runner_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn count_games_using_prefix(&self, prefix: &Path) -> Result<u64, StorageError> {
        let prefix = path_to_string(prefix, "games.prefix_path")?;
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM games WHERE prefix_path = ?1",
            params![prefix],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), StorageError> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<BTreeMap<String, String>, StorageError> {
        let mut statement = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key")?;
        let mut rows = statement.query([])?;
        let mut settings = BTreeMap::new();
        while let Some(row) = rows.next()? {
            settings.insert(row.get(0)?, row.get(1)?);
        }
        Ok(settings)
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

fn map_runner_error(error: rusqlite::Error, runner: &Runner) -> StorageError {
    match &error {
        rusqlite::Error::SqliteFailure(code, message)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            let message = message.as_deref().unwrap_or_default();
            if message.contains("runners.binary_path") {
                return StorageError::DuplicateRunner {
                    path: runner.binary_path.clone(),
                };
            }
            if message.contains("runners.id") {
                return StorageError::RunnerAlreadyExists(runner.id);
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
    use launcher_core::{
        Arch, DisplayConfig, DllOverride, GamescopeMode, OverrideMode, RunnerKind, RunnerSource,
    };
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

    fn sample_runner(name: &str, binary_path: &str) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Proton,
            name: name.into(),
            binary_path: PathBuf::from(binary_path),
            root_dir: PathBuf::from("/opt/GE-Proton9-1"),
            arch: Arch::Win64,
            source: RunnerSource::Imported {
                original_path: PathBuf::from("/opt/GE-Proton9-1"),
            },
            verified: true,
            version_string: Some("GE-Proton9-1".into()),
        }
    }

    #[test]
    fn runner_round_trips_all_fields() {
        let db = Db::open_in_memory().unwrap();
        let runner = sample_runner("GE-Proton9-1", "/opt/GE-Proton9-1/proton");
        db.insert_runner(&runner).unwrap();
        assert_eq!(db.get_runner(runner.id).unwrap(), Some(runner.clone()));
        assert_eq!(db.list_runners().unwrap(), vec![runner]);
    }

    #[test]
    fn duplicate_runner_binary_is_rejected() {
        let db = Db::open_in_memory().unwrap();
        db.insert_runner(&sample_runner("one", "/opt/proton"))
            .unwrap();
        let duplicate = sample_runner("two", "/opt/proton");
        assert!(matches!(
            db.insert_runner(&duplicate),
            Err(StorageError::DuplicateRunner { .. })
        ));
    }

    #[test]
    fn update_and_delete_runner() {
        let db = Db::open_in_memory().unwrap();
        let mut runner = sample_runner("one", "/opt/proton");
        db.insert_runner(&runner).unwrap();

        runner.name = "renamed".into();
        db.update_runner(&runner).unwrap();
        assert_eq!(db.get_runner(runner.id).unwrap().unwrap().name, "renamed");

        db.delete_runner(runner.id).unwrap();
        assert_eq!(db.get_runner(runner.id).unwrap(), None);
        assert!(matches!(
            db.delete_runner(runner.id),
            Err(StorageError::RunnerNotFound(_))
        ));
    }

    #[test]
    fn reference_counts_are_exact() {
        let db = Db::open_in_memory().unwrap();
        let runner = sample_runner("one", "/opt/proton");
        db.insert_runner(&runner).unwrap();

        let mut using_runner = sample_game("one");
        using_runner.runner_id = Some(runner.id);
        using_runner.prefix_path = Some(PathBuf::from("/shared/prefix"));
        let mut using_shared_prefix = sample_game("two");
        using_shared_prefix.runner_id = Some(Uuid::new_v4());
        using_shared_prefix.prefix_path = Some(PathBuf::from("/shared/prefix"));
        let mut using_default_prefix = sample_game("three");
        using_default_prefix.runner_id = None;
        using_default_prefix.prefix_path = None;

        for game in [&using_runner, &using_shared_prefix, &using_default_prefix] {
            db.insert_game(game).unwrap();
        }

        assert_eq!(db.count_games_using_runner(runner.id).unwrap(), 1);
        assert_eq!(
            db.count_games_using_prefix(Path::new("/shared/prefix"))
                .unwrap(),
            2
        );
        assert_eq!(
            db.count_games_using_prefix(Path::new("/unused")).unwrap(),
            0
        );
    }

    #[test]
    fn settings_upsert_and_list() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get_setting("global_env").unwrap(), None);

        db.set_setting("global_env", r#"{"A":"1"}"#).unwrap();
        db.set_setting("prefix_root", "/data/prefixes").unwrap();
        assert_eq!(
            db.get_setting("global_env").unwrap().as_deref(),
            Some(r#"{"A":"1"}"#)
        );

        db.set_setting("global_env", r#"{"A":"2"}"#).unwrap();
        let all = db.get_all_settings().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all["global_env"], r#"{"A":"2"}"#);
        assert_eq!(all["prefix_root"], "/data/prefixes");

        db.delete_setting("prefix_root").unwrap();
        assert_eq!(db.get_setting("prefix_root").unwrap(), None);
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

    #[test]
    fn corrupt_arch_is_reported() {
        let db = Db::open_in_memory().unwrap();
        let game = sample_game("corrupt");
        db.insert_game(&game).unwrap();
        db.conn
            .execute(
                "UPDATE games SET arch = 'win16' WHERE id = ?1",
                params![game.id.to_string()],
            )
            .unwrap();

        assert!(matches!(
            db.get_game(game.id),
            Err(StorageError::CorruptValue {
                column: "games.arch",
                ..
            })
        ));
    }

    #[test]
    fn corrupt_json_is_reported() {
        let db = Db::open_in_memory().unwrap();
        let game = sample_game("corrupt");
        db.insert_game(&game).unwrap();
        db.conn
            .execute(
                "UPDATE games SET display_json = 'not json' WHERE id = ?1",
                params![game.id.to_string()],
            )
            .unwrap();

        assert!(matches!(db.get_game(game.id), Err(StorageError::Json(_))));
    }

    #[test]
    fn non_utf8_paths_are_rejected() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let db = Db::open_in_memory().unwrap();
        let mut game = sample_game("bad path");
        game.executable_path = PathBuf::from(OsStr::from_bytes(b"/tmp/\xff.exe"));

        assert!(matches!(
            db.insert_game(&game),
            Err(StorageError::NonUtf8Path {
                column: "games.executable_path",
                ..
            })
        ));
    }

    #[test]
    fn corrupt_runner_kind_is_reported() {
        let db = Db::open_in_memory().unwrap();
        let runner = sample_runner("one", "/opt/proton");
        db.insert_runner(&runner).unwrap();
        db.conn
            .execute(
                "UPDATE runners SET kind = 'umu' WHERE id = ?1",
                params![runner.id.to_string()],
            )
            .unwrap();

        assert!(matches!(
            db.get_runner(runner.id),
            Err(StorageError::CorruptValue {
                column: "runners.kind",
                ..
            })
        ));
    }
}
