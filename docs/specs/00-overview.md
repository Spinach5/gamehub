# gameHub 实施规格总览（v2）

> **For agentic workers:** 本目录是可直接执行的实施规格。按 `docs/plan.md` 的阶段顺序逐份实施；每份 spec 内的任务按顺序执行，使用 `superpowers:subagent-driven-development` 或 `superpowers:executing-plans`，每个任务结束后提交一次。步骤使用 `- [ ]` 复选框跟踪。

**Goal:** 在 Linux（X11/Wayland）上交付一个完全离线可用的 Windows 游戏启动器：导入本地 exe、管理 Wine/Proton runner、创建 prefix、注入用户提供的 DXVK、配置显示与 gamescope、启动游戏。

**Architecture:** Cargo workspace + 7 个 crate。`launcher-core` 只放纯类型、枚举映射和路径计算；`storage` 负责 SQLite；`runner-wine`、`prefix`、`display`、`launch` 各自封装一个职责；`ui` 是唯一的 Iced 二进制入口。依赖方向单向，禁止环。

**Tech Stack:** Rust stable / edition 2024、Iced 0.13.1、tokio 1.54、rusqlite 0.41（bundled）、rfd 0.17.2、serde/serde_json、thiserror 2、tracing、uuid v4、chrono（仅用于时间显示）。

---

## 1. 本机环境事实（2026-10-26 实测）

这些事实决定了本规格中的版本与验证方式，实施时不要假设其他环境。

| 项 | 实测值 |
|---|---|
| rustc / cargo | 1.99.1 / 1.99.1 |
| 当前仓库 | 单 package（`gameHub`，edition 2024），只有 `src/main.rs`，尚无 commit |
| 会话 | `XDG_SESSION_TYPE=wayland`，`WAYLAND_DISPLAY=wayland-0`，`DISPLAY=:0`（XWayland） |
| Wine | `/usr/bin/wine`，`wine-10.0 (Debian 10.0~repack-6)` |
| xrandr | 可用，XWayland 输出 `eDP-1 connected primary 1920x1080+0+0`，刷新率显示为 `143.88` 这类小数 |
| gamescope | **未安装**（只能做参数构造测试，运行时验证需另找机器） |
| 中文字体 | 系统已有（FangSong/NSimSun/MingLiU 等）；Iced 0.13 文本栈为 cosmic-text，原生平台会加载系统字体 |
| rfd | 0.17.2 默认特性 `xdg-portal + wayland`，不依赖 GTK/WebKit；需要桌面 portal 服务 |
| crates 源 | `~/.cargo/config.toml` 将 crates-io 指向 USTC 镜像，可联网拉取 |
| 依赖冒烟 | `iced 0.13.1 + tokio(full) + rusqlite 0.41 bundled + rfd 0.17 + thiserror 2` 已在本机 `cargo check` 通过（38s） |

---

## 2. 产品边界（不扩大）

实现：本地 exe 导入、库管理、Wine/Proton 探测与导入、每游戏或共享 prefix、DXVK 文件注入、X11 输出选择、Wayland gamescope、环境变量与结构化 DLL override、全离线运行。

明确不做：商店/下载/云同步/成就/封面抓取/mod 管理/Steam 客户端集成/Windows 或 macOS；不实现 Wine/Proton 本身；不修改游戏二进制；不自动下载 DXVK。

---

## 3. 最终仓库结构

P0 会把当前单 package 改造成下面的 workspace。当前根目录的 `src/main.rs` 移到 `crates/ui/src/main.rs` 后删除。

