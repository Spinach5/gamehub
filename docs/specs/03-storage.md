# P2 SQLite 存储 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `storage` crate：打开/迁移 SQLite、Game/Runner/Settings 的 CRUD、删除前引用计数，以及可诊断的数据损坏错误。

**Architecture:** 一个 `Db` 持有 `rusqlite::Connection`，所有方法同步执行；异步化由调用方用 `spawn_blocking` 完成。SQLite 编入二进制（`bundled`）。JSON 列只存 `DisplayConfig`/`env_overrides`/`dll_overrides`/`args`/`RunnerSource`。

**Tech Stack:** rusqlite 0.41（bundled）、serde_json、thiserror、launcher-core。

**前置条件：** P1 完成；`Game`/`Runner` 等类型已固定。

---

## Task S1: 打开数据库、迁移与 schema

> **当前实现提示（v0.11.0）：** 本节 Step 2 的 `SCHEMA_VERSION = 1` 与 `V1` 是 P2 历史快照。现在的 `schema.rs` 是 `SCHEMA_VERSION = 4`，新库直接建 `V4`，旧库按 `v1→v2→v3→v4` 依次迁移：

| 版本 | 变更 | 迁移语句 |
|---|---|---|
| v2 | 封面 | `ALTER TABLE games ADD COLUMN cover_path TEXT;` |
| v3 | 游玩时长 | `ALTER TABLE games ADD COLUMN playtime_seconds INTEGER NOT NULL DEFAULT 0;` |
| v4 | 平台 | `ALTER TABLE games ADD COLUMN platform TEXT NOT NULL DEFAULT 'windows';` |

- `V1`/`V2`/`V3` 保留为 `pub(crate)` 历史常量（仅迁移测试使用），迁移函数每次都在 `unchecked_transaction` 中执行并同步更新 `schema_version`。
- `initialize` 的版本分派：`None` 建 V4；`1` 连续迁移三次；`2` 迁移两次；`3` 迁移一次；`4` 不动；大于 4 报 `UnsupportedSchema`（不再出现 `MigrationRequired` 的正常路径）。
- `Game` 的列映射与 `GAME_COLUMNS` 已包含 `cover_path`、`playtime_seconds`、`platform`；`platform` 解析失败按 `CorruptValue` 处理。
- 迁移测试：`migrates_v1_database_to_v4`、`migrates_v2_database_to_v4`、`migrates_v3_database_to_v4`、`newer_schema_is_rejected`（版本 5）。

**Files:**
- Create: `crates/storage/src/error.rs`
- Create: `crates/storage/src/schema.rs`
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: 写错误类型**

`crates/storage/src/error.rs`：

```rust
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
```

- [ ] **Step 2: 写 schema 定义与初始化**

`crates/storage/src/schema.rs`：

```rust
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

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
    )?;

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
```

`unchecked_transaction` 用于 `&Connection`；此处 `conn` 是值，也可以用 `conn.transaction()`，但 `execute_batch` 与 `tx` 的可变借用更绕。统一用 `unchecked_transaction`。

- [ ] **Step 3: 写 `Db` 打开入口与测试**

`crates/storage/src/lib.rs`：

```rust
mod error;
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
    use rusqlite::params;
    use tempfile::tempdir;

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

        match Db::open(&path) {
            Err(StorageError::UnsupportedSchema { found: 2, supported: 1 }) => {}
            other => panic!("expected unsupported schema, got {other:?}"),
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
```

`other => panic!("{other:?}")` 需要 `Result<Db, StorageError>` 中 `Db: Debug`。为避免给 `Db` 派生 Debug，把测试改成：

```rust
        let result = Db::open(&path);
        match result {
            Err(StorageError::UnsupportedSchema { found: 2, supported: 1 }) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(_) => panic!("expected unsupported schema error"),
        }
```

- [ ] **Step 4: 运行并确认失败/通过**

Run: `cargo test -p storage`
Expected: 首次编译会因 `Db` 存在而通过；如果 schema 有 SQL 错误，会在此暴露。4 passed。

- [ ] **Step 5: 提交**

```bash
git add crates/storage
git commit -m "feat(storage): open sqlite database and apply v1 schema"
```

---

## Task S2: Game 持久化

**Files:**
- Create: `crates/storage/src/mapping.rs`
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: 写映射辅助**

`crates/storage/src/mapping.rs`：

