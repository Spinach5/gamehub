# gameHub 实施规格总览（v2）

> **For agentic workers:** 本目录是可直接执行的实施规格。按 `docs/plan.md` 的阶段顺序逐份实施；每份 spec 内的任务按顺序执行，使用 `superpowers:subagent-driven-development` 或 `superpowers:executing-plans`，每个任务结束后提交一次。步骤使用 `- [ ]` 复选框跟踪。
>
> **现状说明（2026-10-31，v0.12.0）：** 本目录的 P0–P9 任务步骤与 P7–P9 的验收记录是历史实施记录，保持原样不改。凡是与当前代码冲突的「固定契约 / 架构 / 流程」描述已在对应文档中就地修正，并在每篇末尾或开头补「当前实现（v0.12.0）」小节；新增特性（平台、封面、时长、winetricks、i18n、主题、字体、停止、发布流程等）也登记在那里。当前代码是唯一事实来源。

**Goal:** 在 Linux（X11/Wayland）上交付一个完全离线可用的游戏启动器：导入本地 Windows exe 或 Linux 原生可执行文件、管理 Wine/Proton runner、创建 prefix、注入用户提供的 DXVK、配置显示与 gamescope、启动与停止游戏、记录游玩时长。

**Architecture:** Cargo workspace + 8 个 crate。`launcher-core` 只放纯类型、枚举映射、路径计算与日志初始化；`storage` 负责 SQLite；`runner-wine`、`prefix`、`display`、`launch` 各自封装一个职责；`application` 聚合这些能力，提供用例级 API 与统一错误；`ui` 是唯一的 Iced 二进制入口，只做状态机与视图。依赖方向单向，禁止环。

**Tech Stack:** Rust stable / edition 2024（MSRV 1.85）、Iced 0.13.1（`tokio`、`advanced`、`image-without-codecs`、`svg`）、tokio 1.54、rusqlite 0.41（bundled）、rfd 0.17.2、serde/serde_json、thiserror 2、tracing、uuid v4、chrono（仅用于时间显示）、image 0.24（封面解码）、nix 0.29（信号）、zbus 4.4（GNOME Mutter 主屏切换）。workspace 版本 0.12.1。

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

实现：本地 exe / Linux 原生可执行文件导入、库管理、Wine/Proton 探测与导入、每游戏或共享 prefix、DXVK 与 DXVK-NVAPI 文件注入、winetricks 组件安装（调用系统 winetricks）、X11 输出选择、Wayland gamescope、环境变量与结构化 DLL override、esync/fsync/MangoHud 调优、游戏启动/停止与日志、手动封面、游玩时长、简体中文/English 界面、浅色/深色主题、界面字体切换、全离线运行。

明确不做（原始边界仍有效）：商店/下载/云同步/成就/封面抓取/mod 管理/Steam 客户端集成/Windows 或 macOS；不实现 Wine/Proton 本身；不修改游戏二进制；不自动下载 DXVK、DXVK-NVAPI、MangoHud、winetricks 或字体。封面由用户手动选择本地图片，不联网抓取。

后续版本已放宽或补充的边界：

