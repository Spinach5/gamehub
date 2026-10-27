# UI 现代化与游戏封面 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 gameHub 从单一深色默认组件界面改造成「浅色默认 + 深色可切换、侧边栏 + 卡片化」的现代桌面应用，并为每个游戏增加自定义封面。

**Architecture:** 在 `ui` 中新增集中式主题令牌（`theme.rs`）与样式函数（`styles.rs`），用 `Theme::custom` 承载浅/深两套 Palette；视图只消费 `widgets/` 中的组合组件。封面路径进入 `Game` 与 SQLite（schema v2），由 `application` 在保存时复制到应用数据目录并清理旧文件，`ui` 用 iced `image` 特性渲染并缓存 handle。

**Tech Stack:** Rust 2024, iced 0.13.1（features: tokio, advanced, image）、image 0.24、rusqlite、rfd、tokio、tempfile。

**约定：** 按用户偏好，本计划不自动 commit / tag / push。每个任务结束时保持工作区未提交，只做本地验证；发布动作等用户单独指示。

---

## 文件结构

新增：

- `crates/ui/src/theme.rs` — `ThemeMode`、`Tokens`、浅/深令牌表、`Theme::custom`、封面渐变、字体辅助。
- `crates/ui/src/styles.rs` — 按钮/容器/输入/下拉/开关/滚动条/分段控件样式函数。
- `crates/ui/src/widgets/layout.rs` — 页头、导航项、卡片、区块卡片、字段行、空状态、居中提示。
- `crates/ui/src/widgets/badges.rs` — 胶囊与 tone 映射。
- `crates/ui/src/widgets/controls.rs` — 主/次/危险/文字按钮、开关行、小按钮。
- `crates/ui/src/widgets/cover.rs` — 自定义封面与渐变占位。
- `crates/ui/src/widgets/modal.rs` — 居中删除确认模态框。
- `crates/application/tests/covers.rs` — 封面导入/替换/清空/删除测试。

修改：

- `Cargo.toml`、`crates/application/Cargo.toml`
- `crates/launcher-core/src/{types,paths}.rs`
- `crates/storage/src/{schema,mapping,repos/games,lib}.rs`
- `crates/application/src/{error,games,delete,settings,lib}.rs`
- `crates/application/tests/orchestration.rs`（`save_game` 调用签名）
- `crates/ui/src/{lib,app/mod,app/view,app/message,app/update,draft,dialogs}.rs`
- `crates/ui/src/widgets/mod.rs`
- `crates/ui/src/views/{library,game_detail,runners,settings,logs}.rs`
- `README.md`
- `crates/{launch,prefix}/src/*.rs`、`crates/launcher-core/tests/contracts.rs`、`crates/storage/src/lib.rs`、`crates/application/tests/orchestration.rs` 中所有 `Game { .. }` 构造点（补 `cover_path: None`）。

---

## Task 1: 依赖、封面目录与数据路径

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/application/Cargo.toml`
- Modify: `crates/launcher-core/src/paths.rs`
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: 启用 iced 图片特性并加入 image 依赖**

`Cargo.toml`（workspace）：

```toml
iced = { version = "0.13.1", features = ["tokio", "advanced", "image"] }
image = { version = "0.24", default-features = false, features = ["png", "jpeg", "webp", "bmp"] }
```

`crates/application/Cargo.toml` 的 `[dependencies]` 增加：

```toml
image.workspace = true
```

- [ ] **Step 2: AppPaths 增加 covers_dir**

`crates/launcher-core/src/paths.rs`：

```rust
pub struct AppPaths {
    pub data_root: PathBuf,
    pub db_path: PathBuf,
    pub logs_dir: PathBuf,
    pub covers_dir: PathBuf,
    pub default_prefix_root: PathBuf,
    pub steam_compat_client: PathBuf,
}
```

`for_root` 增加一行（放在 `data_root` 之前）：

```rust
covers_dir: data_root.join("covers"),
```

`derived_paths_are_stable` 测试补充：

```rust
assert_eq!(paths.covers_dir, PathBuf::from("/data/gamehub/covers"));
```

- [ ] **Step 3: ensure_app_dirs 创建 covers 目录**

`crates/storage/src/lib.rs`：

```rust
pub fn ensure_app_dirs(paths: &AppPaths) -> Result<(), StorageError> {
    for dir in [
        &paths.data_root,
        &paths.logs_dir,
        &paths.covers_dir,
        &paths.default_prefix_root,
        &paths.steam_compat_client,
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
```

`ensure_app_dirs_creates_all_paths` 测试补充：

```rust
assert!(paths.covers_dir.is_dir());
```

- [ ] **Step 4: 验证**

Run: `cargo test -p launcher-core -p storage`
Expected: PASS（此时 `Game` 尚无新字段，workspace 其余部分仍可编译）。

---

## Task 2: `Game.cover_path` 与全部构造点

**Files:**
- Modify: `crates/launcher-core/src/types.rs`（结构体、`sample_game`、JSON 字段测试）
- Modify: `crates/launcher-core/tests/contracts.rs`
- Modify: `crates/launch/src/plan.rs`
- Modify: `crates/launch/src/resolve.rs`
- Modify: `crates/launch/src/env.rs`
- Modify: `crates/prefix/src/status.rs`
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/src/mapping.rs`
- Modify: `crates/application/tests/orchestration.rs`
- Modify: `crates/ui/src/draft.rs`

- [ ] **Step 1: 添加字段**

`crates/launcher-core/src/types.rs` 的 `Game`，在 `prefix_path` 之后加：

```rust
pub cover_path: Option<PathBuf>,
```

`sample_game()` 增加：

```rust
cover_path: Some(PathBuf::from("/data/covers/abc.png")),
```

`game_json_uses_fixed_field_names` 的字段列表加入 `"cover_path"`。

新增测试：

```rust
#[test]
fn game_round_trips_with_cover_path() {
    let mut value = sample_game();
    value.cover_path = Some(PathBuf::from("/data/covers/cyberpunk.png"));
    let encoded = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<Game>(&encoded).unwrap(), value);
}
```

- [ ] **Step 2: 补齐所有 `Game { .. }` 构造点**

在每个构造里 `prefix_path` 后补一行 `cover_path: None,`（`mapping.rs` 除外，见 Task 3）：

- `crates/launcher-core/tests/contracts.rs`（样本构造）
- `crates/launch/src/plan.rs:202` 附近的测试构造
- `crates/launch/src/resolve.rs:49` 的 `game()`
- `crates/launch/src/env.rs:296` 的 `game()`
- `crates/prefix/src/status.rs:158,181,207` 的三处构造
- `crates/storage/src/lib.rs:61` 的 `sample_game`
- `crates/application/tests/orchestration.rs:48,107,447` 的三处构造
- `crates/ui/src/draft.rs:405` 的 `to_game()`（见 Task 8 改为写 `self.cover_path`，此处先补 `cover_path: None` 保持编译）

- [ ] **Step 3: 验证编译**

Run: `cargo check --workspace --all-targets`
Expected: PASS（storage 仍读旧列；`mapping.rs` 暂时补 `cover_path: None` 也可编译）。

说明：若 `crates/storage/src/mapping.rs` 先补 `cover_path: None` 可在 Task 3 替换为真实列读取。

- [ ] **Step 4: 验证 launcher-core**

Run: `cargo test -p launcher-core`
Expected: PASS，包含新增的 `game_round_trips_with_cover_path`。

---

## Task 3: 数据库 schema v2 与封面列

**Files:**
- Modify: `crates/storage/src/schema.rs`
- Modify: `crates/storage/src/mapping.rs`
- Modify: `crates/storage/src/repos/games.rs`
- Test: `crates/storage/src/lib.rs`（`mod tests`）

- [ ] **Step 1: 先写迁移与往返测试**

`crates/storage/src/lib.rs` 的 `mod tests`：

1. 把 `in_memory_database_has_v1_schema` 改名为 `in_memory_database_has_v2_schema`，断言 `version == 2`。
2. 把 `newer_schema_is_rejected` 里的 `version = 2` 改为 `version = 3`，期望 `found: 3, supported: 2`。
3. 新增：

```rust
#[test]
fn migrates_v1_database_to_v2() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("gamehub.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE schema_version (version INTEGER NOT NULL);")
            .unwrap();
        conn.execute_batch(crate::schema::V1).unwrap();
        conn.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO games (id, name, executable_path, working_dir, runner_id,
                                prefix_path, arch, display_json, env_overrides_json,
                                dll_overrides_json, args_json, created_at, last_played_at)
             VALUES ('11111111-1111-1111-1111-111111111111', 'Old', '/games/old.exe',
                     NULL, NULL, NULL, 'win64',
                     '{\"backend\":\"auto\",\"monitor\":null,\"resolution\":null,\"refresh_rate\":null,\"fullscreen\":true,\"gamescope\":\"auto\"}',
                     '{}', '[]', '[]', 1700000000, NULL)",
            [],
        )
        .unwrap();
    }

    let db = Db::open(&path).unwrap();
    let version: i64 = db
        .conn
        .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 2);

    let mut game = db
        .get_game(Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(game.name, "Old");
    assert_eq!(game.cover_path, None);

    game.cover_path = Some(PathBuf::from("/data/covers/old.png"));
    db.update_game(&game).unwrap();
    assert_eq!(
        db.get_game(game.id).unwrap().unwrap().cover_path,
        game.cover_path
    );
}
```

4. `game_round_trips_all_fields` 里给 `sample_game` 使用的封面路径断言：

```rust
assert_eq!(db.get_game(game.id).unwrap().unwrap().cover_path, game.cover_path);
```

5. 把 `crates/storage/src/lib.rs` 的 `sample_game` 中 `cover_path` 从 `None` 改为：

```rust
cover_path: Some(PathBuf::from("/data/covers/sample.png")),
```

这样往返测试会覆盖新列的写入与读取。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p storage migrates_v1_database_to_v2`
Expected: FAIL（`schema::V1` 未公开 / 版本仍为 1）。

- [ ] **Step 3: 实现 v2 与迁移**

`crates/storage/src/schema.rs`：

```rust
pub(crate) const SCHEMA_VERSION: i64 = 2;

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

const V2: &str = r#"
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

fn migrate_v1_to_v2(conn: &Connection) -> Result<(), StorageError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch("ALTER TABLE games ADD COLUMN cover_path TEXT;")?;
    tx.execute("UPDATE schema_version SET version = 2", [])?;
    tx.commit()?;
    Ok(())
}
```

`initialize` 的 `match current` 改为：

```rust
match current {
    None => {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(V2)?;
        tx.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [SCHEMA_VERSION],
        )?;
        tx.commit()?;
    }
    Some(1) => migrate_v1_to_v2(&conn)?,
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
```

- [ ] **Step 4: 仓储读写封面列**

`crates/storage/src/repos/games.rs`：

```rust
const GAME_COLUMNS: &str = "id, name, executable_path, working_dir, runner_id, prefix_path, \
                            arch, display_json, env_overrides_json, dll_overrides_json, \
                            args_json, created_at, last_played_at, cover_path";
```

`insert_game`：

```rust
let cover_path = optional_path_to_string(game.cover_path.as_deref(), "games.cover_path")?;

self.conn.execute(
    "INSERT INTO games (id, name, executable_path, working_dir, runner_id, prefix_path,
                        arch, display_json, env_overrides_json, dll_overrides_json,
                        args_json, created_at, last_played_at, cover_path)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    params![
        id, game.name, executable_path, working_dir, runner_id, prefix_path,
        game.arch.as_str(), display_json, env_overrides_json, dll_overrides_json,
        args_json, game.created_at, game.last_played_at, cover_path,
    ],
)
```

`update_game`：

```rust
let cover_path = optional_path_to_string(game.cover_path.as_deref(), "games.cover_path")?;

let changed = self.conn.execute(
    "UPDATE games SET name = ?2, executable_path = ?3, working_dir = ?4, runner_id = ?5,
                      prefix_path = ?6, arch = ?7, display_json = ?8,
                      env_overrides_json = ?9, dll_overrides_json = ?10, args_json = ?11,
                      created_at = ?12, last_played_at = ?13, cover_path = ?14
     WHERE id = ?1",
    params![
        game.id.to_string(), game.name, executable_path, working_dir, runner_id,
        prefix_path, game.arch.as_str(), display_json, env_overrides_json,
        dll_overrides_json, args_json, game.created_at, game.last_played_at, cover_path,
    ],
)?;
```

`crates/storage/src/mapping.rs` 的 `row_to_game` 末尾：

```rust
let cover_path: Option<String> = row.get(13)?;
```

并在 `Game { .. }` 中：

```rust
cover_path: opt_path_from_string(cover_path),
```

（此时把 Task 2 里临时写的 `cover_path: None` 替换为上面一行。）

- [ ] **Step 5: 验证**

Run: `cargo test -p storage`
Expected: PASS，包含迁移、往返、未来版本拒绝三个测试。

Run: `cargo check --workspace --all-targets`
Expected: PASS。

---

## Task 4: 应用层封面导入、清理与主题设置键

**Files:**
- Modify: `crates/application/src/error.rs`
- Modify: `crates/application/src/games.rs`
- Modify: `crates/application/src/delete.rs`
- Modify: `crates/application/src/settings.rs`
- Modify: `crates/application/src/lib.rs`
- Modify: `crates/application/tests/orchestration.rs`
- Create: `crates/application/tests/covers.rs`

- [ ] **Step 1: 先写封面测试**

创建 `crates/application/tests/covers.rs`：

```rust
use application::{AppError, AppService, DeleteTarget, Game};
use launcher_core::{Arch, DisplayConfig};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tempfile::tempdir;
use uuid::Uuid;

