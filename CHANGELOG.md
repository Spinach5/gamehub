# Changelog

本文件记录每个实施阶段完成的内容、与规格的偏差以及验证结果。

## P2 — 存储

### 完成内容

- SQLite v1 schema（WAL、5s busy timeout、外键开关）、schema_version 表与
  版本检查：新库自动建表，旧版本报 `MigrationRequired`，未来版本报
  `UnsupportedSchema`。
- `Db::open` 自动创建数据库父目录；`ensure_app_dirs` 创建应用全部目录。
- Game/Runner CRUD；Game 的 JSON 列（display/env/dll/args）与 Runner
  `source_json`；列表按名称 `NOCASE` 排序。
- Settings 键值表：upsert、查询、删除、全量 BTreeMap。
- 删除前引用计数：`count_games_using_runner`、`count_games_using_prefix`。
- 可诊断错误：损坏枚举/UUID（`CorruptValue` 带列名）、损坏 JSON、
  非 UTF-8 路径（`NonUtf8Path` 带列名）、重复 runner 二进制路径。
- `runners.binary_path` 唯一约束。

### 与规格的偏差

- 为避免 S2 提交出现 `dead_code` 警告，`row_to_runner` 与
  `parse_runner_kind` 延后到 S3 才加入 `mapping.rs`；最终公开 API 与
  spec 完全一致。
- 测试模块不再重复 `use rusqlite::params`（`use super::*` 已引入）。

### 验证

- 绑定 SQLite 版本：3.54.1（libsqlite3-sys 0.39.1 bundled）。
- `cargo test -p storage`：18 passed。
- `cargo clippy -p storage --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。

## P1 — 核心类型

### 完成内容

- `launcher-core::types` 定义跨 crate 契约：`Arch`、`RunnerKind`、
  `RunnerSource`、`Runner`、`OverrideMode`、`DllOverride`、`DisplayBackend`、
  `GamescopeMode`、`DisplayConfig`、`Game`。
- 枚举统一 `snake_case` serde 表示；`Arch`/`RunnerKind` 的 `as_str` 与 serde
  输出由测试保证一致，`parse` 仅接受小写。
- `RunnerSource` 使用内部 tag：`{"kind":"system"}` /
  `{"kind":"imported","original_path":...}`。
- `DisplayConfig::default()`：Auto / None / None / None / fullscreen / Auto。
- 集成测试 `tests/contracts.rs` 锁定 JSON 字段与未知枚举拒绝行为。

### 与规格的偏差

- 无。字段名、默认值与 spec 完全一致。

### 验证

- `cargo test -p launcher-core`：22 passed（20 单测 + 2 集成）。
- `cargo clippy -p launcher-core --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。

## P0 — 基线与冒烟

### 完成内容

- 将单 package 仓库转换为 7-crate Cargo workspace：
  `launcher-core`、`storage`、`runner-wine`、`prefix`、`display`、`launch`、`ui`。
- 锁定并提交依赖：iced 0.13.1（启用 `tokio` feature）、tokio 1.54、
  rusqlite 0.41（bundled）、rfd 0.17.2、serde、thiserror 2、tracing、uuid、chrono。
- `launcher-core`：
  - `AppPaths`：`$XDG_DATA_HOME/gamehub` 或 `$HOME/.local/share/gamehub`，
    派生数据库、日志、prefix 根、Proton 兼容客户端目录。
  - `truncate_utf8`：按 UTF-8 字符边界截断，用于限制系统命令 stderr。
- `ui`：tracing 双输出（stderr + 每日滚动日志文件）；Iced 0.13 + tokio
  执行器 + 中文渲染冒烟窗口。

### 与规格的偏差

- 规格假设仓库尚无 commit；实际仓库已有初始 commit `f9f6fa6`，
  因此移动 `src/main.rs` 时直接使用 `mv`（仍不使用 `git mv`）。
- 本机 GNOME 会话禁止通过 D-Bus/ImageMagick 截屏，中文渲染的目视确认
  无法自动完成；已确认系统存在 Noto Sans CJK SC 与 WenQuanYi Zen Hei，
  且 fontconfig 对 `U+6E38`（游）解析到 WenQuanYi Zen Hei。

### 验证

- `cargo build --workspace`：通过。
- `cargo test --workspace`：8 passed（launcher-core）。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。
- `cargo run -p ui`：窗口进程持续运行 6 秒，无 panic、
  无 "no reactor running"；`~/.local/share/gamehub/logs/gamehub.log.2026-10-26` 已生成。
- `cargo tree --depth 0`：iced 0.13.1、rusqlite 0.41.0、rfd 0.17.2。