```
gameHub/
├── Cargo.toml                  # [workspace] + workspace.dependencies + profile
├── Cargo.lock                  # 必须提交
├── .gitignore
├── CHANGELOG.md
├── README.md
├── docs/
│   ├── plan.md                 # v2 入口索引
│   └── specs/
│       ├── 00-overview.md
│       ├── 01-foundation.md
│       ├── 02-core-types.md
│       ├── 03-storage.md
│       ├── 04-runner-wine.md
│       ├── 05-prefix-dxvk.md
│       ├── 06-display.md
│       ├── 07-launch.md
│       ├── 08-ui-shell.md
│       ├── 09-ui-runners-prefix-settings.md
│       ├── 10-ui-config-launch.md
│       ├── 11-acceptance.md
│       └── archive/plan-v1.md
└── crates/
    ├── launcher-core/
    │   └── src/{lib.rs,types.rs,paths.rs,text.rs,error.rs}
    ├── storage/
    │   └── src/{lib.rs,schema.rs,mapping.rs,error.rs}
    ├── runner-wine/
    │   └── src/{lib.rs,detect.rs,context.rs,error.rs}
    ├── prefix/
    │   └── src/{lib.rs,status.rs,initialize.rs,dxvk.rs,error.rs}
    ├── display/
    │   └── src/{lib.rs,session.rs,xrandr.rs,gamescope.rs}
    ├── launch/
    │   └── src/{lib.rs,resolve.rs,env.rs,plan.rs,spawn.rs,error.rs}
    └── ui/
        └── src/
            ├── main.rs
            ├── lib.rs
            ├── app.rs
            ├── draft.rs
            ├── dialogs.rs
            ├── tasks.rs
            ├── widgets.rs
            └── views/{mod.rs,library.rs,game_detail.rs,runners.rs,settings.rs}
```

文件职责边界：

- `launcher-core`：类型、serde 契约、枚举 ↔ 字符串映射、`AppPaths` 计算、UTF-8 截断。**不依赖** tokio/rusqlite/iced，也不执行文件系统命令。
- `storage`：SQLite 连接、迁移、行映射、CRUD、引用计数。仅它可以直接依赖 rusqlite。
- `runner-wine`：runner 探测、Proton 目录校验、`WineContext` 环境与命令行构造、`spawn`。
- `prefix`：prefix 路径解析、状态/arch 检测、wineboot 初始化、DXVK 复制与注册表写入、prefix 删除（调用方负责引用检查）。
- `display`：会话检测、xrandr 解析、gamescope 查找与参数构造。无 UI、无 DB。
- `launch`：runner 解析、启动前校验、环境合成、启动计划、spawn。
- `ui`：Iced 状态机、视图、rfd 对话框、异步任务编排；所有 DB/文件/子进程重活都在 `spawn_blocking` 或 tokio task 中。

依赖方向（禁止反向）：

```
ui ──> launch, prefix, display, runner-wine, storage, launcher-core
launch ──> prefix, display, runner-wine, storage, launcher-core
prefix ──> runner-wine ──> launcher-core
display ──> launcher-core
storage ──> launcher-core
```

---

## 4. 锁定依赖与 workspace 配置

根 `Cargo.toml`（P0 直接照此建立）：

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[workspace.dependencies]
launcher-core = { path = "crates/launcher-core" }
storage = { path = "crates/storage" }
runner-wine = { path = "crates/runner-wine" }
prefix = { path = "crates/prefix" }
display = { path = "crates/display" }
launch = { path = "crates/launch" }

iced = { version = "0.13.1", features = ["tokio", "advanced"] }
tokio = { version = "1.54", features = ["rt-multi-thread", "macros", "process", "sync", "time", "fs"] }
rusqlite = { version = "0.41", features = ["bundled"] }
rfd = "0.17.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
uuid = { version = "1.28", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "ansi"] }
tracing-appender = "0.2"
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
tempfile = "3"