fn game(dir: &Path) -> Game {
    let exe = dir.join("game.exe");
    std::fs::write(&exe, b"MZ").unwrap();
    Game {
        id: Uuid::new_v4(),
        name: "Cover Test".into(),
        executable_path: exe,
        working_dir: None,
        runner_id: None,
        prefix_path: None,
        cover_path: None,
        arch: Arch::Win64,
        display: DisplayConfig::default(),
        env_overrides: BTreeMap::new(),
        dll_overrides: Vec::new(),
        args: Vec::new(),
        created_at: 1,
        last_played_at: None,
    }
}

fn write_image(path: &Path, width: u32, height: u32) {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 90, 255]));
    image.save(path).unwrap();
}

#[tokio::test]
async fn save_copies_cover_into_covers_dir() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    write_image(&source, 4, 6);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(source), covers.clone())
        .await
        .unwrap();

    let expected = covers.join(format!("{id}.png"));
    assert_eq!(saved.cover_path.as_deref(), Some(expected.as_path()));
    assert!(expected.is_file());
}

#[tokio::test]
async fn replacing_cover_removes_the_old_file() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let first = dir.path().join("first.png");
    let second = dir.path().join("second.bmp");
    write_image(&first, 4, 6);
    write_image(&second, 5, 7);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(first), covers.clone())
        .await
        .unwrap();
    let old = saved.cover_path.clone().unwrap();
    assert!(old.is_file());

    let saved = service
        .clone()
        .save_game(saved, false, Some(second), covers.clone())
        .await
        .unwrap();
    let new = covers.join(format!("{id}.bmp"));
    assert_eq!(saved.cover_path.as_deref(), Some(new.as_path()));
    assert!(new.is_file());
    assert!(!old.is_file());
}

#[tokio::test]
async fn clearing_cover_removes_the_file_and_column() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    write_image(&source, 4, 6);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let mut saved = service
        .clone()
        .save_game(value, true, Some(source), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();
    assert!(cover.is_file());

    saved.cover_path = None;
    let cleared = service
        .clone()
        .save_game(saved, false, None, covers.clone())
        .await
        .unwrap();
    assert_eq!(cleared.cover_path, None);
    assert!(!cover.is_file());
    assert_eq!(
        service.load_initial().await.unwrap().games[0].cover_path,
        None
    );
}

#[tokio::test]
async fn rejects_unsupported_cover_extension_without_writing() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.txt");
    std::fs::write(&source, b"not an image").unwrap();

    let service = AppService::open_in_memory().unwrap();
    let result = service
        .clone()
        .save_game(game(dir.path()), true, Some(source), covers.clone())
        .await;

    assert!(matches!(result, Err(AppError::CoverUnsupportedFormat(_))));
    assert!(service.load_initial().await.unwrap().games.is_empty());
    assert!(!covers.exists());
}

#[tokio::test]
async fn rejects_oversized_cover() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("wide.png");
    write_image(&source, 8193, 1);

    let service = AppService::open_in_memory().unwrap();
    let result = service
        .clone()
        .save_game(game(dir.path()), true, Some(source), covers.clone())
        .await;

    assert!(matches!(
        result,
        Err(AppError::CoverTooLarge {
            width: 8193,
            height: 1
        })
    ));
}

#[tokio::test]
async fn deleting_a_game_removes_its_cover() {
    let dir = tempdir().unwrap();
    let covers = dir.path().join("covers");
    let source = dir.path().join("cover.png");
    write_image(&source, 4, 6);

    let service = AppService::open_in_memory().unwrap();
    let value = game(dir.path());
    let id = value.id;
    let saved = service
        .clone()
        .save_game(value, true, Some(source), covers.clone())
        .await
        .unwrap();
    let cover = saved.cover_path.clone().unwrap();

    service
        .clone()
        .delete(
            DeleteTarget::Game(id),
            HashMap::new(),
            None,
            dir.path().join("logs"),
        )
        .await
        .unwrap();

    assert!(!cover.is_file());
    assert!(service.load_initial().await.unwrap().games.is_empty());
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p application --test covers`
Expected: 编译失败（`save_game` 参数数量不匹配、错误变体不存在）。

- [ ] **Step 3: 添加错误变体**

`crates/application/src/error.rs` 增加：

```rust
#[error("封面必须是 PNG / JPEG / WebP / BMP 图片: {0}")]
CoverUnsupportedFormat(PathBuf),
#[error("无法读取封面图片 {path}: {detail}")]
CoverInvalidImage { path: PathBuf, detail: String },
#[error("封面尺寸过大（{width}×{height}），单边不能超过 8192 像素")]
CoverTooLarge { width: u32, height: u32 },
#[error("复制封面失败: {0}")]
CoverCopy(#[source] std::io::Error),
```

- [ ] **Step 4: 实现导入与保存**

`crates/application/src/games.rs` 完整替换为：

```rust
use crate::{AppError, AppService};
use launcher_core::Game;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_COVER_EDGE: u32 = 8192;
const COVER_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];

fn import_cover(source: &Path, game_id: Uuid, covers_dir: &Path) -> Result<PathBuf, AppError> {
    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .ok_or_else(|| AppError::CoverUnsupportedFormat(source.to_path_buf()))?;
    if !COVER_EXTENSIONS.contains(&extension.as_str()) {
        return Err(AppError::CoverUnsupportedFormat(source.to_path_buf()));
    }

    let invalid = |detail: String| AppError::CoverInvalidImage {
        path: source.to_path_buf(),
        detail,
    };

    let reader = image::io::Reader::open(source)
        .map_err(|error| invalid(error.to_string()))?
        .with_guessed_format()
        .map_err(|error| invalid(error.to_string()))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| invalid(error.to_string()))?;
    if width == 0 || height == 0 {
        return Err(invalid("图片尺寸为 0".into()));
    }
    if width > MAX_COVER_EDGE || height > MAX_COVER_EDGE {
        return Err(AppError::CoverTooLarge { width, height });
    }

    std::fs::create_dir_all(covers_dir).map_err(AppError::CoverCopy)?;
    let target = covers_dir.join(format!("{game_id}.{extension}"));
    std::fs::copy(source, &target).map_err(AppError::CoverCopy)?;
    Ok(target)
}

impl AppService {
    pub async fn save_game(
        self,
        game: Game,
        is_new: bool,
        cover_source: Option<PathBuf>,
        covers_dir: PathBuf,
    ) -> Result<Game, AppError> {
        let db = self.db_handle();
        tokio::task::spawn_blocking(move || {
            if !game.executable_path.is_file() {
                return Err(AppError::ExecutableMissing(game.executable_path.clone()));
            }
            if let Some(working_dir) = &game.working_dir {
                if !working_dir.is_dir() {
                    return Err(AppError::WorkingDirMissing(working_dir.clone()));
                }
            }

            let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
            let previous_cover = if is_new {
                None
            } else {
                db.get_game(game.id)?.and_then(|stored| stored.cover_path)
            };

            let mut game = game;
            let mut imported_cover = None;
            if let Some(source) = cover_source {
                let target = import_cover(&source, game.id, &covers_dir)?;
                game.cover_path = Some(target.clone());
                imported_cover = Some(target);
            }

            let result = if is_new {
                db.insert_game(&game)
            } else {
                db.update_game(&game)
            };
            if let Err(error) = result {
                if let Some(path) = &imported_cover {
                    if let Err(remove_error) = std::fs::remove_file(path) {
                        tracing::warn!(
                            error = %remove_error,
                            path = %path.display(),
                            "failed to remove imported cover after save failure"
                        );
                    }
                }
                return Err(error.into());
            }

            if let Some(previous) = previous_cover {
                if game.cover_path.as_deref() != Some(previous.as_path()) {
                    if let Err(error) = std::fs::remove_file(&previous) {
                        tracing::warn!(
                            error = %error,
                            path = %previous.display(),
                            "failed to remove old cover"
                        );
                    }
                }
            }

            Ok(game)
        })
        .await?
    }
}
```

- [ ] **Step 5: 删除游戏时清理封面与日志**

`crates/application/src/delete.rs` 的 `AppService::delete` 增加 `logs_dir: PathBuf` 作为第 4 个参数，
`DeleteTarget::Game(id)` 分支替换为：

```rust
DeleteTarget::Game(id) => {
    if running.contains_key(&id) {
        return Err(AppError::GameRunning);
    }
    let log_path = logs_dir.join("games").join(format!("{id}.log"));
    let db = self.db_handle();
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
        let cover = db.get_game(id)?.and_then(|game| game.cover_path);
        db.delete_game(id)?;
        if let Some(path) = cover {
            if let Err(error) = std::fs::remove_file(&path) {
                tracing::warn!(
                    error = %error,
                    path = %path.display(),
                    "failed to remove game cover"
                );
            }
        }
        if let Err(error) = std::fs::remove_file(&log_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    error = %error,
                    path = %log_path.display(),
                    "failed to remove game log"
                );
            }
        }
        Ok(())
    })
    .await?
}
```

`log_path` 在 `spawn_blocking` 前计算并移入闭包；其余分支忽略 `logs_dir`。

- [ ] **Step 6: 主题设置键**

`crates/application/src/settings.rs` 增加：

```rust
pub const SETTING_UI_THEME: &str = "ui_theme";
```

`crates/application/src/lib.rs` 的 `pub use settings::{...}` 列表加入 `SETTING_UI_THEME`。

- [ ] **Step 7: 更新现有 save_game 调用**

`crates/application/tests/orchestration.rs` 中所有 `save_game` 调用末尾补 `, None, PathBuf::new()`，共 14 处：

- 第 148、156、168、183、269、305、343、344、375、406、488、502、529、590 行。

例如：

```rust
let saved = service
    .clone()
    .save_game(value.clone(), true, None, PathBuf::new())
    .await
    .unwrap();
```

- [ ] **Step 8: 验证**

Run: `cargo test -p application`
Expected: PASS，包含 `covers` 测试文件的 6 个用例。

Run: `cargo clippy -p application --all-targets -- -D warnings`
Expected: PASS。

---

## Task 5: UI 主题令牌与主题设置

**Files:**
- Create: `crates/ui/src/theme.rs`
- Modify: `crates/ui/src/lib.rs`
- Modify: `crates/ui/src/app/mod.rs`

- [ ] **Step 1: 写主题模块**

创建 `crates/ui/src/theme.rs`：

```rust
use iced::font::{Family, Weight};
use iced::theme::Palette;
use iced::{Color, Font, Theme};

pub const FONT_FAMILY: &str = "Noto Sans CJK SC";

pub fn font(weight: Weight) -> Font {
    Font {
        family: Family::Name(FONT_FAMILY),
        weight,
        ..Font::DEFAULT
    }
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
}

const fn rgba(hex: u32, alpha: f32) -> Color {
    let color = rgb(hex);
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: alpha,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

impl ThemeMode {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("dark") => Self::Dark,
            _ => Self::Light,
        }
    }

    pub const fn as_setting(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "浅色",
            Self::Dark => "深色",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Light => "gameHub Light",
            Self::Dark => "gameHub Dark",
        }
    }

    fn palette(self) -> Palette {
        match self {
            Self::Light => Palette {
                background: rgb(0xf4f5fb),
                text: rgb(0x171a2b),
                primary: rgb(0x5b5bd6),
                success: rgb(0x0f9f6e),
                danger: rgb(0xe11d48),
            },
            Self::Dark => Palette {
                background: rgb(0x10121a),
                text: rgb(0xe8eaf3),
                primary: rgb(0x6060e0),
                success: rgb(0x34d399),
                danger: rgb(0xfb7185),
            },
        }
    }

    pub fn theme(self) -> Theme {
        Theme::custom(self.name().to_owned(), self.palette())
    }
}

pub struct Tokens {
    pub canvas: Color,
    pub sidebar: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_faint: Color,
    pub primary: Color,
    pub primary_hover: Color,
    pub primary_soft: Color,
    pub primary_text: Color,
    pub on_primary: Color,
    pub success: Color,
    pub success_soft: Color,
    pub warning: Color,
    pub warning_soft: Color,
    pub danger: Color,
    pub danger_soft: Color,
    pub shadow: Color,
    pub backdrop: Color,
    pub log_surface: Color,
    pub log_text: Color,
}

pub static LIGHT: Tokens = Tokens {
    canvas: rgb(0xf4f5fb),
    sidebar: rgb(0xffffff),
    surface: rgb(0xffffff),
    surface_alt: rgb(0xf8f9fd),
    border: rgb(0xe6e8f2),
    border_strong: rgb(0xd9dcec),
    text: rgb(0x171a2b),
    text_muted: rgb(0x5f6678),
    text_faint: rgb(0x676e85),
    primary: rgb(0x5b5bd6),
    primary_hover: rgb(0x4f4fc4),
    primary_soft: rgb(0xecebff),
    primary_text: rgb(0x4f46c9),
    on_primary: rgb(0xffffff),
    success: rgb(0x0a7a55),
    success_soft: rgb(0xe8f9f1),
    warning: rgb(0xb45309),
    warning_soft: rgb(0xfef3c7),
    danger: rgb(0xd11a43),
    danger_soft: rgb(0xfff1f2),
    shadow: rgba(0x171a2b, 0.07),
    backdrop: rgba(0x171a2b, 0.45),
    log_surface: rgb(0x0f1117),
    log_text: rgb(0xd7dbe8),
};

