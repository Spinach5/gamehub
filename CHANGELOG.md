# Changelog

本文件记录每个实施阶段完成的内容、与规格的偏差以及验证结果。

## P5 — display

### 完成内容

- 会话检测：`XDG_SESSION_TYPE` 精确匹配 `x11`/`wayland`，缺失或其他值
  （包括大小写不匹配）一律为 Unknown。
- `xrandr --query` 解析：仅把第二个 token 为 `connected`/`disconnected`
  的行当作输出头；带 `+offset` 的几何 token 判定为活动输出；模式行以 `*`
  标记当前模式，小数刷新率四舍五入为整数（143.88 → 144）；断开与非活动
  输出仍保留在结果中。
- PATH 查找：按 `split_paths` 顺序返回第一个可执行普通文件，跳过目录与
  无执行位文件；`gamescope_path` 用 `OnceLock` 缓存一次 PATH 扫描。
- gamescope 真值表：Off 永不启用；Auto 仅在 Wayland 且二进制可用时启用；
  Forced 始终启用。参数顺序固定为 `-W/-H`、`-r`、`-f`、`-O <monitor>`
  （仅 Wayland）、`--backend wayland|x11`（Unknown 省略），始终以 `--` 结尾。

### 与规格的偏差

- 无测试计数偏差：D1=2、D2=6、D3=9、D4=16，与计划一致。
- D3 测试中 clippy 报 `useless_conversion`，将 `PathBuf::from(gamescope)`
  改为直接比较，并删除随之未使用的 `PathBuf` 导入。
- 本机仅有 HDMI-1 一个输出（无 eDP-1），D2 使用规格给定的典型笔记本样例
  覆盖 eDP-1 活跃输出与 143.88 → 144 的刷新率取整。

### 验证

- 本机 `xrandr --query`：`HDMI-1 connected primary 1920x1080+0+0`，
  当前模式 `1920x1080 143.88*+`，screen 当前 3840x1080。
- 本机未安装 gamescope（`command -v gamescope` 无结果）；
  `find_in_path`/`gamescope_path` 由临时目录中的伪造可执行文件覆盖，
  Auto 的可用性分支用 `available` 参数覆盖。
- `cargo test -p display`：16 passed。
- `cargo test --workspace`：98 passed，1 ignored。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo build --workspace`：通过。

## P4 — prefix 与 DXVK

### 完成内容

- `PrefixPlan` 与 `plan_for_game`：Wine 使用 `game.arch`，Proton 固定 win64；
  `prefix_path` 显式值优先，否则 `<default_root>/<game_id>`。
- `windows_root` 封装布局差异：Wine 在 prefix 根目录，Proton 在 `pfx/`；
  `detect_arch` 读取 `system.reg` 前 8 KiB 的 `#arch=` 标记。
- `PrefixStatus`：Missing / NotInitialized / Initialized / ArchMismatch。
- `initialize_with_timeout`：已初始化返回 `Ok(false)`；arch 不匹配在 spawn 前报错；
  `wineboot -u` 带完整 Wine/Proton 环境、piped stderr、超时 `kill_on_drop`、
  stderr 4 KiB 截断；wineboot 后必须以 `system.reg` 存在确认成功。
- DXVK：校验用户源目录（x64/x32），win64 映射 x64→system32、x32→syswow64，
  win32 映射 x32→system32；d3d11/dxgi 必需，d3d10core/d3d9 可选；
  复制后对每个唯一 DLL 执行一次 `reg add ... DllOverrides /d native,builtin /f`
  （Proton 走 `proton run reg add`），超时 30 s，错误携带 DLL 名与截断 stderr。
- `delete_prefix`：幂等；仅删除含 `system.reg` 或 `pfx/system.reg` 的目录，
  否则 `RefusingToDeleteNonPrefix`。

### 与规格的偏差

- 计划中的测试计数与实际测试模块定义不一致（PF1 10 而非 8，之后依次为
  14/15/20/23/25+1 ignored，而非 12/13/18/21/23+1）。
- `test_support` 额外提供 `EXEC_LOCK`（tokio Mutex），串行化"写脚本并 spawn"
  的测试，避免 P3 已复现的 ETXTBSY 竞态；prefix 全量测试 30 次运行 0 失败。
- `detect_arch` 的 `File::open` 去掉不必要的 `mut`（clippy）。
- PF5 测试删除未使用的 `PathBuf` 导入（clippy）。

### 验证

- 真实 wineboot：14.23 s 完成 win64 prefix 初始化（Wine 10.0）。
- DXVK：本机无 DXVK，也未引入下载；由伪造源目录与假 runner 覆盖。
- `cargo test -p prefix`：25 passed，1 ignored；
  `--ignored` 真实 Wine 测试通过。
- `cargo test --workspace`：82 passed，1 ignored。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all --check`、
  `cargo build --workspace`：通过。

## P3 — runner-wine

### 完成内容

- `WineContext`：Wine 环境恰好是 `WINEPREFIX`/`WINEARCH`/`WINEDEBUG=-all`，
  Proton 环境恰好是 `STEAM_COMPAT_DATA_PATH`/`STEAM_COMPAT_CLIENT_INSTALL_PATH`/
  `PROTONPATH`；两种 runner 的环境变量互不泄漏。
- 命令构造：Wine 为 `(wine, [exe, args...])`，Proton 为
  `(proton, ["run", exe, args...])`，纯函数、不触碰文件系统。
- `WineContext::spawn` 以 base env + extra env 构造 tokio 子进程，
  `kill_on_drop(false)`，失败映射为 `RunnerError::Spawn`。
- 系统 Wine 探测：PATH 条目 + `/usr/bin/wine{,-stable,-development}` 候选项，
  按 canonical path 去重；逐个执行 `--version`，默认 5s 超时，失败仅告警不 panic。
- Proton 目录校验：要求 `root/proton` 存在且可执行（规范化路径后检查），
  版本优先读 `version` 文件，否则用目录名；导入 runner 固定 win64 且 verified。
- `verify_runner` 在启动前检查二进制存在性与可执行位，分别报
  `MissingBinary`/`NotExecutable`。
- `RunnerError` 覆盖目录/二进制校验、规范化、探测失败/超时、spawn 与 IO。

### 与规格的偏差

- R1 计划预计 7 个测试，实际测试模块定义 6 个（计划计数笔误），后续数量与
  计划一致（R2=13、R3=16、R4=17）。
- `verify_runner` 实现覆盖 `NotExecutable`，计划测试只断言 `MissingBinary`。
- 并行测试暴露 ETXTBSY 竞态（fork 继承其它测试正在写入的脚本 fd，
  复现率 5/40）；新增仅测试可见的 `tokio::sync::Mutex`（`EXEC_LOCK`）
  串行化"写脚本并 spawn"的测试，修复后 50 次全量运行 0 失败。

### 验证

- 真实环境：Wine 10.0（`wine-10.0 (Debian 10.0~repack-6)`），
  `detect_system_wine()` 探测到 1 个 runner；本机无 Proton，
  `validate_proton_dir` 由伪造目录测试覆盖。
- `cargo test -p runner-wine`：17 passed；`cargo test --workspace`：57 passed。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`、`cargo build --workspace`：通过。

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
