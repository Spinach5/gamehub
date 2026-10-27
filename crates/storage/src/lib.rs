mod error;
mod mapping;
mod repos;
mod schema;

use std::path::Path;

use rusqlite::Connection;

pub use error::StorageError;
pub use launcher_core::{AppPaths, Game, Runner};

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
    use rusqlite::params;
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