pub static DARK: Tokens = Tokens {
    canvas: rgb(0x10121a),
    sidebar: rgb(0x151822),
    surface: rgb(0x191c28),
    surface_alt: rgb(0x1e2230),
    border: rgb(0x262b3a),
    border_strong: rgb(0x333a4d),
    text: rgb(0xe8eaf3),
    text_muted: rgb(0x9aa1b8),
    text_faint: rgb(0x848a9e),
    primary: rgb(0x6060e0),
    primary_hover: rgb(0x5a5ad8),
    primary_soft: rgba(0x6060e0, 0.18),
    primary_text: rgb(0xb9b9ff),
    on_primary: rgb(0xffffff),
    success: rgb(0x34d399),
    success_soft: rgba(0x34d399, 0.13),
    warning: rgb(0xfbbf24),
    warning_soft: rgba(0xfbbf24, 0.14),
    danger: rgb(0xfb7185),
    danger_soft: rgba(0xfb7185, 0.13),
    shadow: rgba(0x000000, 0.5),
    backdrop: rgba(0x000000, 0.6),
    log_surface: rgb(0x0b0d13),
    log_text: rgb(0xc9cedd),
};

pub fn tokens(theme: &Theme) -> &'static Tokens {
    if theme.extended_palette().is_dark {
        &DARK
    } else {
        &LIGHT
    }
}

const COVER_GRADIENTS: [(u32, u32); 6] = [
    (0x6366f1, 0xa855f7),
    (0xf59e0b, 0xf43f5e),
    (0x10b981, 0x0ea5e9),
    (0x8b5cf6, 0x1e3a8a),
    (0xf43f5e, 0x7c2d12),
    (0x0ea5e9, 0x155e75),
];

pub fn cover_gradient(seed: u128) -> iced::gradient::Linear {
    let (start, end) = COVER_GRADIENTS[(seed % COVER_GRADIENTS.len() as u128) as usize];
    iced::gradient::Linear::new(135.0_f32.to_radians())
        .add_stop(0.0, rgb(start))
        .add_stop(1.0, rgb(end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_parses_known_values_and_falls_back_to_light() {
        assert_eq!(ThemeMode::from_setting(None), ThemeMode::Light);
        assert_eq!(ThemeMode::from_setting(Some("dark")), ThemeMode::Dark);
        assert_eq!(ThemeMode::from_setting(Some("light")), ThemeMode::Light);
        assert_eq!(ThemeMode::from_setting(Some("whatever")), ThemeMode::Light);
    }

    #[test]
    fn theme_palette_brightness_matches_mode() {
        assert!(!ThemeMode::Light.theme().extended_palette().is_dark);
        assert!(ThemeMode::Dark.theme().extended_palette().is_dark);
    }

    #[test]
    fn tokens_select_the_matching_table() {
        assert!(std::ptr::eq(tokens(&ThemeMode::Light.theme()), &LIGHT));
        assert!(std::ptr::eq(tokens(&ThemeMode::Dark.theme()), &DARK));
    }
}
```

- [ ] **Step 2: 接入应用主题**

`crates/ui/src/lib.rs` 增加：

```rust
pub mod theme;
```

并把：

```rust
.theme(|_| iced::Theme::Dark)
```

改为：

```rust
.theme(App::theme)
```

- [ ] **Step 3: SettingsState 增加主题**

`crates/ui/src/app/mod.rs`：

```rust
use crate::theme::ThemeMode;
```

`SettingsState` 增加字段：

```rust
pub theme: ThemeMode,
```

`default_for` 增加：

```rust
theme: ThemeMode::Light,
```

`from_map` 的 `Ok(Self { .. })` 增加：

```rust
theme: ThemeMode::from_setting(map.get(SETTING_UI_THEME).map(String::as_str)),
```

`to_map` 增加：

```rust
map.insert(
    SETTING_UI_THEME.to_string(),
    Some(self.theme.as_setting().to_string()),
);
```

`pub use application::{...}` 列表加入 `SETTING_UI_THEME`。

在 `impl App` 中增加：

```rust
pub fn theme(&self) -> iced::Theme {
    self.settings.theme.theme()
}
```

- [ ] **Step 4: 更新设置测试**

`crates/ui/src/app/mod.rs` 的测试：

```rust
#[test]
fn settings_default_theme_is_light() {
    let settings = settings_from(&[]).unwrap();
    assert_eq!(settings.theme, ThemeMode::Light);
}

#[test]
fn settings_parse_theme_with_fallback() {
    assert_eq!(
        settings_from(&[(SETTING_UI_THEME, "dark")]).unwrap().theme,
        ThemeMode::Dark
    );
    assert_eq!(
        settings_from(&[(SETTING_UI_THEME, "nope")]).unwrap().theme,
        ThemeMode::Light
    );
}
```

`settings_round_trip_preserves_tuning_fields` 增加：

```rust
settings.theme = ThemeMode::Dark;
```

- [ ] **Step 5: 验证**

Run: `cargo test -p ui theme`
Expected: PASS（theme 模块与设置主题测试）。

Run: `cargo check -p ui`
Expected: PASS。

---

## Task 6: 样式函数库

**Files:**
- Create: `crates/ui/src/styles.rs`

- [ ] **Step 1: 实现样式函数**

创建 `crates/ui/src/styles.rs`：

```rust
use crate::theme::tokens;
use iced::overlay::menu;
use iced::widget::{button, container, pick_list, scrollable, text_input, toggler};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

const RADIUS_CARD: f32 = 14.0;
const RADIUS_CONTROL: f32 = 8.0;

fn border(color: Color, width: f32, radius: f32) -> Border {
    Border {
        color,
        width,
        radius: radius.into(),
    }
}

fn shadow(color: Color, y: f32, blur: f32) -> Shadow {
    Shadow {
        color,
        offset: Vector::new(0.0, y),
        blur_radius: blur,
    }
}

fn button_style(
    background: Option<Background>,
    text_color: Color,
    border: Border,
) -> button::Style {
    button::Style {
        background,
        text_color,
        border,
        shadow: Shadow::default(),
    }
}

pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Active => button_style(Some(t.primary.into()), t.on_primary, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL)),
        button::Status::Hovered | button::Status::Pressed => button_style(Some(t.primary_hover.into()), t.on_primary, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL)),
        button::Status::Disabled => button_style(Some(t.surface_alt.into()), t.text_faint, border(t.border, 1.0, RADIUS_CONTROL)),
    }
}

pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Active => button_style(Some(t.surface.into()), t.text, border(t.border_strong, 1.0, RADIUS_CONTROL)),
        button::Status::Hovered | button::Status::Pressed => button_style(Some(t.surface_alt.into()), t.text, border(t.border_strong, 1.0, RADIUS_CONTROL)),
        button::Status::Disabled => button_style(Some(t.surface_alt.into()), t.text_faint, border(t.border, 1.0, RADIUS_CONTROL)),
    }
}

pub fn danger(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Disabled => button_style(
            Some(t.surface_alt.into()),
            t.text_faint,
            border(t.border, 1.0, RADIUS_CONTROL),
        ),
        button::Status::Hovered | button::Status::Pressed => button_style(
            Some(t.danger_soft.into()),
            t.danger,
            border(t.danger, 1.0, RADIUS_CONTROL),
        ),
        button::Status::Active => button_style(
            Some(t.danger_soft.into()),
            t.danger,
            border(t.danger, 0.0, RADIUS_CONTROL),
        ),
    }
}

pub fn quiet(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Active => button_style(None, t.text_muted, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL)),
        button::Status::Hovered | button::Status::Pressed => button_style(Some(t.surface_alt.into()), t.text, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL)),
        button::Status::Disabled => button_style(None, t.text_faint, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL)),
    }
}

pub fn nav_item(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(Some(t.surface_alt.into()), t.text, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL)),
        _ => button_style(None, t.text_muted, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL)),
    }
}

pub fn nav_active(theme: &Theme, _status: button::Status) -> button::Style {
    let t = tokens(theme);
    button_style(Some(t.primary_soft.into()), t.primary_text, border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL))
}

pub fn segment(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(Some(t.surface.into()), t.text, border(Color::TRANSPARENT, 0.0, 7.0)),
        _ => button_style(None, t.text_muted, border(Color::TRANSPARENT, 0.0, 7.0)),
    }
}

pub fn segment_selected(theme: &Theme, _status: button::Status) -> button::Style {
    let t = tokens(theme);
    button::Style {
        background: Some(t.surface.into()),
        text_color: t.primary_text,
        border: border(Color::TRANSPARENT, 0.0, 7.0),
        shadow: shadow(t.shadow, 1.0, 4.0),
    }
}

pub fn segment_track(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.surface_alt.into()),
        border: border(Color::TRANSPARENT, 0.0, 9.0),
        ..Default::default()
    }
}

pub fn canvas(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(theme).canvas.into()),
        ..Default::default()
    }
}

pub fn sidebar(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.sidebar.into()),
        border: Border {
            color: t.border,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn card(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.surface.into()),
        border: border(t.border, 1.0, RADIUS_CARD),
        shadow: shadow(t.shadow, 2.0, 8.0),
        ..Default::default()
    }
}

pub fn modal_card(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.surface.into()),
        border: border(t.border, 1.0, 16.0),
        shadow: shadow(t.shadow, 12.0, 32.0),
        ..Default::default()
    }
}

pub fn backdrop(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(theme).backdrop.into()),
        ..Default::default()
    }
}

pub fn log_panel(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.log_surface.into()),
        border: border(t.border, 1.0, RADIUS_CARD),
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Live,
    Starting,
    Warning,
    Danger,
    Neutral,
    Info,
}

pub fn tone_colors(theme: &Theme, tone: Tone) -> (Color, Color) {
    let t = tokens(theme);
    match tone {
        Tone::Live => (t.success_soft, t.success),
        Tone::Starting => (t.warning_soft, t.warning),
        Tone::Warning => (t.warning_soft, t.warning),
        Tone::Danger => (t.danger_soft, t.danger),
        Tone::Neutral => (t.surface_alt, t.text_muted),
        Tone::Info => (t.primary_soft, t.primary_text),
    }
}