```rust
use std::path::{Path, PathBuf};

use launcher_core::{Arch, RunnerKind, RunnerSource};
use rusqlite::Row;
use uuid::Uuid;

use crate::StorageError;

pub(crate) fn path_to_string(
    path: &Path,
    column: &'static str,
) -> Result<String, StorageError> {
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

pub(crate) fn parse_runner_kind(
    column: &'static str,
    value: &str,
) -> Result<RunnerKind, StorageError> {
    RunnerKind::parse(value).ok_or_else(|| StorageError::CorruptValue {
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
    value
        .map(|value| parse_uuid(column, &value))
        .transpose()
}

pub(crate) fn source_from_json(value: &str) -> Result<RunnerSource, StorageError> {
    Ok(serde_json::from_str(value)?)
}

pub(crate) fn row_to_game(row: &Row<'_>) -> Result<launcher_core::Game, StorageError> {
    use launcher_core::{DllOverride, DisplayConfig};

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

pub(crate) fn row_to_runner(row: &Row<'_>) -> Result<launcher_core::Runner, StorageError> {
    let id: String = row.get(0)?;
    let kind: String = row.get(1)?;
    let name: String = row.get(2)?;
    let binary_path: String = row.get(3)?;
    let root_dir: String = row.get(4)?;
    let arch: String = row.get(5)?;
    let source_json: String = row.get(6)?;
    let verified: i64 = row.get(7)?;
    let version_string: Option<String> = row.get(8)?;

    Ok(launcher_core::Runner {
        id: parse_uuid("runners.id", &id)?,
        kind: parse_runner_kind("runners.kind", &kind)?,
        name,
        binary_path: PathBuf::from(binary_path),
        root_dir: PathBuf::from(root_dir),
        arch: parse_arch("runners.arch", &arch)?,
        source: source_from_json(&source_json)?,
        verified: verified != 0,
        version_string,
    })
}
```

`row.get` 在列类型不符时返回 `rusqlite::Error`，通过 `?` 自动转为 `StorageError::Sqlite`。

- [ ] **Step 2: 在 `lib.rs` 加入 `mod mapping;` 与 Game API**

```rust
mod mapping;

use launcher_core::Game;
use mapping::{optional_path_to_string, path_to_string, row_to_game};
use rusqlite::params;
use uuid::Uuid;

const GAME_COLUMNS: &str = "id, name, executable_path, working_dir, runner_id, prefix_path, \
                            arch, display_json, env_overrides_json, dll_overrides_json, \
                            args_json, created_at, last_played_at";

impl Db {
    pub fn insert_game(&self, game: &Game) -> Result<(), StorageError> {
        let id = game.id.to_string();
        let executable_path = path_to_string(&game.executable_path, "games.executable_path")?;
        let working_dir = optional_path_to_string(game.working_dir.as_deref(), "games.working_dir")?;
        let runner_id = game.runner_id.map(|id| id.to_string());
        let prefix_path = optional_path_to_string(game.prefix_path.as_deref(), "games.prefix_path")?;
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
        let working_dir = optional_path_to_string(game.working_dir.as_deref(), "games.working_dir")?;
        let runner_id = game.runner_id.map(|id| id.to_string());
        let prefix_path = optional_path_to_string(game.prefix_path.as_deref(), "games.prefix_path")?;
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
```

`OptionalExtension` 在 S4 会用到，到那时再加入导入，避免当前阶段出现 unused import warning。

- [ ] **Step 3: 追加 Game 测试**

在 `lib.rs` 的 `tests` 模块中加入：

```rust
    use launcher_core::{
        Arch, DllOverride, DisplayConfig, GamescopeMode, OverrideMode,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
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
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p storage`
Expected: 全部通过。

- [ ] **Step 5: 提交**

```bash
git add crates/storage
git commit -m "feat(storage): persist games with round-trip tests"
```

---

## Task S3: Runner 持久化与引用计数

**Files:**
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: 追加 Runner API**

```rust
use launcher_core::Runner;
use mapping::row_to_runner;

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
```

- [ ] **Step 2: 追加 Runner 测试**

```rust
    use launcher_core::{RunnerKind, RunnerSource};

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
            db.count_games_using_prefix(Path::new("/shared/prefix")).unwrap(),
            2
        );
        assert_eq!(
            db.count_games_using_prefix(Path::new("/unused")).unwrap(),
            0
        );
    }
```

- [ ] **Step 3: 运行测试**

Run: `cargo test -p storage`
Expected: 全部通过。

- [ ] **Step 4: 提交**

```bash
git add crates/storage
git commit -m "feat(storage): persist runners and expose reference counts"
```

---

## Task S4: Settings

