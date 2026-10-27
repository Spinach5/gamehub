use crate::mapping::{path_to_string, row_to_runner};
use crate::{Db, StorageError};
use launcher_core::Runner;
use rusqlite::params;
use uuid::Uuid;

const RUNNER_COLUMNS: &str =
    "id, kind, name, binary_path, root_dir, arch, source_json, verified, version_string";

impl Db {
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
