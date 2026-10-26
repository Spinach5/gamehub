use rusqlite::{Connection, OptionalExtension};
use std::time::Duration;

use crate::StorageError;

pub(crate) const SCHEMA_VERSION: i64 = 1;

const V1: &str = r#"
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
            tx.execute_batch(V1)?;
            tx.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [SCHEMA_VERSION],
            )?;
            tx.commit()?;
        }
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