**Files:**
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: 追加 Settings API**

```rust
use std::collections::BTreeMap;
use rusqlite::OptionalExtension;

impl Db {
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
        let mut statement = self.conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
        let mut rows = statement.query([])?;
        let mut settings = BTreeMap::new();
        while let Some(row) = rows.next()? {
            settings.insert(row.get(0)?, row.get(1)?);
        }
        Ok(settings)
    }
}
```

- [ ] **Step 2: 追加测试**

```rust
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
```

- [ ] **Step 3: 运行测试**

Run: `cargo test -p storage`
Expected: 全部通过。

- [ ] **Step 4: 提交**

```bash
git add crates/storage
git commit -m "feat(storage): add settings key-value persistence"
```

---

## Task S5: 损坏数据与路径错误

**Files:**
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: 追加测试**

```rust
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
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p storage`
Expected: 全部通过。若 `Json` 变体未匹配，检查 `serde_json::Error` 是否经 `#[from]` 自动转换。

- [ ] **Step 3: 提交**

```bash
git add crates/storage
git commit -m "test(storage): cover corrupt rows and non-UTF-8 paths"
```

---

## Task S6: 收尾

**Files:**
- Modify: `crates/storage/src/lib.rs`（仅当需要修正导出）
- Modify: `CHANGELOG.md`

- [ ] **Step 1: 确认公开 API**

`lib.rs` 顶部应有：

```rust
mod error;
mod mapping;
mod schema;

pub use error::StorageError;
```

不要在公开 API 中泄漏 `Connection`。`Db::conn` 保持私有；测试在 crate 内部访问它。

- [ ] **Step 2: 全量检查**

Run:

```bash
cargo fmt --all
cargo clippy -p storage --all-targets -- -D warnings
cargo test -p storage
```

Expected: 全部通过。

- [ ] **Step 3: 更新 CHANGELOG**

```markdown
## P2 — 存储

- SQLite v1 schema、WAL、迁移版本检查。
- Game/Runner CRUD 与 settings KV。
- 删除前引用计数：`count_games_using_runner`、`count_games_using_prefix`。
- 损坏枚举行/JSON/非 UTF-8 路径的可诊断错误。

验证：`cargo test -p storage`，共 15+ 个测试通过。
```

- [ ] **Step 4: 提交**

```bash
git add crates/storage CHANGELOG.md
git commit -m "chore(storage): finish storage phase verification"
```

---

## P2 验收

- [x] `cargo test -p storage` 通过。
- [x] 首次打开自动建 v1；版本 2 报 `UnsupportedSchema`。
- [x] 每个 CRUD 有 round-trip 测试；Game 的所有字段都被断言。
- [x] 同一 `binary_path` 不能注册两次。
- [x] `count_games_using_runner` 与 `count_games_using_prefix` 返回准确计数。
- [x] 非 UTF-8 路径、损坏 enum、损坏 JSON 都有专门错误，不 panic。
- [x] `Db::open` 自动创建 DB 父目录；`ensure_app_dirs` 创建全部应用目录。

## 实施记录

- 2026-10-26 完成。SQLite 3.54.1（libsqlite3-sys 0.39.1 bundled），
  `cargo test -p storage` 18 passed。
- 偏差：`row_to_runner`/`parse_runner_kind` 延后到 S3 加入，避免 S2 出现
  dead_code 警告；测试模块不重复导入 `rusqlite::params`。公开 API 与 spec 一致。

## 当前实现（v0.11.0）

- 版本与迁移以本文 S1 的「当前实现提示」为准：v4，新库直接建 V4，旧库逐级迁移。P2 验收中「首次打开自动建 v1；版本 2 报 UnsupportedSchema」仅描述 P2 当时状态。
- `Game` 写入路径：
  - `insert_game` 写入全部列，含 `cover_path`/`playtime_seconds`/`platform`。
  - `update_game` 写可编辑字段（含 `platform`），刻意不写 `playtime_seconds`，避免保存草稿覆盖运行中的统计；时长只由 `add_playtime` 用 `MAX(playtime_seconds + ?2, 0)` 累加。
  - `last_played_at` 由启动事件在开始时写入；UI 合并草稿时对 `last_played_at`/`playtime_seconds` 取 `max`。
- 设置仍是 KV 表，值统一 TEXT；`SaveSettings` 以 `Option<String>` 表达删除键。
- 引用计数与删除保护不变：`count_games_using_runner`、`count_games_using_prefix`、`delete_game`（只删行，不删封面/prefix）。
