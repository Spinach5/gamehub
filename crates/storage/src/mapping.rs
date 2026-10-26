use std::path::{Path, PathBuf};

use launcher_core::Arch;
use rusqlite::Row;
use uuid::Uuid;

use crate::StorageError;

pub(crate) fn path_to_string(path: &Path, column: &'static str) -> Result<String, StorageError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| StorageError::NonUtf8Path {
            column,
            path: path.to_path_buf(),
        })
}

pub(crate) fn optional_path_to_string(
    path: Option<&Path>,
    column: &'static str,
) -> Result<Option<String>, StorageError> {
    path.map(|path| path_to_string(path, column)).transpose()
}

pub(crate) fn parse_uuid(column: &'static str, value: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(value).map_err(|_| StorageError::CorruptValue {
        column,
        value: value.to_owned(),
    })
}

pub(crate) fn parse_arch(column: &'static str, value: &str) -> Result<Arch, StorageError> {
    Arch::parse(value).ok_or_else(|| StorageError::CorruptValue {
        column,
        value: value.to_owned(),
    })
}

pub(crate) fn path_from_string(value: String) -> PathBuf {
    PathBuf::from(value)
}

pub(crate) fn opt_path_from_string(value: Option<String>) -> Option<PathBuf> {
    value.map(PathBuf::from)
}

pub(crate) fn opt_uuid_from_string(
    column: &'static str,
    value: Option<String>,
) -> Result<Option<Uuid>, StorageError> {
    value.map(|value| parse_uuid(column, &value)).transpose()
}

pub(crate) fn row_to_game(row: &Row<'_>) -> Result<launcher_core::Game, StorageError> {
    use launcher_core::{DisplayConfig, DllOverride};

    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let executable_path: String = row.get(2)?;
    let working_dir: Option<String> = row.get(3)?;
    let runner_id: Option<String> = row.get(4)?;
    let prefix_path: Option<String> = row.get(5)?;
    let arch: String = row.get(6)?;
    let display_json: String = row.get(7)?;
    let env_overrides_json: String = row.get(8)?;
    let dll_overrides_json: String = row.get(9)?;
    let args_json: String = row.get(10)?;
    let created_at: i64 = row.get(11)?;
    let last_played_at: Option<i64> = row.get(12)?;

    Ok(launcher_core::Game {
        id: parse_uuid("games.id", &id)?,
        name,
        executable_path: path_from_string(executable_path),
        working_dir: opt_path_from_string(working_dir),
        runner_id: opt_uuid_from_string("games.runner_id", runner_id)?,
        prefix_path: opt_path_from_string(prefix_path),
        arch: parse_arch("games.arch", &arch)?,
        display: serde_json::from_str::<DisplayConfig>(&display_json)?,
        env_overrides: serde_json::from_str(&env_overrides_json)?,
        dll_overrides: serde_json::from_str::<Vec<DllOverride>>(&dll_overrides_json)?,
        args: serde_json::from_str(&args_json)?,
        created_at,
        last_played_at,
    })
}
