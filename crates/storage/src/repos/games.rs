use crate::mapping::{optional_path_to_string, path_to_string, row_to_game};
use crate::{Db, StorageError};
use launcher_core::Game;
use rusqlite::params;
use std::path::Path;
use uuid::Uuid;

const GAME_COLUMNS: &str = "id, name, executable_path, working_dir, runner_id, prefix_path, \
                            arch, display_json, env_overrides_json, dll_overrides_json, \
                            args_json, created_at, last_played_at, cover_path, playtime_seconds";

impl Db {
    pub fn insert_game(&self, game: &Game) -> Result<(), StorageError> {
        let id = game.id.to_string();
        let executable_path = path_to_string(&game.executable_path, "games.executable_path")?;
        let working_dir =
            optional_path_to_string(game.working_dir.as_deref(), "games.working_dir")?;
        let runner_id = game.runner_id.map(|id| id.to_string());
        let prefix_path =
            optional_path_to_string(game.prefix_path.as_deref(), "games.prefix_path")?;
        let cover_path = optional_path_to_string(game.cover_path.as_deref(), "games.cover_path")?;
        let display_json = serde_json::to_string(&game.display)?;
        let env_overrides_json = serde_json::to_string(&game.env_overrides)?;
        let dll_overrides_json = serde_json::to_string(&game.dll_overrides)?;
        let args_json = serde_json::to_string(&game.args)?;

        self.conn
            .execute(
                "INSERT INTO games (id, name, executable_path, working_dir, runner_id, prefix_path,
                                    arch, display_json, env_overrides_json, dll_overrides_json,
                                    args_json, created_at, last_played_at, cover_path,
                                    playtime_seconds)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
                    cover_path,
                    game.playtime_seconds,
                ],
            )
            .map_err(|error| map_game_insert_error(error, game.id))?;
        Ok(())
    }

    /// Writes the editable fields of `game`. `playtime_seconds` is deliberately
    /// left alone: it is accumulated by launch sessions, and a draft saved
    /// while a game runs would otherwise overwrite it with a stale value.
    pub fn update_game(&self, game: &Game) -> Result<(), StorageError> {
        let executable_path = path_to_string(&game.executable_path, "games.executable_path")?;
        let working_dir =
            optional_path_to_string(game.working_dir.as_deref(), "games.working_dir")?;
        let runner_id = game.runner_id.map(|id| id.to_string());
        let prefix_path =
            optional_path_to_string(game.prefix_path.as_deref(), "games.prefix_path")?;
        let cover_path = optional_path_to_string(game.cover_path.as_deref(), "games.cover_path")?;
        let display_json = serde_json::to_string(&game.display)?;
        let env_overrides_json = serde_json::to_string(&game.env_overrides)?;
        let dll_overrides_json = serde_json::to_string(&game.dll_overrides)?;
        let args_json = serde_json::to_string(&game.args)?;

        let changed = self.conn.execute(
            "UPDATE games SET name = ?2, executable_path = ?3, working_dir = ?4, runner_id = ?5,
                              prefix_path = ?6, arch = ?7, display_json = ?8,
                              env_overrides_json = ?9, dll_overrides_json = ?10, args_json = ?11,
                              created_at = ?12, last_played_at = ?13, cover_path = ?14
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
                cover_path,
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

    pub fn add_playtime(&self, id: Uuid, seconds: i64) -> Result<(), StorageError> {
        let changed = self.conn.execute(
            "UPDATE games SET playtime_seconds = MAX(playtime_seconds + ?2, 0) WHERE id = ?1",
            params![id.to_string(), seconds],
        )?;
        if changed == 0 {
            return Err(StorageError::GameNotFound(id));
        }
        Ok(())
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