| 原边界 | 当前实现 |
|---|---|
| 只导入 exe | 增加 Linux 原生平台；原生游戏不经过 Wine/prefix/DLL，但仍可使用 gamescope、MangoHud、显示选择、环境变量与启动参数 |
| 「不做封面」 | 支持为每个游戏手动选择本地图片封面，复制到 `<data_root>/covers/`；没有在线封面抓取 |
| 未提时长 | 记录 `last_played_at`（本次启动时间）与累计 `playtime_seconds` |
| 未提停止 | 卡片与详情页可停止运行中的游戏：先杀进程组，Wine/Proton 再通过 `wineserver -k` 清理 prefix 内进程 |
| 仅中文 | 内建 zh-CN / en-US 两套目录，可跟随系统或手动切换 |
| 固定深色 | 浅色（默认）/ 深色两套主题，可切换并持久化 |
| 固定字体 | 通过 `fc-list` 扫描本机字体家族，可在设置中切换界面字体 |

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
├── packaging/                  # nfpm 打包配置（deb/rpm/pacman）
├── .github/workflows/          # release.yml：多架构构建与发布
├── docs/
│   ├── plan.md                 # v2 入口索引
│   ├── acceptance.md           # v2 验收证据
│   ├── i18n.md                 # 语言目录维护指南
│   ├── specs/                  # 本目录（P0–P9 历史计划 + P12 设计）
│   ├── superpowers/specs/      # v0.4–v0.8 特性设计（主题/封面、i18n、字体）
│   └── archive/plan-v1.md
└── crates/
    ├── launcher-core/
    │   └── src/{lib.rs,types.rs,paths.rs,text.rs,logging.rs,error.rs}
    ├── storage/
    │   └── src/{lib.rs,schema.rs,mapping.rs,error.rs,repos/{mod,games,runners,settings}.rs}
    ├── runner-wine/
    │   └── src/{lib.rs,detect.rs,context.rs,status.rs,stop.rs,error.rs}
    ├── prefix/
    │   └── src/{lib.rs,status.rs,initialize.rs,dxvk.rs,nvapi.rs,winetricks.rs,error.rs}
    ├── display/
    │   └── src/{lib.rs,session.rs,xrandr.rs,gamescope.rs,primary.rs,error.rs}
    ├── launch/
    │   └── src/{lib.rs,resolve.rs,env.rs,plan.rs,spawn.rs,error.rs}
    ├── application/
    │   └── src/{lib.rs,error.rs,service.rs,data.rs,editor.rs,games.rs,runners.rs,prefixes.rs,components.rs,launching.rs,delete.rs,settings.rs}
    └── ui/
        ├── locales/{zh-CN.json,en-US.json}
        └── src/
            ├── main.rs
            ├── lib.rs
            ├── app/{mod.rs,message.rs,update.rs,view.rs}
            ├── dialogs.rs
            ├── fonts.rs
            ├── i18n/{mod.rs,errors.rs}
            ├── styles.rs
            ├── theme.rs
            ├── widgets/{mod.rs,badges.rs,choices.rs,controls.rs,cover.rs,layout.rs,modal.rs}
            └── views/{mod.rs,library.rs,game_detail.rs,logs.rs,runners.rs,settings.rs}
```

文件职责边界：

- `launcher-core`：类型、serde 契约、枚举 ↔ 字符串映射、`AppPaths` 计算、UTF-8 截断、`init_logging`。**不依赖** tokio/rusqlite/iced，也不执行文件系统命令（日志目录创建是唯一例外）。
- `storage`：SQLite 连接、迁移、行映射、CRUD、引用计数，按 `games`/`runners`/`settings` 拆分在 `repos/`。仅它可以直接依赖 rusqlite。
- `runner-wine`：runner 探测、Proton 目录校验、`WineContext` 环境与命令行构造、`spawn`、prefix 内进程检测与 `wineserver -k` 停止。
- `prefix`：prefix 路径解析、状态/arch 检测、wineboot 初始化、DXVK / DXVK-NVAPI 复制与注册表写入、winetricks 命令构造与已安装 verb 读取、prefix 删除（调用方负责引用检查）。
- `display`：会话检测、xrandr 解析与输出扫描、Mutter DisplayConfig 主屏切换、gamescope / MangoHud 查找与参数构造。无 UI、无 DB。
- `launch`：runner 解析、启动前校验、环境合成、启动计划、spawn（可带日志文件）。
- `application`：用例编排（加载、保存、草稿校验、探测、prefix、DXVK、组件安装、删除、启动/停止、设置），持有 `AppService` 连接与运行中进程表，定义统一的 `AppError`。UI 不再直接依赖底层 crate（只依赖 `application` 与 `launcher-core` 的再导出类型）。
- `ui`：Iced 状态机、视图、主题/样式、i18n、字体扫描、rfd 对话框；只把 `AppService` 的异步结果映射成 `Message`。所有 DB/文件/子进程重活都在 `application` 的 `spawn_blocking` 或 tokio task 中。

依赖方向（禁止反向）：

```
ui ──> application, launcher-core
application ──> launch, prefix, display, runner-wine, storage, launcher-core
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
version = "0.12.1"
edition = "2024"
rust-version = "1.85"

[workspace.dependencies]
launcher-core = { path = "crates/launcher-core" }
application = { path = "crates/application" }
storage = { path = "crates/storage" }
runner-wine = { path = "crates/runner-wine" }
prefix = { path = "crates/prefix" }
display = { path = "crates/display" }
launch = { path = "crates/launch" }

