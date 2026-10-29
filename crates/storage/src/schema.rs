use rusqlite::{Connection, OptionalExtension};
use std::time::Duration;

use crate::StorageError;

pub(crate) const SCHEMA_VERSION: i64 = 3;

// Kept as the historical v1 schema so the migration test can build a v1 database.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const V1: &str = r#"
CREATE TABLE games (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    executable_path TEXT NOT NULL,
    working_dir TEXT,
    runner_id TEXT,
    prefix_path TEXT,
    arch TEXT NOT NULL,
    display_json TEXT NOT NULL,
    env_overrides_json TEXT NOT NULL,
    dll_overrides_json TEXT NOT NULL,
    args_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_played_at INTEGER
);

CREATE TABLE runners (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    binary_path TEXT NOT NULL UNIQUE,
    root_dir TEXT NOT NULL,
    arch TEXT NOT NULL,
    source_json TEXT NOT NULL,
    verified INTEGER NOT NULL,
    version_string TEXT
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX idx_games_runner_id ON games(runner_id);
CREATE INDEX idx_games_prefix_path ON games(prefix_path);
"#;

// Kept so the migration test can build a v2 database.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const V2: &str = r#"
CREATE TABLE games (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    executable_path TEXT NOT NULL,
    working_dir TEXT,
    runner_id TEXT,
    prefix_path TEXT,
    arch TEXT NOT NULL,
    display_json TEXT NOT NULL,
    env_overrides_json TEXT NOT NULL,
    dll_overrides_json TEXT NOT NULL,
    args_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_played_at INTEGER,
    cover_path TEXT
);

CREATE TABLE runners (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    binary_path TEXT NOT NULL UNIQUE,
    root_dir TEXT NOT NULL,
    arch TEXT NOT NULL,
    source_json TEXT NOT NULL,
    verified INTEGER NOT NULL,
    version_string TEXT
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX idx_games_runner_id ON games(runner_id);
CREATE INDEX idx_games_prefix_path ON games(prefix_path);
"#;

const V3: &str = r#"
CREATE TABLE games (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    executable_path TEXT NOT NULL,
    working_dir TEXT,
    runner_id TEXT,
    prefix_path TEXT,
    arch TEXT NOT NULL,
    display_json TEXT NOT NULL,
    env_overrides_json TEXT NOT NULL,
    dll_overrides_json TEXT NOT NULL,
    args_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_played_at INTEGER,
    cover_path TEXT,
    playtime_seconds INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE runners (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    binary_path TEXT NOT NULL UNIQUE,
    root_dir TEXT NOT NULL,
    arch TEXT NOT NULL,
    source_json TEXT NOT NULL,
    verified INTEGER NOT NULL,
    version_string TEXT
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX idx_games_runner_id ON games(runner_id);
CREATE INDEX idx_games_prefix_path ON games(prefix_path);
"#;

fn migrate_v1_to_v2(conn: &Connection) -> Result<(), StorageError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("ALTER TABLE games ADD COLUMN cover_path TEXT;")?;
    tx.execute("UPDATE schema_version SET version = 2", [])?;
    tx.commit()?;
    Ok(())
}

fn migrate_v2_to_v3(conn: &Connection) -> Result<(), StorageError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("ALTER TABLE games ADD COLUMN playtime_seconds INTEGER NOT NULL DEFAULT 0;")?;
    tx.execute("UPDATE schema_version SET version = 3", [])?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn initialize(conn: Connection) -> Result<Connection, StorageError> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let _: String = conn.query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))?;

    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")?;

    let current: Option<i64> = conn
        .query_row(
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    match current {
        None => {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(V3)?;
            tx.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
            tx.commit()?;
        }
        Some(1) => {
            migrate_v1_to_v2(&conn)?;
            migrate_v2_to_v3(&conn)?;
        }
        Some(2) => migrate_v2_to_v3(&conn)?,
        Some(version) if version == SCHEMA_VERSION => {}
        Some(version) if version < SCHEMA_VERSION => {
            return Err(StorageError::MigrationRequired {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        Some(version) => {
            return Err(StorageError::UnsupportedSchema {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
    }

    Ok(conn)
}