pub fn badge(theme: &Theme, tone: Tone) -> container::Style {
    let (background, _) = tone_colors(theme, tone);
    container::Style {
        background: Some(background.into()),
        border: Border {
            radius: 999.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn text_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let t = tokens(theme);
    let (background, border_color, width) = match status {
        text_input::Status::Active => (t.surface_alt, t.border, 1.0),
        text_input::Status::Hovered => (t.surface_alt, t.border_strong, 1.0),
        text_input::Status::Focused => (t.surface, t.primary, 1.5),
        text_input::Status::Disabled => (t.surface_alt, t.border, 1.0),
    };
    text_input::Style {
        background: background.into(),
        border: border(border_color, width, RADIUS_CONTROL),
        icon: t.text_muted,
        placeholder: t.text_faint,
        value: t.text,
        selection: t.primary_soft,
    }
}

pub fn pick_list(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let t = tokens(theme);
    let (background, border_color) = match status {
        pick_list::Status::Active => (t.surface_alt, t.border),
        pick_list::Status::Hovered => (t.surface_alt, t.border_strong),
        pick_list::Status::Opened => (t.surface, t.primary),
    };
    pick_list::Style {
        text_color: t.text,
        placeholder_color: t.text_faint,
        handle_color: t.text_muted,
        background: background.into(),
        border: border(border_color, 1.0, RADIUS_CONTROL),
    }
}

pub fn menu(theme: &Theme) -> menu::Style {
    let t = tokens(theme);
    menu::Style {
        background: t.surface.into(),
        border: border(t.border, 1.0, RADIUS_CONTROL),
        text_color: t.text,
        selected_text_color: t.primary_text,
        selected_background: t.primary_soft.into(),
    }
}

pub fn toggler(theme: &Theme, status: toggler::Status) -> toggler::Style {
    let t = tokens(theme);
    let is_toggled = match status {
        toggler::Status::Active { is_toggled }
        | toggler::Status::Hovered { is_toggled } => is_toggled,
        toggler::Status::Disabled => false,
    };
    toggler::Style {
        background: if is_toggled { t.primary } else { t.border_strong },
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Color::WHITE,
        foreground_border_width: 1.0,
        foreground_border_color: t.border_strong,
    }
}

pub fn scrollable(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    let t = tokens(theme);
    let rail = scrollable::Rail {
        background: Some(t.surface_alt.into()),
        border: Border::default(),
        scroller: scrollable::Scroller {
            color: t.border_strong,
            border: Border {
                radius: 999.0.into(),
                ..Default::default()
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}

pub fn log_scrollable(theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    let t = tokens(theme);
    let rail = scrollable::Rail {
        background: Some(t.log_surface.into()),
        border: Border::default(),
        scroller: scrollable::Scroller {
            color: t.log_text.scale_alpha(0.35),
            border: Border {
                radius: 999.0.into(),
                ..Default::default()
            },
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}
```

- [ ] **Step 2: 注册模块**

`crates/ui/src/lib.rs` 增加：

```rust
pub mod styles;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p ui`
Expected: PASS（尚无调用方，函数允许暂时未使用）。

---

## Task 7: 通用组件

**Files:**
- Create: `crates/ui/src/widgets/layout.rs`
- Create: `crates/ui/src/widgets/badges.rs`
- Create: `crates/ui/src/widgets/controls.rs`
- Create: `crates/ui/src/widgets/cover.rs`
- Create: `crates/ui/src/widgets/modal.rs`
- Modify: `crates/ui/src/widgets/mod.rs`
- Keep temporarily: `crates/ui/src/widgets/confirm.rs`（Task 9 删除）

- [ ] **Step 1: 布局组件**

创建 `crates/ui/src/widgets/layout.rs`：

```rust
use crate::app::Message;
use crate::styles;
use crate::theme::{self, font};
use iced::font::Weight;
use iced::widget::{button, container, text};
use iced::{Element, Length};

pub fn muted<'a>(value: impl Into<String>) -> iced::widget::Text<'a> {
    text(value.into()).style(|t| iced::widget::text::Style {
        color: Some(theme::tokens(t).text_muted),
    })
}

pub fn faint<'a>(value: impl Into<String>) -> iced::widget::Text<'a> {
    text(value.into()).style(|t| iced::widget::text::Style {
        color: Some(theme::tokens(t).text_faint),
    })
}

pub fn page_header<'a>(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    actions: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced::widget::row![
        iced::widget::column![
            text(title.into()).font(font(Weight::Bold)).size(26),
            muted(subtitle).size(12),
        ]
        .spacing(2),
        iced::widget::horizontal_space(),
        actions.into(),
    ]
    .width(Length::Fill)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn nav_item<'a>(label: &'a str, active: bool, message: Message) -> Element<'a, Message> {
    let style: fn(&iced::Theme, button::Status) -> button::Style = if active {
        styles::nav_active
    } else {
        styles::nav_item
    };
    let bar = container("")
        .width(Length::Fixed(3.0))
        .height(Length::Fixed(16.0))
        .style(move |t| container::Style {
            background: Some(
                if active {
                    theme::tokens(t).primary
                } else {
                    iced::Color::TRANSPARENT
                }
                .into(),
            ),
            border: iced::Border {
                radius: 2.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    button(
        iced::widget::row![
            bar,
            text(label)
                .size(13)
                .font(font(if active { Weight::Semibold } else { Weight::Normal })),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([9, 10])
    .width(Length::Fill)
    .style(style)
    .on_press(message)
    .into()
}

pub fn card<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(16)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

pub fn section_card<'a>(
    title: impl Into<String>,
    description: Option<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut header = iced::widget::column![
        text(title.into()).font(font(Weight::Bold)).size(16)
    ]
    .spacing(2);
    if let Some(description) = description {
        header = header.push(muted(description).size(12));
    }

    container(
        iced::widget::column![header, content.into()]
            .spacing(12)
            .width(Length::Fill),
    )
    .padding(18)
    .width(Length::Fill)
    .style(styles::card)
    .into()
}

pub fn field<'a>(
    label: impl Into<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced::widget::row![
        container(muted(label).size(12)).width(Length::Fixed(72.0)),
        content.into(),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn empty_state<'a>(
    title: impl Into<String>,
    hint: impl Into<String>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut content = iced::widget::column![
        text(title.into()).font(font(Weight::Bold)).size(18),
        muted(hint).size(12),
    ]
    .spacing(6)
    .align_x(iced::Alignment::Center);

    if let Some(action) = action {
        content = content.push(container(action).padding([8, 0]));
    }

    container(content)
        .width(Length::Fill)
        .padding([56, 24])
        .align_x(iced::Alignment::Center)
        .style(styles::card)
        .into()
}

pub fn centered_message(message: &str) -> Element<'_, Message> {
    container(muted(message).size(14))
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
}

pub fn fatal(message: &str) -> Element<'_, Message> {
    let card = container(
        iced::widget::column![
            text("gamehub 无法启动").font(font(Weight::Bold)).size(24),
            muted(message.to_owned())
                .size(13)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        ]
        .spacing(10)
        .width(Length::Fill),
    )
    .padding(28)
    .width(Length::Fill)
    .max_width(560)
    .style(styles::card);

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .style(styles::canvas)
        .into()
}
```

- [ ] **Step 2: 胶囊组件**

创建 `crates/ui/src/widgets/badges.rs`：

```rust
use crate::app::Message;
use crate::styles::{self, Tone};
use crate::theme;
use iced::font::Weight;
use iced::widget::{container, text};
use iced::Element;

pub fn badge<'a>(label: impl Into<String>, tone: Tone) -> Element<'a, Message> {
    container(
        text(label.into())
            .size(11)
            .font(theme::font(Weight::Bold))
            .style(move |theme| iced::widget::text::Style {
                color: Some(styles::tone_colors(theme, tone).1),
            }),
    )
    .padding([3, 9])
    .style(move |theme| styles::badge(theme, tone))
    .into()
}
```

- [ ] **Step 3: 按钮与开关**

创建 `crates/ui/src/widgets/controls.rs`：

```rust
use crate::app::Message;
use crate::styles;
use crate::theme;
use crate::widgets::layout::muted;
use iced::font::Weight;
use iced::widget::{button, text, toggler, Button};
use iced::{Element, Length};

fn label<'a>(value: &'a str, size: u16) -> iced::widget::Text<'a> {
    text(value).size(size).font(theme::font(Weight::Semibold))
}

pub fn primary<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 13)).padding([9, 14]).style(styles::primary)
}

pub fn secondary<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 13))
        .padding([9, 14])
        .style(styles::secondary)
}

pub fn danger<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 13)).padding([9, 14]).style(styles::danger)
}

pub fn quiet<'a>(value: &'a str) -> Button<'a, Message> {
    button(text(value).size(12))
        .padding([6, 8])
        .style(styles::quiet)
}

pub fn primary_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 11)).padding([5, 10]).style(styles::primary)
}

pub fn secondary_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 11))
        .padding([5, 10])
        .style(styles::secondary)
}

pub fn danger_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(label(value, 11)).padding([5, 10]).style(styles::danger)
}

pub fn quiet_small<'a>(value: &'a str) -> Button<'a, Message> {
    button(text(value).size(11))
        .padding([5, 8])
        .style(styles::quiet)
}

pub fn switch_row<'a>(
    title: impl Into<String>,
    description: impl Into<String>,
    value: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    iced::widget::row![
        iced::widget::column![
            text(title.into()).size(13).font(theme::font(Weight::Semibold)),
            muted(description).size(11),
        ]
        .spacing(2)
        .width(Length::Fill),
        toggler(value).on_toggle(on_toggle).style(styles::toggler),
    ]
    .spacing(16)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center)
    .into()
}
```

- [ ] **Step 4: 封面组件**

创建 `crates/ui/src/widgets/cover.rs`：

```rust
use crate::app::Message;
use crate::theme;
use iced::font::Weight;
use iced::widget::image;
use iced::{Border, Color, ContentFit, Element, Length};
use uuid::Uuid;

pub fn cover<'a>(
    name: &str,
    id: Uuid,
    handle: Option<image::Handle>,
    width: f32,
    height: f32,
) -> Element<'a, Message> {
    let initial = name
        .trim()
        .chars()
        .next()
        .map(|character| character.to_string())
        .unwrap_or_else(|| "?".to_owned());
    let font_size = (width.min(height) * 0.34).clamp(14.0, 42.0);
    let gradient = theme::cover_gradient(id.as_u128());

    let placeholder = iced::widget::container(
        iced::widget::text(initial)
            .size(font_size)
            .font(theme::font(Weight::Bold))
            .color(Color::WHITE),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .align_x(iced::Alignment::Center)
    .align_y(iced::Alignment::Center)
    .style(move |_| iced::widget::container::Style {
        background: Some(gradient.into()),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let mut layers = iced::widget::Stack::new().push(placeholder);
    if let Some(handle) = handle {
        layers = layers.push(
            iced::widget::image(handle)
                .width(Length::Fixed(width))
                .height(Length::Fixed(height))
                .content_fit(ContentFit::Cover),
        );
    }

    layers
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}
```

- [ ] **Step 5: 删除确认模态框**

创建 `crates/ui/src/widgets/modal.rs`：

```rust
use crate::app::Message;
use crate::styles;
use crate::theme::font;
use crate::widgets::{controls, layout::muted};
use iced::font::Weight;
use iced::widget::{column, container, horizontal_space, opaque, row, text};
use iced::{Element, Length};

pub fn confirm_delete<'a>(
    title: String,
    body: String,
    confirm: Message,
    cancel: Message,
) -> Element<'a, Message> {
    let card = container(
        column![
            text(title)
                .font(font(Weight::Bold))
                .size(18)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            muted(body)
                .size(12)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            row![
                horizontal_space(),
                controls::secondary("取消").on_press(cancel),
                controls::danger("确认删除").on_press(confirm),
            ]
            .spacing(8)
            .width(Length::Fill),
        ]
        .spacing(14)
        .width(Length::Fill),
    )
    .padding(22)
    .max_width(430)
    .style(styles::modal_card);

    opaque(
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(styles::backdrop),
    )
    .into()
}
```

- [ ] **Step 6: 重写 widgets/mod.rs**

`crates/ui/src/widgets/mod.rs` 完整替换为：

```rust
pub mod badges;
mod choices;
mod confirm;
pub mod controls;
pub mod cover;
pub mod layout;
pub mod modal;

pub use badges::badge;
pub use choices::*;
pub use confirm::confirm_bar;
pub use controls::*;
pub use cover::cover;
pub use layout::*;
pub use modal::confirm_delete;

use crate::app::{ErrorNotice, Message};
use crate::styles;
pub use crate::styles::Tone;
use crate::theme::font;
use chrono::{DateTime, Local};
use iced::font::Weight;
use iced::widget::{column, container, text};
use iced::{Border, Element, Length};
use std::path::Path;

/// temporary for old views; removed in Task 15
pub fn section<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    layout::section_card(title, None, content)
}