[workspace.lints.rust]
unsafe_code = "forbid"

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"
```

关键点（不要改）：

- **`iced` 必须启用 `tokio` feature**。否则 `Task::perform` 运行在 iced 自带的线程池执行器上，调用 `tokio::process` / `tokio::task::spawn_blocking` 会 panic（no reactor running）。
- 保留 iced 默认的特性（`wgpu + tiny-skia + fira-sans + auto-detect-theme`），渲染器会自动在 wgpu 失败时回退 tiny-skia。
- 各 crate 的 `Cargo.toml` 使用 `edition.workspace = true`、`version.workspace = true`、`rust-version.workspace = true`，依赖使用 `xxx.workspace = true`。
- `Cargo.lock` 必须提交；禁止 `cargo update` 后不提交。

每个 crate 依赖白名单：

| crate | 允许的依赖 |
|---|---|
| launcher-core | serde, uuid, thiserror；dev-dependency 允许 serde_json |
| storage | launcher-core, rusqlite, serde_json, thiserror, uuid |
| runner-wine | launcher-core, tokio(process/time), thiserror, tracing, uuid |
| prefix | launcher-core, runner-wine, tokio(process/time/fs), thiserror, tracing, uuid |
| display | launcher-core, tracing |
| launch | launcher-core, storage, prefix, display, runner-wine, tokio(process), thiserror, tracing, uuid |
| ui | iced, tokio, rfd, launcher-core, storage, prefix, display, runner-wine, launch, tracing, tracing-subscriber, tracing-appender, serde_json, uuid, chrono |

禁止的依赖：任何 Web/WebView/Node/Electron/Tauri 技术、`egui`/`gtk`/`qt`、网络 HTTP 客户端、自动下载库。

---

## 5. 全局约定

### 5.1 异步与阻塞

- `view` / `update` 中禁止文件 IO、DB、子进程、`rfd` 调用。
- DB 查询/写入：`tokio::task::spawn_blocking`，内部 `db.lock()`。
- 文件对话框、Proton 目录校验、xrandr、DXVK 复制：`spawn_blocking`。
- 子进程：使用 `tokio::process::Command`，并设置超时。
- `Db` 由 `Arc<std::sync::Mutex<Db>>` 持有；`rusqlite::Connection` 是 `Send` 非 `Sync`，Mutex 使其可共享。

### 5.2 错误

- 每个库 crate 一个 `thiserror` 枚举，库层返回 `Result`；UI 转成用户可读的 `AppError { title, detail }`。
- 系统命令失败时捕获 stderr，并用 `launcher_core::text::truncate_utf8(&stderr, 4096)` 截断后再放进错误。
- 禁止对系统调用/命令结果 `unwrap()`/`expect()`（测试除外）。
- 子进程错误必须带退出码（`Option<i32>`）和截断后的 stderr。

### 5.3 路径与数据目录

- 应用名固定 `gamehub`。
- `AppPaths::discover()`：`$XDG_DATA_HOME/gamehub`，否则 `$HOME/.local/share/gamehub`；两者都没有则报错。
- 默认路径：
  - DB：`<data_root>/gamehub.db`
  - 日志：`<data_root>/logs/`
  - 默认 prefix 根：`<data_root>/prefixes/`（可在设置中覆盖）
  - Proton 兼容客户端目录：`<data_root>/steam-compat-client/`
- 数据库中的路径必须是 UTF-8；导入时遇到非 UTF-8 路径直接报错（`StorageError::NonUtf8Path`），不做有损转换。
- `Game.prefix_path` 必须是绝对路径；`None` 表示 `<prefix_root>/<game_id>`。

### 5.4 设置键（`settings` 表，值是 TEXT）

| key | 值 |
|---|---|
| `global_env` | JSON object，`{"KEY":"VALUE"}` |
| `default_runner_id` | UUID 字符串 |
| `prefix_root` | 绝对路径字符串 |

### 5.5 离线

- 运行期不得发起任何网络请求。首期没有任何联网代码。
- `rfd` 使用 xdg-portal（D-Bus）不是网络；`xrandr`、`wine`、`gamescope` 是本地子进程。
- 验收用 `strace` 时只关注 `AF_INET`/`AF_INET6`；`AF_UNIX`（D-Bus/X11）与 `AF_NETLINK`（接口枚举）是正常现象。

### 5.6 UI 语言与字体

- UI 文案使用简体中文，代码标识符用英文。
- Iced 0.13 使用 cosmic-text，原生平台自动加载系统字体并做缺字回退；不打包字体文件。P0 必须人工确认中文不出现方块。

---

## 6. v1 方案的修正与优化（实施时以 v2 为准）

| # | v1 内容 | v2 修正 | 原因 |
|---|---|---|---|
| 1 | 直接在单 package 上实现 `crates/*` | P0 先转换为 workspace，`src/main.rs` 移入 `crates/ui` | 当前仓库是单 package，不迁移会与第 3 节冲突 |
| 2 | `iced::application(...).run_with(App::new)`，示例用 `Command` | Iced 0.13 用 `Task`，`run_with` 返回 `(App, Task)`；事件循环用 tokio feature | 0.13 API 已把 `Command` 改名 `Task`；否则编译失败 |
| 3 | 未指定 iced 执行器 | `iced` 启用 `tokio` feature | 不启用时 `tokio::process` 无 reactor，运行期 panic |
| 4 | rfd “无 portal 时依赖 zenity” | rfd 0.17 默认 `xdg-portal + wayland`，无 GTK/zenity；需要 portal 服务 | 原描述与 0.17 实际特性不符 |
| 5 | Proton prefix 的 `system.reg` 在 `prefix/`，DXVK 写到 `prefix/drive_c` | Proton (`STEAM_COMPAT_DATA_PATH`) 下是 `pfx/`；状态、初始化、DXVK 都必须写 `prefix/pfx/...` | Proton 脚本要求 `STEAM_COMPAT_DATA_PATH/pfx`，原路径会导致永远“未初始化” |
| 6 | `WineContext { runner, prefix, arch }` | 增加 `steam_compat_client: PathBuf` | Proton 需要 `STEAM_COMPAT_CLIENT_INSTALL_PATH`，且该值必须显式传入以便测试 |
| 7 | `OverrideMode::Disabled` 和 `Native` 都映射为 `n` | `Disabled` 映射为 Wine 的 `disabled`（不是 `n`） | `n` 表示 native，语义错误 |
| 8 | X11 选择分辨率后无实际作用 | X11 非 gamescope 路径只设置 `__GL_SYNC_DISPLAY_DEVICE`；分辨率/刷新仅在 gamescope 生效时启用，UI 需禁用并提示 | 原方案没有 xrandr 切模式逻辑，避免 UI 撒谎 |
| 9 | `PrefixPlan::default_for` 隐式读环境 | 所有路径显式传入：`default_root`、`steam_compat_client` | 便于测试，避免深层隐式 IO |
| 10 | gamescope 用 `"gamescope"` 字符串 | `gamescope_path() -> Option<PathBuf>`，使用绝对路径 | PATH 查找失败时错误信息明确，且可测试 |
| 11 | `LaunchHandle` + `Launched` + `GameExited` 两条消息 | 去掉 `LaunchHandle`；UI 用一条终态消息 `LaunchFinished`，点击后乐观标记运行中 | 避免为“中途发消息”引入事件通道复杂度 |
| 12 | DB 无引用计数查询 | `storage` 增加 `count_games_using_runner`、`count_games_using_prefix`、`delete_game`、`get_all_settings` | UI 删除保护需要 |
| 13 | `build_env` 直接读 `std::env` | `LaunchRequest.inherited_env` 注入继承环境；UI 用 `vars_os` 过滤非 UTF-8 | 测试可确定，避免并行测试改环境变量 |
| 14 | spawn 只 `.envs(env)` | `spawn_plan` 先 `.env_clear()` 再 `.envs(env)` | `build_env` 已包含继承环境，避免重复/不确定 |
| 15 | 启动子进程无超时 | wineboot/探测带超时；`kill_on_drop(true)` | 防止 wineboot 卡死拖住 UI |
| 16 | prefix 无状态/arch 检测 | 增加 `PrefixStatus` 与从 `system.reg` 的 `#arch=` 检测 | 启动前能给出明确错误；防止 arch 冲突 |
| 17 | UI `DraftChanged(DraftField, String)` 无法编辑键值表 | 引入 `GameDraft` + `EnvRow`/`DllRow`/`args_rows`，保存时统一校验 | 环境变量/DLL/参数需要增删行，且要保留用户输入 |
| 18 | 启动后可选 `wineserver -k` | 移除自动清理 | 共享 prefix 下可能误杀其他游戏；wineserver 会随进程退出自行结束 |
| 19 | 离线验收用 `strace -e trace=network` 直接判定 | 只检查 `AF_INET/AF_INET6` | netlink/UNIX socket 是正常本地通信 |
| 20 | prefix 初版要求真实 Wine 测试 | 用临时伪造 `wine`/`proton` 脚本做主力测试；真实 Wine 10.0 测试 `#[ignore]` | CI/本机都可在无游戏时验证 |
| 21 | prefix 删除前引用数 >0 一律拒绝 | 仅当前游戏引用时可清除引用并删除；共享/其他游戏引用仍拒绝；且只允许删除含 `system.reg` 或 `pfx/system.reg` 的目录 | 单游戏 prefix 可清理，同时防止误删任意目录 |

---

## 7. 阶段与任务索引

每一阶段完成后必须：`cargo build --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、更新 `CHANGELOG.md`。未通过不得进入下一阶段。

| 阶段 | 文档 | 任务 ID | 依赖 |
|---|---|---|---|
| P0 基线与冒烟 | [01-foundation.md](01-foundation.md) | F1–F7 | — |
| P1 核心类型 | [02-core-types.md](02-core-types.md) | C1–C4 | P0 |
| P2 存储 | [03-storage.md](03-storage.md) | S1–S6 | P1 |
| P3 runner | [04-runner-wine.md](04-runner-wine.md) | R1–R4 | P1 |
| P4 prefix/DXVK | [05-prefix-dxvk.md](05-prefix-dxvk.md) | PF1–PF6 | P3 |
| P5 display | [06-display.md](06-display.md) | D1–D5 | P1 |
| P6 launch | [07-launch.md](07-launch.md) | L1–L6 | P2–P5 |
| P7 UI 骨架 | [08-ui-shell.md](08-ui-shell.md) | U1–U6 | P2–P6 |
| P8 Runner/Prefix/Settings UI | [09-ui-runners-prefix-settings.md](09-ui-runners-prefix-settings.md) | U7–U11 | P7, P4 |
| P9 配置与启动 UI | [10-ui-config-launch.md](10-ui-config-launch.md) | U12–U15 | P6, P8 |
| 交付验收 | [11-acceptance.md](11-acceptance.md) | A1–A10 | 全部 |

任务 ID 全局唯一，跨文档引用时使用该 ID。

---

## 8. 每阶段完成定义（DoD）

1. 新增代码有对应测试；纯 UI 布局无法自动测试的部分在验收文档中列出手工步骤。
2. `cargo fmt --all --check` 通过。
3. `cargo clippy --workspace --all-targets -- -D warnings` 通过。
4. `cargo test --workspace` 通过（真实 Wine 测试标记 `#[ignore]`，单独手工执行）。
5. 本阶段没有引入新的顶层 crate，没有违反依赖方向。
6. `CHANGELOG.md` 记录：阶段、完成内容、与 spec 的偏差（如有）、验证命令与结果。
7. 提交信息格式：`feat(<crate>): <what>` / `test(<crate>): <what>` / `chore(workspace): <what>`。

---

## 9. 执行方式

- 每份 spec 的步骤是 TDD 顺序：先写失败测试 → 运行确认失败 → 最小实现 → 运行通过 → 提交。
- 不要一次性把一份 spec 的所有任务塞进一个 commit；每个任务至少一个 commit。
- 发现 spec 与实际 API 不符时：做最小适配，在 `CHANGELOG.md` 和对应 spec 末尾的“实施记录”中注明；不要静默改架构。
- 任何需要新增依赖、改变 crate 边界、引入联网功能的情况，先停下来向用户确认。