iced = { version = "0.13.1", features = ["tokio", "advanced", "image-without-codecs"] }
image = { version = "0.24", default-features = false, features = ["png", "jpeg", "webp", "bmp"] }
tokio = { version = "1.54", features = ["rt-multi-thread", "macros", "process", "sync", "time", "fs", "io-util"] }
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
nix = { version = "0.29", features = ["signal"] }
zbus = "4.4"
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
| display | launcher-core, tracing, zbus |
| launch | launcher-core, storage, prefix, display, runner-wine, tokio(process), thiserror, tracing, uuid |
| application | launch, prefix, display, runner-wine, storage, launcher-core, tokio, thiserror, tracing, uuid, nix |
| ui | iced, image, tokio, rfd, application, launcher-core, tracing, tracing-subscriber, tracing-appender, serde_json, uuid, chrono |

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
| `enable_esync` | `"1"` / `"0"`（缺省视为开启） |
| `enable_fsync` | `"1"` / `"0"`（缺省视为开启） |
| `enable_mangohud` | `"1"` / `"0"` |
| `auto_install_dxvk_nvapi` | `"1"` / `"0"` |
| `dxvk_nvapi_source` | 绝对路径字符串 |
| `ui_theme` | `"light"` / `"dark"` |
| `ui_language` | `"auto"` 或语言 code（`"zh-CN"` / `"en-US"`） |
| `ui_font` | 字体家族名；缺失表示默认 `Noto Sans CJK SC` |

### 5.5 离线

- 运行期不得发起任何网络请求。首期没有任何联网代码。
- `rfd` 使用 xdg-portal（D-Bus）不是网络；`xrandr`、`wine`、`gamescope` 是本地子进程。
- 验收用 `strace` 时只关注 `AF_INET`/`AF_INET6`；`AF_UNIX`（D-Bus/X11）与 `AF_NETLINK`（接口枚举）是正常现象。

### 5.6 UI 语言与字体

- 界面文案通过内建 i18n 目录提供：`crates/ui/locales/zh-CN.json` 与 `en-US.json`，编译期 `include_str!` 嵌入，无第三方依赖。默认跟随系统（`LANGUAGE → LC_ALL → LC_MESSAGES → LANG`），可在设置中固定为简体中文或 English。
- 代码标识符用英文；新文案必须同时加入两种目录，测试会校验 key 集合、占位符集合一致，且源码中使用的字面量 key 都存在。
- 默认字体 `Noto Sans CJK SC`；设置页可扫描 `fc-list` 并切换为其他已安装家族。所选家族没有 Bold 面时所有字重强制 Regular，避免 iced 回退到无中文字形的面导致文字不可见（见 v0.4.1 修复）。不打包字体文件。
- 日志与 winetricks 输出保持等宽字体，不随界面字体变化。

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
| P12 卡片操作与兼容层增强 | [12-game-card-actions-settings.md](12-game-card-actions-settings.md) | — | P9 |

任务 ID 全局唯一，跨文档引用时使用该 ID。

P12 之后（v0.2.0–v0.12.0）的特性没有新增 P 级文档，设计与实施记录在：

| 版本 | 内容 | 设计/记录 |
|---|---|---|
| v0.2.0 | 游戏卡片操作、启动日志、Esync/Fsync/MangoHud、DXVK-NVAPI | [12-game-card-actions-settings.md](12-game-card-actions-settings.md)、CHANGELOG |
| v0.3.0 | 停止游戏、显示器扫描、Wayland 主屏切换、`wineserver -k` | CHANGELOG、第 6 篇「当前实现」 |
| v0.4.0 / v0.4.1 | 浅色/深色主题、侧边栏、封面卡片、CJK 字重修复 | [UI 现代化设计](../superpowers/specs/2026-10-27-ui-modernization-design.md) |
| v0.5.0 | 简体中文 / English 界面 | [i18n 设计](../superpowers/specs/2026-10-28-i18n-design.md)、[docs/i18n.md](../i18n.md) |
| v0.6.0 | 游玩时长、启动会话保活、草稿逻辑迁入 `application::editor` | CHANGELOG |
| v0.7.0 | 系统 winetricks 组件安装 | CHANGELOG |
| v0.8.0 | 界面字体切换 | [字体设置设计](../superpowers/specs/2026-10-29-ui-font-setting-design.md) |
| v0.9.0 | Linux 原生平台支持 | 第 2/3/7/10/12 篇「当前实现」 |
| v0.10.0 | DLL Override 目录扫描与勾选 | [设计](../superpowers/specs/2026-10-31-dll-scan-picker-design.md)、第 10 篇「当前实现」 |
| v0.11.0 | 侧边栏版本链接跳转 GitHub | CHANGELOG、第 8 篇「当前实现」 |
| v0.12.0 | 侧边栏改用 SVG 品牌图标 | CHANGELOG、第 8 篇「当前实现」 |

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

---

## 10. 当前实现（v0.12.0，2026-10-31）

本节是 P0–P12 之后所有已交付能力的索引；各篇文档的「当前实现」小节给出该领域的具体契约。

### 10.1 领域与存储

- `Game.platform: Platform`（`Windows` / `Linux`，默认 Windows）、`Game.cover_path: Option<PathBuf>`、`Game.playtime_seconds: i64` 已进入固定契约。SQLite schema 为 v4，迁移链 v1→v2（cover）→v3（playtime）→v4（platform），已有库原地升级。
- 草稿与校验位于 `application::editor`（`GameDraft`、`DraftAction`、`DraftError`），UI 不再拥有校验逻辑。
- 设置键扩展到兼容层调优、DXVK-NVAPI、主题、语言、字体（见 5.4）。

