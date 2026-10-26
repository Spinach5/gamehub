use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema version {found} is newer than supported version {supported}")]
    UnsupportedSchema { found: i64, supported: i64 },
    #[error("schema version {found} requires a migration to {supported}")]
    MigrationRequired { found: i64, supported: i64 },
    #[error("non-UTF-8 path in {column}: {path:?}")]
    NonUtf8Path { column: &'static str, path: PathBuf },
    #[error("invalid value {value:?} in column {column}")]
    CorruptValue { column: &'static str, value: String },
    #[error("game not found: {0}")]
    GameNotFound(Uuid),
    #[error("game already exists: {0}")]
    GameAlreadyExists(Uuid),
    #[error("runner not found: {0}")]
    RunnerNotFound(Uuid),
    #[error("runner already exists: {0}")]
    RunnerAlreadyExists(Uuid),
    #[error("runner binary already registered: {path:?}")]
    DuplicateRunner { path: PathBuf },
}