pub fn error_banner(error: &ErrorNotice) -> Element<'_, Message> {
    let mut content = column![iced::widget::row![
        badge("错误", Tone::Danger),
        text(error.title.clone())
            .size(14)
            .font(font(Weight::Semibold)),
        iced::widget::horizontal_space(),
        quiet_small("关闭").on_press(Message::DismissError),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center)]
    .spacing(6)
    .width(Length::Fill);

    if let Some(detail) = &error.detail {
        content = content.push(
            muted(detail.clone())
                .size(12)
                .width(Length::Fill)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        );
    }

    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(|theme| {
            let (_, danger) = styles::tone_colors(theme, Tone::Danger);
            container::Style {
                background: Some(crate::theme::tokens(theme).surface.into()),
                border: Border {
                    color: danger,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

pub fn path_text(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "未选择".into())
}

pub fn format_timestamp(timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0))
        .map(|date| {
            date.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "从未".into())
}
```

`crates/ui/src/widgets/confirm.rs` 暂时保留（旧根视图仍引用 `confirm_bar`），到 Task 9 重写根视图时再删除；`section` 桥接仅服务旧视图，Task 15 删除。

- [ ] **Step 7: 验证**

Run: `cargo check -p ui`
Expected: PASS（旧视图尚未使用新组件，但模块可编译）。

---

## Task 8: App 状态、消息与草稿封面

**Files:**
- Modify: `crates/ui/src/app/message.rs`
- Modify: `crates/ui/src/app/mod.rs`
- Modify: `crates/ui/src/app/update.rs`
- Modify: `crates/ui/src/draft.rs`
- Modify: `crates/ui/src/dialogs.rs`

- [ ] **Step 1: 消息定义**

`crates/ui/src/app/message.rs`：

```rust
use crate::theme::ThemeMode;
```

`Message` 增加：

```rust
WindowResized(f32),
ThemeSaved {
    mode: crate::theme::ThemeMode,
    result: Result<(), String>,
},
```

`SettingsAction` 增加：

```rust
Theme(ThemeMode),
```

- [ ] **Step 2: 草稿字段与动作**

`crates/ui/src/draft.rs`：

`GameDraft` 在 `prefix_path` 后增加：

```rust
pub cover_path: Option<PathBuf>,
pub cover_source: Option<PathBuf>,
```

`DraftAction` 增加：

```rust
CoverSource(PathBuf),
ClearCover,
```

`GameDraft::new` 增加：

```rust
cover_path: None,
cover_source: None,
```

`GameDraft::from_game` 增加：

```rust
cover_path: game.cover_path.clone(),
cover_source: None,
```

`GameDraft::to_game` 的 `Game { .. }` 中把 `cover_path` 写为：

```rust
cover_path: self.cover_path.clone(),
```

`GameDraft::apply` 增加：

```rust
DraftAction::CoverSource(path) => self.cover_source = Some(path),
DraftAction::ClearCover => {
    self.cover_source = None;
    self.cover_path = None;
}
```

测试 `draft()` 辅助函数补：

```rust
cover_path: None,
cover_source: None,
```

`from_game_round_trips` 在构造 `game` 后加：

```rust
game.cover_path = Some(PathBuf::from("/data/covers/game.png"));
```

新增测试：

```rust
#[test]
fn cover_actions_set_and_clear() {
    let mut draft = draft();
    draft.apply(DraftAction::CoverSource(PathBuf::from("/tmp/a.png")));
    assert_eq!(draft.cover_source, Some(PathBuf::from("/tmp/a.png")));

    draft.cover_path = Some(PathBuf::from("/data/covers/a.png"));
    draft.apply(DraftAction::ClearCover);
    assert_eq!(draft.cover_source, None);
    assert_eq!(draft.cover_path, None);
}
```

- [ ] **Step 3: 文件对话框**

`crates/ui/src/dialogs.rs`：

`DialogIntent` 增加 `PickCover`。

`dialog_impl` 增加：

```rust
DialogIntent::PickCover => dialog
    .add_filter("封面图片", &["png", "jpg", "jpeg", "webp", "bmp"])
    .pick_file(),
```

（从现有 `pick_folder` 的组合分支中移除 `PickCover`，它不属于文件夹。）

- [ ] **Step 4: App 字段与封面 handle**

`crates/ui/src/app/mod.rs`：

```rust
use crate::theme::ThemeMode;
```

`App` 增加：

```rust
pub theme_saving: bool,
pub cover_handles: HashMap<Uuid, iced::widget::image::Handle>,
pub window_width: f32,
```

`App::new` 初始化：

```rust
theme_saving: false,
cover_handles: HashMap::new(),
window_width: 1180.0,
```

增加辅助方法：

```rust
fn cover_handle(path: Option<&Path>) -> Option<iced::widget::image::Handle> {
    path.filter(|path| path.is_file())
        .map(|path| iced::widget::image::Handle::from_path(path.to_path_buf()))
}

fn cover_handle_bytes(path: Option<&Path>) -> Option<iced::widget::image::Handle> {
    let path = path.filter(|path| path.is_file())?;
    let bytes = std::fs::read(path).ok()?;
    Some(iced::widget::image::Handle::from_bytes(bytes))
}
```

`apply_loaded` 末尾增加（`data.games` 已被移入 `self.games`，因此遍历 `self.games`）：

```rust
self.cover_handles = self
    .games
    .iter()
    .filter_map(|game| Some((game.id, Self::cover_handle(game.cover_path.as_deref())?)))
    .collect();
```

`apply_draft_saved` 的 `Ok(game)` 分支增加（在写入 `self.games` 之后）：

```rust
match Self::cover_handle_bytes(game.cover_path.as_deref()) {
    Some(handle) => {
        self.cover_handles.insert(game.id, handle);
    }
    None => {
        self.cover_handles.remove(&game.id);
    }
}
```

`save_draft` 开头增加防重复提交：

```rust
if self.pending.saving {
    return iced::Task::none();
}
```

`save_draft` 中：

```rust
let cover_source = draft.cover_source.clone();
let covers_dir = self.paths.covers_dir.clone();
iced::Task::perform(
    service.save_game(game, draft.is_new, cover_source, covers_dir),
    |result| Message::DraftSaved(result.map_err(|error| error.to_string())),
)
```

`apply_dialog_result` 增加：

```rust
DialogIntent::PickCover => {
    if let (Some(draft), Some(path)) = (&mut self.draft, path) {
        draft.apply(crate::draft::DraftAction::CoverSource(path));
    }
}
```

- [ ] **Step 5: 更新处理器**

`crates/ui/src/app/update.rs`：

顶部增加：

```rust
use std::collections::BTreeMap;
```

在 `Message::Settings(action)` 的 `match action` 中增加：

```rust
SettingsAction::Theme(mode) => {
    self.settings.theme = mode;
    if self.theme_saving {
        return Task::none();
    }
    let Some(service) = self.service.clone() else {
        self.error = Some(ErrorNotice::new("数据库不可用"));
        return Task::none();
    };
    self.theme_saving = true;
    return save_theme(service, mode);
}
```

`match message` 增加：

```rust
Message::WindowResized(width) => {
    self.window_width = width;
    Task::none()
}
Message::ThemeSaved { mode, result } => {
    self.theme_saving = false;
    match result {
        Ok(()) => {
            if self
                .error
                .as_ref()
                .is_some_and(|notice| notice.title == "保存主题失败")
            {
                self.error = None;
            }
            if self.settings.theme != mode {
                let Some(service) = self.service.clone() else {
                    return Task::none();
                };
                self.theme_saving = true;
                return save_theme(service, self.settings.theme);
            }
        }
        Err(error) => {
            self.error = Some(ErrorNotice::new("保存主题失败").with_detail(error));
        }
    }
    Task::none()
}
```

`update.rs` 底部增加自由函数：

```rust
fn save_theme(service: application::AppService, mode: crate::theme::ThemeMode) -> Task<Message> {
    let values = BTreeMap::from([(
        application::SETTING_UI_THEME.to_string(),
        Some(mode.as_setting().to_string()),
    )]);
    Task::perform(service.save_settings(values), move |result| {
        Message::ThemeSaved {
            mode,
            result: result.map_err(|error| error.to_string()),
        }
    })
}
```

`Message::SaveSettings` 中把 `values` 绑定改为可变，并移除主题键，避免底部保存覆盖即时主题：

```rust
let mut values = match self.settings.to_map() {
    Ok(values) => values,
    Err(error) => {
        self.error = Some(ErrorNotice::new("设置校验失败").with_detail(error));
        return Task::none();
    }
};
values.remove(application::SETTING_UI_THEME);
```

`Message::DeleteFinished` 的 `DeleteTarget::Game(id)` 分支增加：

```rust
self.cover_handles.remove(&id);
```

`Message::DeleteConfirmed` 的 `service.delete(...)` 调用补第 4 个参数 `logs_dir`，删除游戏时一并清理日志：

```rust
Task::perform(
    service.delete(
        target.clone(),
        running,
        default_runner_id,
        self.paths.logs_dir.clone(),
    ),
    move |result| Message::DeleteFinished {
        target: target.clone(),
        result: result.map_err(|error| error.to_string()),
    },
)
```

- [ ] **Step 6: 订阅窗口尺寸**

`crates/ui/src/app/mod.rs` 的 `subscription` 替换为：

```rust
pub fn subscription(&self) -> iced::Subscription<Message> {
    let resized = iced::window::resize_events()
        .map(|(_id, size)| Message::WindowResized(size.width));

    if self.screen == Screen::Logs {
        iced::Subscription::batch([
            resized,
            iced::time::every(std::time::Duration::from_secs(1))
                .map(|_| Message::RefreshLogs),
        ])
    } else {
        resized
    }
}
```

- [ ] **Step 7: 验证**

Run: `cargo test -p ui`
Expected: PASS（`confirm_bar` 临时保留，旧视图仍可编译）。

Run: `cargo check -p ui`
Expected: PASS（旧视图仍使用临时的 `section` 与 `confirm_bar`，Task 9 再移除确认条）。

---

## Task 9: 根视图、侧边栏与删除模态框

**Files:**
- Modify: `crates/ui/src/app/view.rs`
- Modify: `crates/ui/src/widgets/mod.rs`
- Delete: `crates/ui/src/widgets/confirm.rs`

- [ ] **Step 1: 重写根视图**

`crates/ui/src/app/view.rs` 完整替换为：

```rust
use super::{App, DeleteTarget, Message, Screen};
use crate::theme::font;
use crate::widgets::{layout, modal};
use iced::font::Weight;
use iced::widget::{column, container, row, stack, text};
use iced::{Element, Length};

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        if let Some(fatal) = &self.fatal {
            return layout::fatal(fatal);
        }

        let screen: Element<'_, Message> = match self.screen {
            Screen::Library => crate::views::library::view(self),
            Screen::GameDetail => crate::views::game_detail::view(self),
            Screen::RunnerManager => crate::views::runners::view(self),
            Screen::Settings => crate::views::settings::view(self),
            Screen::Logs => crate::views::logs::view(self),
        };

        let mut main = column![].spacing(16).width(Length::Fill).height(Length::Fill);
        if let Some(error) = &self.error {
            main = main.push(crate::widgets::error_banner(error));
        }
        main = main.push(screen);

        let base = container(
            row![
                sidebar(self),
                container(main)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(iced::Padding {
                        top: 20.0,
                        right: 24.0,
                        bottom: 20.0,
                        left: 24.0,
                    }),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::canvas);

        let mut layers = stack![base];
        if let Some(target) = &self.confirming {
            layers = layers.push(modal::confirm_delete(
                delete_title(self, target),
                delete_body(target).to_owned(),
                Message::DeleteConfirmed,
                Message::CancelDelete,
            ));
        }
        layers.into()
    }
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let active = match app.screen {
        Screen::RunnerManager => Screen::RunnerManager,
        Screen::Settings => Screen::Settings,
        _ => Screen::Library,
    };

    let mark = container("")
        .width(Length::Fixed(22.0))
        .height(Length::Fixed(22.0))
        .style(|_| iced::widget::container::Style {
            background: Some(crate::theme::cover_gradient(0).into()),
            border: iced::Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let footer = layout::faint(format!(
        "{} · {}",
        session_label(app.session),
        default_runner_label(app)
    ))
    .size(10);

    container(
        column![
            row![
                mark,
                text("gameHub").font(font(Weight::Bold)).size(15)
            ]
            .spacing(9)
            .align_y(iced::Alignment::Center),
            iced::widget::vertical_space().height(Length::Fixed(10.0)),
            layout::nav_item(
                "游戏库",
                active == Screen::Library,
                Message::Navigate(Screen::Library)
            ),
            layout::nav_item(
                "Runner 管理",
                active == Screen::RunnerManager,
                Message::Navigate(Screen::RunnerManager)
            ),
            layout::nav_item(
                "设置",
                active == Screen::Settings,
                Message::Navigate(Screen::Settings)
            ),
            iced::widget::vertical_space(),
            footer,
        ]
        .spacing(3)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([16, 12]),
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fill)
    .style(crate::styles::sidebar)
    .into()
}

fn session_label(session: application::SessionType) -> &'static str {
    match session {
        application::SessionType::Wayland => "Wayland",
        application::SessionType::X11 => "X11",
        application::SessionType::Unknown => "未知会话",
    }
}

fn default_runner_label(app: &App) -> &str {
    app.settings
        .default_runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.as_str())
        .unwrap_or("未设置默认 Runner")
}

fn delete_title(app: &App, target: &DeleteTarget) -> String {
    match target {
        DeleteTarget::Game(id) => app
            .games
            .iter()
            .find(|game| game.id == *id)
            .map(|game| format!("删除游戏「{}」？", game.name))
            .unwrap_or_else(|| "删除游戏？".to_owned()),
        DeleteTarget::Runner(id) => app
            .runners
            .iter()
            .find(|runner| runner.id == *id)
            .map(|runner| format!("删除 Runner「{}」？", runner.name))
            .unwrap_or_else(|| "删除 Runner？".to_owned()),
        DeleteTarget::Prefix { path, .. } => format!("删除 Prefix {}？", path.display()),
    }
}

fn delete_body(target: &DeleteTarget) -> &'static str {
    match target {
        DeleteTarget::Game(_) => "将同时删除该游戏的配置、日志与封面，此操作不可撤销。",
        DeleteTarget::Runner(_) => "删除后需要重新导入才能使用，此操作不可撤销。",
        DeleteTarget::Prefix { .. } => "Prefix 内的游戏存档与运行环境会被删除，此操作不可撤销。",
    }
}
```

- [ ] **Step 2: 验证**

Run: `cargo check -p ui`
Expected: PASS（旧视图仍可编译；新布局开始生效）。

Run: `cargo run -p ui`
Expected: 出现左侧导航 + 右侧内容；浅色主题；点击导航可切换页面（内容仍为旧样式）。

- [ ] **Step 3: 删除旧 confirm_bar**

新根视图已改用 `modal::confirm_delete`，不再引用 `confirm_bar`，此时删除临时组件：

Run: `rm crates/ui/src/widgets/confirm.rs`

在 `crates/ui/src/widgets/mod.rs` 中移除 `mod confirm;` 与 `pub use confirm::confirm_bar;`（`section` 桥接仍服务旧视图，保留到 Task 15）。

Run: `cargo check -p ui`
Expected: PASS。

---

## Task 10: 游戏库（主卡 + 封面网格）

**Files:**
- Modify: `crates/ui/src/views/library.rs`

- [ ] **Step 1: 重写游戏库**

`crates/ui/src/views/library.rs` 完整替换为：

```rust
use crate::app::{App, Message, Screen};
use crate::theme::{font, tokens};
use crate::widgets::{badges, controls, cover, layout, Tone};
use application::Game;
use iced::font::Weight;
use iced::widget::{column, container, row, scrollable, stack, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    if app.pending.loading {
        return layout::centered_message("正在加载…");
    }

    let running_count = app.running.len();
    let header = layout::page_header(
        "游戏库",
        format!(
            "{} 个游戏 · {} 个正在运行",
            app.games.len(),
            running_count
        ),
        row![
            controls::secondary("Runner 管理").on_press(Message::Navigate(Screen::RunnerManager)),
            controls::primary("＋ 新建游戏").on_press(Message::NewGame),
        ]
        .spacing(8)
        .into(),
    );

    let mut content = column![header].spacing(20).width(Length::Fill);

    if let Some(game) = app
        .games
        .iter()
        .find(|game| app.launch_state_label(game.id).is_some())
    {
        content = content.push(hero(app, game));
    }

    if app.games.is_empty() {
        content = content.push(layout::empty_state(
            "还没有游戏",
            "点击下方按钮选择一个 .exe 开始",
            Some(
                controls::primary("＋ 新建游戏")
                    .on_press(Message::NewGame)
                    .into(),
            ),
        ));
    } else {
        content = content.push(
            row![
                text("全部游戏").font(font(Weight::Bold)).size(16),
                text(app.games.len().to_string())
                    .size(12)
                    .style(|theme| iced::widget::text::Style {
                        color: Some(tokens(theme).text_faint),
                    }),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        );
        content = content.push(grid(app));
    }

    scrollable(content)
        .style(crate::styles::scrollable)
        .height(Length::Fill)
        .into()
}

fn hero<'a>(app: &'a App, game: &'a Game) -> Element<'a, Message> {
    let handle = app.cover_handles.get(&game.id).cloned();
    let state = app.launch_state_label(game.id).unwrap_or_default();
    let tone = if app.starting.contains(&game.id) {
        Tone::Starting
    } else {
        Tone::Live
    };

    container(
        row![
            cover(game.name.as_str(), game.id, handle, 56.0, 76.0),
            column![
                badges::badge(state, tone),
                text(game.name.clone()).font(font(Weight::Bold)).size(18),
                layout::muted(format!(
                    "{} · {}",
                    runner_label(app, game),
                    crate::widgets::format_timestamp(game.last_played_at)
                ))
                .size(11),
            ]
            .spacing(4),
            iced::widget::horizontal_space(),
            row![
                launch_or_stop(app, game, false),
                controls::secondary("日志").on_press(Message::OpenLogs(game.id)),
            ]
            .spacing(8),
        ]
        .spacing(14)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding(16)
    .width(Length::Fill)
    .style(crate::styles::card)
    .into()
}

fn grid(app: &App) -> Element<'_, Message> {
    let content_width = (app.window_width - 200.0 - 48.0).max(360.0);
    let columns = if content_width >= 860.0 { 4 } else { 3 };
    let card_width =
        ((content_width - 16.0 * (columns as f32 - 1.0)) / columns as f32).min(240.0);
    let cover_width = card_width - 20.0;
    let cover_height = cover_width * 4.0 / 3.0;

    let mut rows = column![].spacing(16);
    for chunk in app.games.chunks(columns) {
        let mut line = row![].spacing(16).align_y(iced::Alignment::Start);
        for game in chunk {
            line = line.push(game_card(app, game, card_width, cover_width, cover_height));
        }
        for _ in chunk.len()..columns {
            line = line.push(container("").width(Length::Fixed(card_width)));
        }
        rows = rows.push(line);
    }
    rows.into()
}

fn game_card<'a>(
    app: &'a App,
    game: &'a Game,
    card_width: f32,
    cover_width: f32,
    cover_height: f32,
) -> Element<'a, Message> {
    let handle = app.cover_handles.get(&game.id).cloned();
    let mut cover_stack = stack![cover(
        game.name.as_str(),
        game.id,
        handle,
        cover_width,
        cover_height
    )];

    if let Some(label) = app.launch_state_label(game.id) {
        let tone = if app.starting.contains(&game.id) {
            Tone::Starting
        } else {
            Tone::Live
        };
        cover_stack = cover_stack.push(container(badges::badge(label, tone)).padding(8));
    }

    container(
        column![
            cover_stack,
            text(game.name.clone())
                .font(font(Weight::Semibold))
                .size(14)
                .width(Length::Fill)
                .height(Length::Fixed(38.0))
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
            layout::faint(format!(
                "{} · {}",
                crate::widgets::format_timestamp(game.last_played_at),
                runner_label(app, game)
            ))
            .size(10),
            row![
                launch_or_stop(app, game, true),
                controls::secondary_small("设置").on_press(Message::SelectGame(game.id)),
                controls::quiet_small("日志").on_press(Message::OpenLogs(game.id)),
            ]
            .spacing(6),
        ]
        .spacing(8),
    )
    .padding(10)
    .width(Length::Fixed(card_width))
    .style(crate::styles::card)
    .into()
}