### 10.2 启动与运行

- `LaunchRequest.runner` 为 `Option<Runner>`：Linux 原生游戏为 `None`，直接执行可执行文件；Windows 游戏必须有 runner。原生游戏跳过 runner 校验、prefix 状态/arch、DLL override 与全部 Wine/Proton 环境，但保留显示选择、gamescope、MangoHud、全局/游戏环境变量与启动参数。
- `LaunchTuning { enable_esync, enable_fsync, enable_mangohud, mangohud_path, enable_dxvk_nvapi }` 在环境合成阶段注入；MangoHud 在非 gamescope 路径作为包装器，gamescope 路径使用 `--mangoapp`。
- 启动可写 `logs/games/<game-id>.log`（新日志覆盖当前文件，上一次保留为 `.old`）；`application::launching` 以 `LaunchEvent::{Started, Finished, Failed}` 驱动 UI，`Started` 立即写 `last_played_at`，`Finished` 累加 `playtime_seconds`。
- 会话保活：只有启动器进程组与 prefix 内进程（`/proc/*/environ` 的 `WINEPREFIX`，排除 `wineserver`）都为空才结束；原生游戏只看进程组。
- 停止：Wine/Proton 先调用对应 runner 的 `wineserver -k` 结束 prefix 内进程，
  再终止启动器进程组兜底；原生游戏只终止进程组。
- 主屏切换：非 gamescope 且目标显示器有 X11 呈现时，Wayland 用 Mutter `ApplyMonitorsConfig`、X11 用 `xrandr --output ... --primary` 临时切换，退出后恢复。

### 10.3 Prefix 与组件

- DXVK 安装保持 x64/x32 → system32/syswow64 映射并写注册表；新增 DXVK-NVAPI 安装（`nvapi64/nvofapi64` 与 `nvapi/nvofapi`，注册表 `native,builtin`，marker `.gamehub-dxvk-nvapi`）。
- 调用系统 `winetricks`（`PATH` 查找，不下载）：verb 白名单校验、从 prefix 的 `winetricks.log` 读取已安装项、每前缀一次一个 verb、输出流式写入 `logs/winetricks/<game-id>.log`、可取消（进程组 SIGTERM→SIGKILL）。

### 10.4 界面

- 侧边栏 + 卡片网格布局，浅色默认 / 深色可切换；封面支持本地图片（复制到 `covers/`）或默认渐变。
- 新建游戏先选平台再选文件：Windows 只显示 `.exe`，Linux 显示全部文件；详情页对原生游戏隐藏架构、Runner、prefix、DXVK、winetricks、DLL override 等 Wine 专属项，但保留并保存其值，切回 Windows 时恢复生效。
- 游戏卡片提供启动/停止、设置、日志；详情页提供封面、游玩时长、摘要卡；日志页只保留最近一次启动输出。
- i18n、主题、语言、字体、兼容层调优、DXVK-NVAPI 目录均在设置页；设置即时保存（主题/语言/字体）或点保存提交（其余）。
- DLL Override 支持「从目录添加」：递归扫描游戏目录、按小写基名去重、在模态框内搜索/排序/全选/反选后批量追加草稿；已有 Override 标「已添加」并禁用；上次扫描目录按游戏记住。
- 侧边栏底部显示会话类型、默认 Runner 与 workspace 版本号；版本号为链接，点击经 `xdg-open` 打开项目 GitHub 主页，悬停显示提示。
- 侧边栏顶部使用 `packaging/gamehub.svg` 品牌图标（`include_bytes!` 内嵌，iced `svg` 渲染），替换原渐变方块；浅色与深色下均保留图标原色。

### 10.5 发布

- `Cargo.toml` workspace 版本 + `CHANGELOG.md` + annotated tag 是发布三步；`.github/workflows/release.yml` 在 Ubuntu 22.04 容器内为 x86_64/aarch64 构建二进制、`.deb`、`.rpm`、pacman 包与 AppImage。发布只在明确要求时执行。
- 打 tag 后先校验 tag、`Cargo.toml` 与 `CHANGELOG.md` 版本一致；发布页由 `.github/scripts/render_release_notes.py` 从 CHANGELOG 生成，包含更新摘要、按包格式与架构分组的下载地址、安装命令、系统要求与构建信息。发布过程生成 `gamehub_<version>_SHA256SUMS` 与 SLSA 来源证明，并按草稿→上传全部产物→正式发布执行。