fn launch_or_stop<'a>(app: &'a App, game: &'a Game, small: bool) -> Element<'a, Message> {
    if app.starting.contains(&game.id) {
        return if small {
            controls::secondary_small("启动中").into()
        } else {
            controls::secondary("启动中").into()
        };
    }

    if app.running.contains_key(&game.id) {
        let stopping = app.stopping.contains(&game.id);
        let label = if stopping { "结束中" } else { "结束" };
        let press = (!stopping).then_some(Message::Stop(game.id));
        return if small {
            controls::danger_small(label).on_press_maybe(press).into()
        } else {
            controls::danger(label).on_press_maybe(press).into()
        };
    }

    let press = Some(Message::Launch(game.id));
    if small {
        controls::primary_small("启动").on_press_maybe(press).into()
    } else {
        controls::primary("启动").on_press_maybe(press).into()
    }
}

fn runner_label<'a>(app: &'a App, game: &'a Game) -> &'a str {
    game.runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.as_str())
        .unwrap_or("默认 Runner")
}
```

- [ ] **Step 2: 验证**

Run: `cargo check -p ui`
Expected: PASS。

Run: `cargo run -p ui`
Expected: 游戏库显示页头、正在游玩主卡（如有运行中游戏）、封面网格与空状态；窗口拉伸时列数在 3/4 之间切换。

---

## Task 11: 游戏详情（卡片表单 + 封面摘要）

**Files:**
- Modify: `crates/ui/src/views/game_detail.rs`

- [ ] **Step 1: 重写详情页**

`crates/ui/src/views/game_detail.rs` 完整替换为：

```rust
use crate::app::{App, DeleteTarget, Message, Screen};
use crate::dialogs::DialogIntent;
use crate::draft::{ArgsAction, DllAction, DraftAction, EnvAction, GameDraft};
use crate::styles;
use crate::theme::font;
use crate::widgets::{badges, controls, cover, layout, path_text, Tone};
use application::PrefixStatus;
use iced::font::Weight;
use iced::widget::{
    column, container, horizontal_space, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length};
use launcher_core::GamescopeMode;

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(draft) = &app.draft else {
        return column![
            controls::secondary("返回").on_press(Message::Navigate(Screen::Library)),
            layout::empty_state("未选择游戏", "请从游戏库选择一个游戏", None),
        ]
        .spacing(16)
        .into();
    };

    let launch_state = app.launch_state_label(draft.id);
    let header = row![
        controls::secondary("返回").on_press(Message::Navigate(Screen::Library)),
        column![
            text(if draft.is_new {
                "新建游戏".to_owned()
            } else {
                draft.name.clone()
            })
            .font(font(Weight::Bold))
            .size(22),
            layout::muted(if draft.is_new {
                "保存后可启动、创建 Prefix 或安装 DXVK".to_owned()
            } else {
                format!(
                    "最近游玩：{}",
                    crate::widgets::format_timestamp(draft.last_played_at)
                )
            })
            .size(11),
        ]
        .spacing(2),
        horizontal_space(),
        controls::danger("删除游戏").on_press_maybe(
            (!draft.is_new && launch_state.is_none() && !app.pending.deleting)
                .then_some(Message::ConfirmDelete(DeleteTarget::Game(draft.id))),
        ),
        controls::primary("保存")
            .on_press_maybe((!app.pending.saving).then_some(Message::SaveDraft)),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let left = column![
        basic_card(draft),
        runner_card(app, draft),
        display_card(app, draft),
        env_card(draft),
        dll_card(draft),
        args_card(draft),
    ]
    .spacing(16)
    .width(Length::Fill);

    let body = row![
        container(scrollable(left).style(styles::scrollable).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
        summary_card(app, draft),
    ]
    .spacing(24)
    .width(Length::Fill)
    .height(Length::Fill);

    column![header, body]
        .spacing(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn basic_card(draft: &GameDraft) -> Element<'_, Message> {
    let content = column![
        layout::field(
            "名称",
            text_input("游戏名称", draft.name.as_str())
                .on_input(|value| Message::Draft(DraftAction::Name(value)))
                .style(styles::text_input)
                .width(Length::Fill)
        ),
        layout::field(
            "可执行文件",
            row![
                layout::faint(path_text(draft.executable_path.as_deref()))
                    .size(11)
                    .width(Length::Fill),
                controls::secondary_small("选择")
                    .on_press(Message::Pick(DialogIntent::PickExe)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        layout::field(
            "工作目录",
            row![
                layout::faint(path_text(draft.working_dir.as_deref()))
                    .size(11)
                    .width(Length::Fill),
                controls::secondary_small("选择")
                    .on_press(Message::Pick(DialogIntent::PickWorkingDir)),
                controls::quiet_small("清除").on_press_maybe(
                    draft
                        .working_dir
                        .is_some()
                        .then_some(Message::Draft(DraftAction::ClearWorkingDir))
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        layout::field(
            "架构",
            pick_list(
                crate::widgets::arch_choices(),
                Some(crate::widgets::arch_choice(draft.arch)),
                |choice| Message::Draft(DraftAction::Arch(choice.value)),
            )
            .style(styles::pick_list)
            .menu_style(styles::menu)
        ),
    ]
    .spacing(10)
    .width(Length::Fill);

    layout::section_card("基本信息", None, content)
}

fn runner_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let none_label = "使用默认 Runner";
    let choices = crate::widgets::runner_choices(&app.runners, none_label);
    let selected = crate::widgets::runner_choice(&app.runners, none_label, draft.runner_id);
    let default_prefix = app.settings.prefix_root.join(draft.id.to_string());
    let prefix_path = draft.prefix_path.clone().unwrap_or(default_prefix);
    let status = app.prefix_status.as_ref();
    let dxvk_text = path_text(app.dxvk_dir.as_deref());

    let mut content = column![
        layout::field(
            "Runner",
            pick_list(choices, Some(selected), |choice| {
                Message::Draft(DraftAction::Runner(choice.value))
            })
            .style(styles::pick_list)
            .menu_style(styles::menu)
        ),
        layout::field(
            "Prefix",
            layout::faint(prefix_path.display().to_string())
                .size(11)
                .width(Length::Fill)
        ),
        layout::field(
            "状态",
            row![
                badges::badge(prefix_status_label(status), prefix_tone(status)),
                layout::faint(format!("DXVK 目录：{dxvk_text}"))
                    .size(11)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
        ),
        row![
            controls::secondary_small("选择 Prefix 目录")
                .on_press(Message::Pick(DialogIntent::PickPrefixDir)),
            controls::quiet_small("清除").on_press_maybe(
                draft
                    .prefix_path
                    .is_some()
                    .then_some(Message::Draft(DraftAction::ClearPrefix))
            ),
            controls::secondary_small("创建 / 初始化").on_press_maybe(
                (!draft.is_new && !app.pending.prefix.contains(&draft.id))
                    .then_some(Message::CreatePrefix(draft.id))
            ),
            controls::danger_small("删除 Prefix").on_press_maybe(
                (!draft.is_new
                    && !app.pending.deleting
                    && matches!(
                        app.prefix_status,
                        Some(PrefixStatus::Initialized | PrefixStatus::ArchMismatch { .. })
                    ))
                .then_some(Message::ConfirmDelete(DeleteTarget::Prefix {
                    game_id: draft.id,
                    path: prefix_path.clone(),
                }))
            ),
        ]
        .spacing(8),
        row![
            controls::secondary_small("选择 DXVK 目录")
                .on_press(Message::Pick(DialogIntent::PickDxvkDir)),
            controls::primary_small("安装 DXVK").on_press_maybe(
                (!draft.is_new
                    && app.dxvk_dir.is_some()
                    && !app.pending.dxvk.contains(&draft.id))
                .then_some(Message::InstallDxvk(draft.id))
            ),
        ]
        .spacing(8),
    ]
    .spacing(10)
    .width(Length::Fill);

    if draft.is_new {
        content = content.push(layout::faint("保存游戏后才能创建 Prefix 或安装 DXVK").size(11));
    }
    if let Some(PrefixStatus::ArchMismatch { existing }) = &app.prefix_status {
        content = content.push(
            layout::faint(format!(
                "架构不匹配：现有 prefix 为 {existing:?}，请更换 Prefix 或删除后重建"
            ))
            .size(11),
        );
    }
    if app.prefix_status != Some(PrefixStatus::Initialized) {
        content = content.push(layout::faint("启动前必须有一个已初始化的 Prefix").size(11));
    }

    layout::section_card("兼容层与 Prefix", None, content)
}

fn display_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let base = draft.display.clone();
    let gamescope_effective = app.gamescope_path.is_some()
        && application::should_use_gamescope(
            &draft.display,
            app.session,
            app.gamescope_path.is_some(),
        );

    let backend_pick = {
        let base = base.clone();
        pick_list(
            crate::widgets::backend_choices(),
            Some(crate::widgets::backend_choice(draft.display.backend)),
            move |choice| {
                let mut config = base.clone();
                config.backend = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
    };

    let gamescope_pick = {
        let base = base.clone();
        pick_list(
            crate::widgets::gamescope_choices(),
            Some(crate::widgets::gamescope_choice(draft.display.gamescope)),
            move |choice| {
                let mut config = base.clone();
                config.gamescope = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
    };

    let monitor_pick = {
        let base = base.clone();
        pick_list(
            crate::widgets::monitor_choices(&app.monitors),
            Some(crate::widgets::monitor_choice(
                &app.monitors,
                draft.display.monitor.clone(),
            )),
            move |choice| {
                let mut config = base.clone();
                config.monitor = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
    };

    let fullscreen = {
        let base = base.clone();
        controls::switch_row(
            "全屏",
            "以全屏模式启动游戏",
            draft.display.fullscreen,
            move |value| {
                let mut config = base.clone();
                config.fullscreen = value;
                Message::Draft(DraftAction::Display(config))
            },
        )
    };

    let mut content = column![
        layout::field("后端", backend_pick),
        layout::faint("后端字段仅作记录；实际 X11/Wayland 以启动时的会话检测为准").size(11),
        layout::field("gamescope", gamescope_pick),
        layout::field(
            "显示器",
            row![
                monitor_pick,
                controls::secondary_small("刷新").on_press(Message::RefreshMonitors),
            ]
            .spacing(8)
        ),
        fullscreen,
    ]
    .spacing(10)
    .width(Length::Fill);

    if gamescope_effective {
        content = content.push(
            column![
                row![
                    layout::muted("分辨率").size(12).width(Length::Fixed(72.0)),
                    text_input("宽", draft.resolution_width.as_str())
                        .on_input(|value| Message::Draft(DraftAction::ResolutionWidth(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(90.0)),
                    layout::muted("×").size(12),
                    text_input("高", draft.resolution_height.as_str())
                        .on_input(|value| Message::Draft(DraftAction::ResolutionHeight(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(90.0)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
                row![
                    layout::muted("刷新率").size(12).width(Length::Fixed(72.0)),
                    text_input("例如 144", draft.refresh_rate.as_str())
                        .on_input(|value| Message::Draft(DraftAction::RefreshRate(value)))
                        .style(styles::text_input)
                        .width(Length::Fixed(100.0)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(8),
        );
    } else {
        content = content.push(
            layout::faint(
                "当前不会使用 gamescope：分辨率与刷新率不生效；选择显示器时会临时切换系统 primary",
            )
            .size(11),
        );
    }

    if !app.monitors.iter().any(|output| output.connected) {
        content = content.push(
            layout::faint("未检测到已连接显示器，点击“刷新”重新扫描").size(11),
        );
    }

    if app.session == application::SessionType::Wayland {
        let hint = if std::env::var_os("DISPLAY").is_some() {
            "选择显示器时会临时把该显示器设为系统 primary，并让 Wine/Proton 以 X11 模式启动；启动后会自动恢复原 primary。"
        } else {
            "未检测到 XWayland（DISPLAY），显示器选择无法保证生效。"
        };
        content = content.push(layout::faint(hint).size(11));
    }

    if app.gamescope_path.is_none() {
        if let Some(hint) = match draft.display.gamescope {
            GamescopeMode::Forced => Some("强制 gamescope 但未检测到 gamescope，启动会被拒绝"),
            GamescopeMode::Auto => {
                Some("未检测到 gamescope，自动模式不会生效，分辨率/刷新率不会应用")
            }
            GamescopeMode::Off => None,
        } {
            content = content.push(layout::faint(hint).size(11));
        }
    }

    layout::section_card("显示", None, content)
}

fn env_card(draft: &GameDraft) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, entry) in draft.env_rows.iter().enumerate() {
        rows = rows.push(
            row![
                text_input("KEY", entry.key.as_str())
                    .on_input(move |key| Message::Draft(DraftAction::Env(EnvAction::Key {
                        index,
                        key
                    })))
                    .style(styles::text_input)
                    .width(Length::FillPortion(2)),
                text_input("VALUE", entry.value.as_str())
                    .on_input(
                        move |value| Message::Draft(DraftAction::Env(EnvAction::Value {
                            index,
                            value
                        }))
                    )
                    .style(styles::text_input)
                    .width(Length::FillPortion(3)),
                controls::quiet_small("删除")
                    .on_press(Message::Draft(DraftAction::Env(EnvAction::Remove(index)))),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }
    rows = rows.push(
        controls::secondary_small("添加环境变量")
            .on_press(Message::Draft(DraftAction::Env(EnvAction::Add))),
    );
    rows = rows.push(
        layout::faint("WINEDLLOVERRIDES 请使用下方的结构化 DLL Override，不要手填").size(11),
    );

    layout::section_card("环境变量", None, rows)
}

fn dll_card(draft: &GameDraft) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, entry) in draft.dll_rows.iter().enumerate() {
        rows = rows.push(
            row![
                text_input("DLL 名称（不含 .dll）", entry.name.as_str())
                    .on_input(
                        move |name| Message::Draft(DraftAction::Dll(DllAction::Name {
                            index,
                            name
                        }))
                    )
                    .style(styles::text_input)
                    .width(Length::FillPortion(2)),
                pick_list(
                    crate::widgets::override_mode_choices(),
                    Some(crate::widgets::override_mode_choice(entry.mode)),
                    move |choice| Message::Draft(DraftAction::Dll(DllAction::Mode {
                        index,
                        mode: choice.value,
                    })),
                )
                .style(styles::pick_list)
                .menu_style(styles::menu),
                controls::quiet_small("删除")
                    .on_press(Message::Draft(DraftAction::Dll(DllAction::Remove(index)))),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }
    rows = rows.push(
        controls::secondary_small("添加 DLL Override")
            .on_press(Message::Draft(DraftAction::Dll(DllAction::Add))),
    );

    layout::section_card("DLL Override", None, rows)
}

fn args_card(draft: &GameDraft) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);
    for (index, argument) in draft.args_rows.iter().enumerate() {
        rows = rows.push(
            row![
                text_input("参数", argument.as_str())
                    .on_input(move |value| Message::Draft(DraftAction::Args(
                        ArgsAction::Value { index, value }
                    )))
                    .style(styles::text_input)
                    .width(Length::Fill),
                controls::quiet_small("删除")
                    .on_press(Message::Draft(DraftAction::Args(ArgsAction::Remove(index)))),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }
    rows = rows.push(
        controls::secondary_small("添加参数")
            .on_press(Message::Draft(DraftAction::Args(ArgsAction::Add))),
    );

    layout::section_card("启动参数", None, rows)
}

fn summary_card<'a>(app: &'a App, draft: &'a GameDraft) -> Element<'a, Message> {
    let preview = draft.cover_source.as_deref().or(draft.cover_path.as_deref());
    let handle = preview
        .filter(|path| path.is_file())
        .map(|path| iced::widget::image::Handle::from_path(path.to_path_buf()));
    let has_preview = preview.is_some();

    let launch: Element<'a, Message> = if app.starting.contains(&draft.id) {
        controls::secondary("启动中").into()
    } else if app.running.contains_key(&draft.id) {
        let stopping = app.stopping.contains(&draft.id);
        controls::danger(if stopping { "结束中" } else { "结束游戏" })
            .on_press_maybe((!stopping).then_some(Message::Stop(draft.id)))
            .into()
    } else {
        controls::primary("启动游戏")
            .on_press_maybe(
                (!draft.is_new && !app.pending.deleting).then_some(Message::Launch(draft.id)),
            )
            .into()
    };

    let runner = draft
        .runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.clone())
        .unwrap_or_else(|| "默认 Runner".to_owned());

    let status = app.prefix_status.as_ref();
    let facts = column![
        fact("Prefix", prefix_status_label(status).to_owned()),
        fact("Runner", runner.clone()),
        fact(
            "最近游玩",
            crate::widgets::format_timestamp(draft.last_played_at),
        ),
    ]
    .spacing(6)
    .width(Length::Fill);

    container(
        column![
            cover(draft.name.as_str(), draft.id, handle, 148.0, 197.0),
            text(draft.name.clone()).font(font(Weight::Bold)).size(16),
            layout::muted(format!(
                "{} · {}",
                runner,
                crate::widgets::arch_choice(draft.arch).label
            ))
            .size(11),
            launch,
            row![
                controls::secondary_small("选择封面")
                    .on_press(Message::Pick(DialogIntent::PickCover)),
                controls::secondary_small("默认").on_press_maybe(
                    has_preview.then_some(Message::Draft(DraftAction::ClearCover))
                ),
            ]
            .spacing(8),
            facts,
        ]
        .spacing(10)
        .width(Length::Fill)
        .align_x(iced::Alignment::Center),
    )
    .padding(16)
    .width(Length::Fixed(260.0))
    .style(styles::card)
    .into()
}

fn fact(label: &'static str, value: String) -> Element<'static, Message> {
    row![
        layout::muted(label).size(11),
        horizontal_space(),
        text(value).size(11).font(font(Weight::Semibold)),
    ]
    .width(Length::Fill)
    .into()
}

fn prefix_tone(status: Option<&PrefixStatus>) -> Tone {
    match status {
        Some(PrefixStatus::Initialized) => Tone::Live,
        Some(PrefixStatus::ArchMismatch { .. }) => Tone::Danger,
        Some(PrefixStatus::Missing | PrefixStatus::NotInitialized) => Tone::Starting,
        None => Tone::Neutral,
    }
}

fn prefix_status_label(status: Option<&PrefixStatus>) -> &'static str {
    match status {
        None => "未知（未解析 Runner 或未加载）",
        Some(PrefixStatus::Missing) => "未创建",
        Some(PrefixStatus::NotInitialized) => "目录存在，未初始化",
        Some(PrefixStatus::Initialized) => "已初始化",
        Some(PrefixStatus::ArchMismatch { .. }) => "架构不匹配",
    }
}
```

- [ ] **Step 2: 验证**

Run: `cargo check -p ui`
Expected: PASS。

Run: `cargo run -p ui`
Expected: 详情页为左侧卡片 + 右侧封面摘要；所有原有字段与按钮仍可用；点「选择封面」可选择图片并在摘要卡预览。

---

## Task 12: Runner 管理

**Files:**
- Modify: `crates/ui/src/views/runners.rs`
- Modify: `crates/ui/src/widgets/mod.rs`（新增共享 `pub fn truncate`）
- Modify: `crates/ui/src/views/library.rs`（改为 `use crate::widgets::truncate`）
- Modify: `crates/ui/src/app/update.rs`（动作去重与替换插入）

- [ ] **Step 1: 重写 Runner 管理**

`crates/ui/src/widgets/mod.rs` 新增共享截断函数（`views/library.rs` 删除本地定义并改为导入，`runners.rs` 同样复用）：

```rust
pub fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}
```

`crates/ui/src/views/runners.rs` 完整替换为：

```rust
use crate::app::{App, DeleteTarget, Message};
use crate::styles;
use crate::theme::font;
use crate::widgets::{Tone, badges, controls, layout, truncate};
use application::Runner;
use iced::font::Weight;
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Length};
use launcher_core::RunnerKind;

pub fn view(app: &App) -> Element<'_, Message> {
    if app.pending.loading {
        return layout::centered_message("正在加载…");
    }

    let busy = app.pending.detecting_wine || app.pending.importing_proton;
    let header = layout::page_header(
        "Runner 管理",
        "探测系统 Wine，或导入自备的 Proton / GE-Proton 目录",
        row![
            controls::secondary("重新探测系统 Wine")
                .on_press_maybe((!busy).then_some(Message::DetectSystemWine)),
            controls::primary("导入 Proton 目录")
                .on_press_maybe((!busy).then_some(Message::ImportProton)),
        ]
        .spacing(8),
    );

    let body: Element<'_, Message> = if app.runners.is_empty() {
        if app.error.is_none() {
            layout::empty_state(
                "尚未检测到 Runner",
                "先探测系统 Wine，或导入 Proton 目录",
                Some(
                    row![
                        controls::secondary("探测系统 Wine")
                            .on_press_maybe((!busy).then_some(Message::DetectSystemWine)),
                        controls::primary("导入 Proton 目录")
                            .on_press_maybe((!busy).then_some(Message::ImportProton)),
                    ]
                    .spacing(8)
                    .into(),
                ),
            )
        } else {
            layout::centered_message("暂无可用的 Runner")
        }
    } else {
        let mut grid = column![].spacing(16).width(Length::Fill);
        for chunk in app.runners.chunks(2) {
            let mut line = row![]
                .spacing(16)
                .width(Length::Fill)
                .align_y(iced::Alignment::Start);
            for runner in chunk {
                line = line.push(runner_card(app, runner));
            }
            if chunk.len() == 1 {
                line = line.push(iced::widget::horizontal_space());
            }
            grid = grid.push(line);
        }
        grid.into()
    };

    column![
        header,
        scrollable(body)
            .style(styles::scrollable)
            .height(Length::Fill)
    ]
    .spacing(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn runner_card<'a>(app: &'a App, runner: &'a Runner) -> Element<'a, Message> {
    let is_default = app.settings.default_runner_id == Some(runner.id);
    let in_use = app
        .games
        .iter()
        .filter(|game| game.runner_id == Some(runner.id))
        .count();
    let can_delete = !is_default && in_use == 0 && !app.pending.deleting;
    let kind = match runner.kind {
        RunnerKind::Wine => "Wine",
        RunnerKind::Proton => "Proton",
    };

    let mut title = row![
        text(truncate(runner.name.as_str(), 20))
            .font(font(Weight::Bold))
            .size(15)
            .width(Length::Fill)
            .height(Length::Fixed(20.0))
            .wrapping(iced::widget::text::Wrapping::None),
        badges::badge(kind, Tone::Neutral),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    if is_default {
        title = title.push(badges::badge("默认", Tone::Info));
    }

    let verified = if runner.verified {
        badges::badge("已验证", Tone::Live)
    } else {
        badges::badge("未验证", Tone::Warning)
    };

    let version = row![
        layout::faint(truncate(
            runner.version_string.as_deref().unwrap_or("版本未知"),
            28
        ))
        .size(11)
        .width(Length::Fill)
        .height(Length::Fixed(14.0))
        .wrapping(iced::widget::text::Wrapping::None),
        verified,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let path = layout::faint(middle_truncate(
        &runner.binary_path.display().to_string(),
        48,
    ))
    .size(11)
    .width(Length::Fill)
    .height(Length::Fixed(14.0))
    .wrapping(iced::widget::text::Wrapping::None);

    let actions = row![
        controls::secondary_small("设为默认").on_press_maybe(
            (!is_default && !app.pending.settings_saving)
                .then_some(Message::SetDefaultRunner(Some(runner.id)))
        ),
        controls::danger_small("删除").on_press_maybe(
            can_delete.then_some(Message::ConfirmDelete(DeleteTarget::Runner(runner.id)))
        ),
    ]
    .spacing(8);

    let mut content = column![title, version, path, actions]
        .spacing(8)
        .width(Length::Fill);

    if !can_delete {
        if is_default {
            content = content.push(layout::faint("默认 Runner，请先更换").size(11));
        } else if in_use > 0 {
            content = content.push(layout::faint(format!("{in_use} 个游戏正在使用")).size(11));
        }
    }

    layout::card(content)
}

fn middle_truncate(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_owned();
    }
    let keep = max_chars.saturating_sub(1);
    let front = keep / 2;
    let back = keep - front;
    let head: String = chars[..front].iter().collect();
    let tail: String = chars[chars.len() - back..].iter().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_truncate_keeps_both_ends() {
        assert_eq!(middle_truncate("short", 10), "short");
        assert_eq!(
            middle_truncate("/very/long/path/to/proton", 10),
            "/ver…roton"
        );
    }
}
```

要点：加载中显示占位；空状态提供“探测系统 Wine / 导入 Proton 目录”双按钮，有错误时改为居中提示；类型胶囊 `Tone::Neutral`、默认 `Tone::Info`、验证状态 `Tone::Live`/`Tone::Warning`；名称与版本单行截断，路径用 `middle_truncate` 保留首尾；卡片统一使用 `layout::card`；奇数行用 `horizontal_space()` 补齐；删除受默认/占用/删除中三重门控并给出提示。

`crates/ui/src/app/update.rs` 为 `DetectSystemWine`、`ImportProton`、`SetDefaultRunner`、`DeleteConfirmed` 增加忙碌守卫，`ProtonImported` 按 id 替换而非无条件追加。

- [ ] **Step 2: 验证**

Run: `cargo check -p ui`
Expected: PASS。

Run: `cargo test -p ui`
Expected: PASS（含 `middle_truncate_keeps_both_ends`）。

Run: `cargo run -p ui`
Expected: Runner 页显示两列卡片、类型/验证/默认胶囊与操作按钮；忙碌时按钮禁用。

---

## Task 13: 设置（外观分段控件 + 开关行）

**Files:**
- Modify: `crates/ui/src/views/settings.rs`

- [ ] **Step 1: 重写设置页**

`crates/ui/src/views/settings.rs` 完整替换为：

```rust
use crate::app::{App, Message, SettingsAction};
use crate::draft::EnvAction;
use crate::styles;
use crate::theme::{font, ThemeMode};
use crate::widgets::{controls, layout, path_text};
use iced::font::Weight;
use iced::widget::{column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = layout::page_header(
        "设置",
        "外观、默认兼容层与全局启动选项",
        controls::primary("保存")
            .on_press_maybe((!app.pending.settings_saving).then_some(Message::SaveSettings))
            .into(),
    );

    let appearance = layout::section_card(
        "外观",
        Some("切换立即生效并保存".to_owned()),
        row![
            layout::muted("主题").size(13).width(Length::Fill),
            theme_segment(app.settings.theme),
        ]
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    );

    let default_runner = layout::section_card(
        "默认 Runner",
        Some("未单独指定 Runner 的游戏会使用它".to_owned()),
        pick_list(
            crate::widgets::runner_choices(&app.runners, "无默认（使用游戏级选择）"),
            Some(crate::widgets::runner_choice(
                &app.runners,
                "无默认（使用游戏级选择）",
                app.settings.default_runner_id,
            )),
            |choice| Message::Settings(SettingsAction::DefaultRunner(choice.value)),
        )
        .style(styles::pick_list)
        .menu_style(styles::menu),
    );

    let prefix_root = layout::section_card(
        "Prefix 根目录",
        Some("新建 Prefix 的默认父目录".to_owned()),
        row![
            layout::faint(app.settings.prefix_root.display().to_string())
                .size(12)
                .width(Length::Fill),
            controls::secondary_small("选择目录").on_press(Message::Pick(
                crate::dialogs::DialogIntent::PickPrefixRoot
            )),
        ]
        .spacing(8)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    );

    scrollable(
        column![
            header,
            appearance,
            default_runner,
            prefix_root,
            global_env_card(app),
            compatibility_card(app),
        ]
        .spacing(16)
        .width(Length::Fill),
    )
    .style(styles::scrollable)
    .height(Length::Fill)
    .into()
}

fn theme_segment<'a>(selected: ThemeMode) -> Element<'a, Message> {
    let item = |mode: ThemeMode| {
        let active = selected == mode;
        let style: fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style =
            if active {
                styles::segment_selected
            } else {
                styles::segment
            };

        iced::widget::button(
            text(mode.label())
                .size(12)
                .font(font(if active { Weight::Semibold } else { Weight::Normal })),
        )
        .padding([5, 12])
        .style(style)
        .on_press(Message::Settings(SettingsAction::Theme(mode)))
    };

    container(row![item(ThemeMode::Light), item(ThemeMode::Dark)].spacing(2))
        .padding(2)
        .style(styles::segment_track)
        .into()
}

fn global_env_card(app: &App) -> Element<'_, Message> {
    let mut rows = column![].spacing(8).width(Length::Fill);

    for (index, entry) in app.settings.global_env_rows.iter().enumerate() {
        rows = rows.push(
            row![
                text_input("KEY", entry.key.as_str())
                    .on_input(move |key| Message::Settings(SettingsAction::Env(
                        EnvAction::Key { index, key }
                    )))
                    .style(styles::text_input)
                    .width(Length::FillPortion(2)),
                text_input("VALUE", entry.value.as_str())
                    .on_input(move |value| Message::Settings(SettingsAction::Env(
                        EnvAction::Value { index, value }
                    )))
                    .style(styles::text_input)
                    .width(Length::FillPortion(3)),
                controls::quiet_small("删除").on_press(Message::Settings(
                    SettingsAction::Env(EnvAction::Remove(index))
                )),
            ]
            .spacing(8)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center),
        );
    }

    rows = rows.push(
        controls::secondary_small("添加变量")
            .on_press(Message::Settings(SettingsAction::Env(EnvAction::Add))),
    );

    layout::section_card("全局环境变量", None, rows)
}

fn compatibility_card(app: &App) -> Element<'_, Message> {
    let source = app.settings.dxvk_nvapi_source.as_deref();
    let mut content = column![
        controls::switch_row(
            "启用 Esync",
            "减少同步开销（部分旧游戏可能不兼容）",
            app.settings.enable_esync,
            |value| Message::Settings(SettingsAction::EnableEsync(value)),
        ),
        controls::switch_row(
            "启用 Fsync",
            "减少文件系统同步开销",
            app.settings.enable_fsync,
            |value| Message::Settings(SettingsAction::EnableFsync(value)),
        ),
        controls::switch_row(
            "启用 MangoHud",
            match &app.mangohud_path {
                Some(_) => "启动时叠加性能监控",
                None => "未检测到 mangohud，启用后启动会被拒绝",
            },
            app.settings.enable_mangohud,
            |value| Message::Settings(SettingsAction::EnableMangoHud(value)),
        ),
        controls::switch_row(
            "自动安装并更新 DXVK-NVAPI",
            "Wine 游戏启动前检查并更新",
            app.settings.auto_install_dxvk_nvapi,
            |value| Message::Settings(SettingsAction::AutoInstallDxvkNvapi(value)),
        ),
        row![
            layout::faint(format!("DXVK-NVAPI 目录：{}", path_text(source)))
                .size(11)
                .width(Length::Fill),
            controls::secondary_small("选择目录").on_press(Message::Pick(
                crate::dialogs::DialogIntent::PickDxvkNvapiDir
            )),
            controls::quiet_small("清除").on_press_maybe(
                app.settings
                    .dxvk_nvapi_source
                    .is_some()
                    .then_some(Message::Settings(SettingsAction::DxvkNvapiSource(None)))
            ),
        ]
        .spacing(8)
        .width(Length::Fill)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(8)
    .width(Length::Fill);

    if app.settings.auto_install_dxvk_nvapi && app.settings.dxvk_nvapi_source.is_none() {
        content = content.push(
            layout::faint("已启用自动安装，但未选择 DXVK-NVAPI 目录；Wine 游戏启动会失败")
                .size(11),
        );
    }

    if let Some(path) = &app.mangohud_path {
        content = content.push(
            layout::faint(format!("已检测到 MangoHud：{}", path.display())).size(11),
        );
    }

    layout::section_card("兼容层增强", None, content)
}
```

- [ ] **Step 2: 验证**

Run: `cargo check -p ui`
Expected: PASS。

Run: `cargo run -p ui`
Expected: 设置页出现「外观」卡片；点浅色/深色立即切换整窗主题；重启后保持。

---

## Task 14: 日志页

**Files:**
- Modify: `crates/ui/src/views/logs.rs`

- [ ] **Step 1: 重写日志页**

`crates/ui/src/views/logs.rs` 完整替换为：

```rust
use crate::app::{App, Message, Screen};
use crate::styles;
use crate::theme;
use crate::widgets::{controls, layout};
use iced::widget::{column, container, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(game_id) = app.logs_game else {
        return column![
            controls::secondary("返回").on_press(Message::Navigate(Screen::Library)),
            layout::empty_state("未选择游戏", "请从游戏库打开一个游戏的日志", None),
        ]
        .spacing(16)
        .into();
    };

    let name = app
        .games
        .iter()
        .find(|game| game.id == game_id)
        .map(|game| game.name.as_str())
        .unwrap_or("游戏");

    let header = layout::page_header(
        format!("{name} 的日志"),
        "每秒自动刷新，显示最后 1000 行",
        controls::secondary("刷新")
            .on_press(Message::RefreshLogs)
            .into(),
    );

    let line_count = if app.logs_content.is_empty() {
        0
    } else {
        app.logs_content.lines().count()
    };

    let body: Element<'_, Message> = if app.logs_content.is_empty() {
        text("该游戏还没有启动日志")
            .size(13)
            .style(|t| iced::widget::text::Style {
                color: Some(theme::tokens(t).log_text),
            })
            .into()
    } else {
        scrollable(
            text(app.logs_content.as_str())
                .font(iced::Font::MONOSPACE)
                .size(12)
                .style(|t| iced::widget::text::Style {
                    color: Some(theme::tokens(t).log_text),
                }),
        )
        .style(styles::log_scrollable)
        .height(Length::Fill)
        .into()
    };

    let panel = container(
        column![
            text(format!("{line_count} 行"))
                .size(10)
                .style(|t| iced::widget::text::Style {
                    color: Some(theme::tokens(t).log_text),
                }),
            body,
        ]
        .spacing(8)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(styles::log_panel);

    column![header, panel]
        .spacing(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
```

- [ ] **Step 2: 验证**

Run: `cargo check -p ui`
Expected: PASS。

Run: `cargo run -p ui`
Expected: 打开某个游戏的日志，面板为深色、等宽字体、每秒刷新。

---

## Task 15: README 与全量验证

**Files:**
- Modify: `README.md`
- Modify: `crates/ui/src/widgets/mod.rs`

- [ ] **Step 1: 更新 README**

数据目录表增加一行：

```markdown
| `covers/` | 游戏封面副本（自定义封面） |
```

已知限制追加：

```markdown
- 自定义封面在 iced 0.13 中无法做圆角裁剪，显示为直角；默认渐变封面为圆角。
- 模态框打开时，滚轮仍可能滚动背后的页面（iced 0.13 `opaque` 只拦截点击）
```

- [ ] **Step 2: 格式化与静态检查**

Run: `cargo fmt --all`
Run: `cargo fmt --all --check`
Expected: PASS。

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS（若出现 `unused import`，删除对应导入；若出现 `too many arguments`，保持现有签名并添加 `#[allow(clippy::too_many_arguments)]` 到该函数）。

- [ ] **Step 3: 全量测试**

Run: `cargo test --workspace`
Expected: PASS。重点确认：

- `storage`：`migrates_v1_database_to_v2`、`in_memory_database_has_v2_schema`、`newer_schema_is_rejected`。
- `application`：`covers` 的 6 个用例、原有 orchestration 全部通过。
- `ui`：`theme` 两个用例、`settings_parse_theme_with_fallback`、`cover_actions_set_and_clear`。

- [ ] **Step 4: release 构建**

Run: `cargo build --release`
Expected: PASS，生成 `target/release/gamehub`。

- [ ] **Step 5: 数据库迁移手工验证**

先备份现有数据库：

```bash
cp ~/.local/share/gamehub/gamehub.db /tmp/gamehub.db.backup
```

再启动应用：

```bash
cargo run -p ui
```

Expected：应用正常打开，原有游戏列表完整；无迁移错误。检查版本：

```bash
sqlite3 ~/.local/share/gamehub/gamehub.db 'SELECT version FROM schema_version;'
```

Expected：`2`（若未安装 `sqlite3`，跳过本命令，以应用正常打开为准）。

- [ ] **Step 6: 手工验收清单**

在 GNOME Wayland 主机上逐项确认：

- [ ] 首次启动为浅色；侧栏、页头、卡片、间距与视觉稿一致。
- [ ] 设置 → 外观切深色：立即生效；重启后仍为深色；再切回浅色同样持久。
- [ ] 新建游戏：选择一个 PNG 封面 → 右侧摘要卡出现预览 → 保存 → 库页卡片显示封面。
- [ ] 重启应用后封面仍在（文件位于 `~/.local/share/gamehub/covers/<id>.png`）。
- [ ] 编辑游戏替换封面：旧文件被删除，新文件存在；点「默认」回退为渐变封面。
- [ ] 删除游戏：居中模态框显示游戏名；确认后游戏、日志与 `covers/<id>.*` 均被删除。
- [ ] 运行游戏时库页出现「正在游玩」主卡；结束按钮可用；状态胶囊正确。
- [ ] 日志页深色面板、等宽字体、行数与每秒刷新正常。
- [ ] 窗口缩到 900×600 时库页为 3 列且不溢出；1180×760 为 4 列。
- [ ] 所有原有功能（Prefix 创建/删除、DXVK 安装、显示器刷新、Runner 导入/默认/删除、设置保存）仍可用。

- [ ] **Step 7: 对照规格自检**

先清理临时过渡代码：仅删除 `crates/ui/src/widgets/mod.rs` 中的 `section` 包装函数（保留 `layout` 模块与其 `section_card`）。`confirm.rs` 及其 `mod confirm;` / `pub use confirm::confirm_bar;` 已在 Task 9 删除，本步骤不再处理。

用以下两条命令确认没有残留引用后再执行最终检查：

Run: `rg "widgets::section|confirm_bar" crates/ui/src`
Run: `rg "use crate::widgets::\{[^}]*section" crates/ui/src`
Expected: 均无输出。

打开 `docs/superpowers/specs/2026-10-27-ui-modernization-design.md`，逐节确认实现覆盖；若发现遗漏，记录在完成总结中并补齐。按用户偏好，此时不 commit，向用户汇报改动与验证结果。





