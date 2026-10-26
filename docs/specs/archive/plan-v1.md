下面是可直接交给 agent 执行的完整实施方案。文档自包含，所有架构决策已固定，agent 不需要再做设计选择，只需按阶段实现并满足每阶段的验收条件。

---

# Linux 离线游戏启动器 — Rust + Iced 完整实施方案

## 0. 给 Agent 的总体约束

- **技术栈锁定**：Rust stable、Iced 0.13.x、tokio、rusqlite、rfd、serde。不得引入 Electron/Tauri/WebView/Node.js/任何 Web 技术。
- **架构锁定**：按第 3 节 crate 结构实施，不得合并或新增顶层 crate，除非经确认。
- **产品边界**：启动器不实现 Wine/Proton，不修改游戏二进制，不依赖 Steam 客户端。所有兼容层调用均为子进程。
- **离线优先**：除用户显式触发的可选功能外，任何代码路径不得发起网络请求。首期不实现任何联网功能。
- **每阶段完成定义**：`cargo build --workspace` 通过、`cargo clippy` 无 warning 级问题、`cargo test` 通过、满足该阶段"验收"条目。未满足不得进入下一阶段。
- **平台**：仅 Linux（X11 与 Wayland）。不写 Windows/macOS 分支。
- **错误处理**：库层返回 `Result<T, E>`，`E` 为 `thiserror` 定义的枚举；UI 层捕获后转 `Message`，不得 `unwrap()` 系统调用结果。

---

## 1. 项目目标与功能范围

一个 Linux 桌面游戏启动器，用于导入并启动 Windows 游戏，提供：

1. 游戏导入（本地 `.exe`）与库管理
2. Wine / Proton（含 GE-Proton）作为兼容层，支持系统 Wine 探测与导入 Proton 目录
3. 每个游戏独立或共享的 Wine prefix，支持创建、初始化、DXVK/VKD3D 注入（用户提供 DLL 目录）
4. 显示器配置：X11 输出选择、Wayland 下 gamescope 嵌套
5. 环境变量与结构化 DLL override 配置
6. 全离线可用

**明确不做**：游戏商店、下载、云同步、成就、封面在线抓取、mod 管理、Steam 集成、Windows 端。

---

## 2. 技术栈（不可协商）

| 项 | 选择 | 说明 |
|---|---|---|
| GUI | **Iced 0.13.x** | 使用其功能式 `iced::application` API；若编译时发现版本 API 差异，以 0.13 文档为准调整，不得换成 egui/GTK/Qt |
| 异步 | tokio（full 或 process + rt-multi-thread + sync） | 所有阻塞/长时操作在 tokio task 或 `spawn_blocking` 中执行 |
| DB | rusqlite（bundled feature） | SQLite 编入二进制，无系统依赖 |
| 文件对话框 | rfd | 在 `spawn_blocking` 中调用 |
| 序列化 | serde + serde_json | 复杂配置存 JSON 列 |
| 错误 | thiserror | 每 crate 一个错误枚举 |
| 日志 | tracing + tracing-subscriber | 输出到 stderr 与 `~/.local/share/<app>/logs/` |
| UUID | uuid（v4） | 实体 ID |
| 时间 | std::time 或 chrono | 时间戳用 i64 unix 秒即可 |

推荐 Cargo profile：

```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"
```

---

## 3. 仓库结构

```
game-launcher/
├── Cargo.toml               # workspace
├── crates/
│   ├── launcher-core/       # 纯类型与错误，无 I/O
│   ├── storage/             # SQLite 持久化
│   ├── runner-wine/         # Wine/Proton 发现与调用封装
│   ├── prefix/              # prefix 生命周期、DXVK 注入
│   ├── display/             # 会话检测、显示器枚举、gamescope 参数
│   ├── launch/              # 启动管线（依赖以上所有）
│   └── ui/                  # Iced 应用，二进制入口
└── assets/                  # 图标等（可选）
```

依赖方向（禁止反向或环）：

```
ui → launch → prefix, display, runner-wine, storage, launcher-core
prefix → runner-wine → launcher-core
display → launcher-core
storage → launcher-core
```

`launcher-core` 只依赖 serde/uuid/thiserror，**不得**依赖 rusqlite/tokio/iced。

---

## 4. `launcher-core`：核心数据类型

`crates/launcher-core/src/lib.rs` 导出以下类型（字段名固定，序列化格式固定）。

```rust
use std::collections::BTreeMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Arch { Win32, Win64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunnerKind { Wine, Proton }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunnerSource {
    System,                              // 从 PATH/常见路径探测
    Imported { original_path: PathBuf }, // 用户指定的 Proton 根目录
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runner {
    pub id: Uuid,
    pub kind: RunnerKind,
    pub name: String,
    pub binary_path: PathBuf,   // Wine: wine 可执行文件；Proton: 根目录下的 proton 脚本
    pub root_dir: PathBuf,      // Wine: binary 所在目录；Proton: 根目录
    pub arch: Arch,
    pub source: RunnerSource,
    pub verified: bool,
    pub version_string: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OverrideMode { Disabled, Native, Builtin, NativeBuiltin }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DllOverride {
    pub dll_name: String,   // 不含 .dll
    pub mode: OverrideMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisplayBackend { Auto, X11, Wayland }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GamescopeMode { Off, Auto, Forced }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub backend: DisplayBackend,
    pub monitor: Option<String>,           // X11: xrandr 输出名；Wayland: 忽略或 gamescope -O
    pub resolution: Option<(u32, u32)>,
    pub refresh_rate: Option<u32>,
    pub fullscreen: bool,
    pub gamescope: GamescopeMode,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            backend: DisplayBackend::Auto,
            monitor: None,
            resolution: None,
            refresh_rate: None,
            fullscreen: true,
            gamescope: GamescopeMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: Uuid,
    pub name: String,
    pub executable_path: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub runner_id: Option<Uuid>,           // None = 使用全局默认
    pub prefix_path: Option<PathBuf>,      // None = 使用游戏专属默认 prefix
    pub arch: Arch,
    pub display: DisplayConfig,
    pub env_overrides: BTreeMap<String, String>,
    pub dll_overrides: Vec<DllOverride>,
    pub args: Vec<String>,
    pub created_at: i64,
    pub last_played_at: Option<i64>,
}
```

**关键约束**：`dll_overrides` 必须结构化存储；环境变量 `WINEDLLOVERRIDES` 由 `launch` crate 在启动时从 `dll_overrides` **确定性拼接**，不接受用户在 `env_overrides` 中手动设置该键（启动前检测并报错）。

---

## 5. `storage`：SQLite 持久化

### 5.1 Schema（首版迁移 v1）

```sql
CREATE TABLE games (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    executable_path TEXT NOT NULL,
    working_dir TEXT,
    runner_id TEXT,
    prefix_path TEXT,
    arch TEXT NOT NULL,              -- 'win32' | 'win64'
    display_json TEXT NOT NULL,      -- serde_json of DisplayConfig
    env_overrides_json TEXT NOT NULL,
    dll_overrides_json TEXT NOT NULL,
    args_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_played_at INTEGER
);

CREATE TABLE runners (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,              -- 'wine' | 'proton'
    name TEXT NOT NULL,
    binary_path TEXT NOT NULL,
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

CREATE TABLE schema_version (version INTEGER NOT NULL);
```

### 5.2 API（`crates/storage/src/lib.rs`）

```rust
pub struct Db { conn: rusqlite::Connection }

impl Db {
    pub fn open(path: &Path) -> Result<Db, StorageError>;   // 建表 + 迁移
    pub fn open_in_memory() -> Result<Db, StorageError>;    // 测试用

    // games
    pub fn insert_game(&self, g: &Game) -> Result<(), StorageError>;
    pub fn update_game(&self, g: &Game) -> Result<(), StorageError>;
    pub fn delete_game(&self, id: Uuid) -> Result<(), StorageError>;
    pub fn get_game(&self, id: Uuid) -> Result<Option<Game>, StorageError>;
    pub fn list_games(&self) -> Result<Vec<Game>, StorageError>;
    pub fn touch_last_played(&self, id: Uuid, ts: i64) -> Result<(), StorageError>;

    // runners
    pub fn insert_runner(&self, r: &Runner) -> Result<(), StorageError>;
    pub fn update_runner(&self, r: &Runner) -> Result<(), StorageError>;
    pub fn delete_runner(&self, id: Uuid) -> Result<(), StorageError>;
    pub fn get_runner(&self, id: Uuid) -> Result<Option<Runner>, StorageError>;
    pub fn list_runners(&self) -> Result<Vec<Runner>, StorageError>;

    // settings（字符串 KV，JSON 或原始值）
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError>;
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError>;
}
```

**验收**：为每个方法写 round-trip 单元测试（insert → get/list 断言相等）；迁移用 `schema_version` 表，首次打开自动建 v1。

---

## 6. `runner-wine`：兼容层发现与调用

### 6.1 系统 Wine 探测

```rust
pub fn detect_system_wine() -> Vec<Runner>;
```

逻辑：

1. 遍历 `$PATH` 查找 `wine`，若存在执行 `wine --version`，成功则生成 `RunnerKind::Wine` 的 `Runner`（`source: System`）。
2. 额外探测常见路径：`/usr/bin/wine`、`/usr/bin/wine-stable`、`/usr/bin/wine-development`（去重）。
3. `version_string` 保存 `wine --version` 输出（trim）。
4. 探测失败不 panic，返回空 vec。

### 6.2 导入 Proton 目录

```rust
pub fn validate_proton_dir(root: &Path) -> Result<Runner, RunnerError>;
```

校验：

1. `root/proton` 存在且可执行（Unix `PermissionsExt` 检查执行位）。
2. 读取 `root/version`（若存在）作为 `version_string`；否则用目录名。
3. `binary_path = root/proton`，`root_dir = root`，`arch = Win64`（Proton 一律按 win64 记录，实际由 prefix 决定）。
4. 返回未持久化的 `Runner`，由上层写入 storage。

### 6.3 调用封装

```rust
pub struct WineContext {
    pub runner: Runner,
    pub prefix: PathBuf,
    pub arch: Arch,
}

impl WineContext {
    /// 返回将要设置的环境变量（不含游戏级覆盖）
    pub fn base_env(&self) -> BTreeMap<String, String>;

    /// 构造完整命令行（不 spawn）：参数为要传给 windows 程序的 args
    pub fn build_command(&self, exe: &Path, args: &[String]) -> (PathBuf, Vec<String>);

    /// 便捷 spawn（供 prefix 模块与 launch 模块使用）
    pub async fn spawn(&self, exe: &Path, args: &[String], extra_env: &BTreeMap<String,String>)
        -> Result<tokio::process::Child, RunnerError>;
}
```

`base_env` 规则：

- Wine：
  - `WINEPREFIX = prefix`
  - `WINEARCH = "win64" | "win32"`
  - `WINEDEBUG = "-all"`（可被游戏级覆盖）
- Proton：
  - `STEAM_COMPAT_DATA_PATH = prefix`（注意：Proton 期望该路径下存在 `pfx` 子目录；本方案规定 prefix 目录本身即 `STEAM_COMPAT_DATA_PATH`，即 `pfx` 位于 `prefix/pfx`）
  - `STEAM_COMPAT_CLIENT_INSTALL_PATH = 一个稳定的空目录`（首次使用时创建，例如 `~/.local/share/<app>/steam-compat-client`；仅需存在）
  - `PROTONPATH = runner.root_dir`

`build_command` 规则：

- Wine：`(wine_binary, [exe, args...])`
- Proton：`(root_dir/proton, ["run", exe, args...])`

**约束**：`WineContext` 不做任何 I/O 校验（由 prefix/launch 层保证）。

**验收**：单元测试覆盖 `base_env` 和 `build_command` 对 Wine/Proton 两种 kind 的输出；`validate_proton_dir` 用临时目录构造伪 proton 脚本测试成功与失败路径。

---

## 7. `prefix`：Prefix 生命周期

```rust
pub struct PrefixPlan {
    pub path: PathBuf,
    pub arch: Arch,
    pub runner: Runner,
}

impl PrefixPlan {
    /// 仅计算路径与属性，不触盘。默认路径：
    /// ~/.local/share/<app>/prefixes/<game_id>
    pub fn default_for(game_id: Uuid, runner: Runner, arch: Arch) -> Self;

    /// 创建目录（若不存在）并执行 wineboot 初始化。
    /// 已初始化（存在 system.reg）则直接返回 Ok(false)。
    /// 返回 true 表示本次执行了初始化。
    pub async fn initialize(&self) -> Result<bool, PrefixError>;
}
```

`initialize` 实现要点：

1. `create_dir_all(path)`。
2. 若 `path/system.reg` 已存在 → 返回 `Ok(false)`。
3. 构造 `WineContext`，spawn：
   - Wine：`wineboot -u`
   - Proton：`proton run wineboot -u`（通过 `WineContext::build_command`，exe 传 `wineboot` 不可行时，改为直接构造 `proton run wineboot -u`；实现时以 Proton 脚本接受 `run <cmd>` 为准，`exe` 位置传 `wineboot`，`args` 传 `["-u"]`）
4. 等待退出；非零退出码 → `Err(PrefixError::WinebootFailed { code, stderr })`（捕获 stderr 便于 UI 展示，截断到 4KB）。
5. 成功后可选：若指定了 DXVK 目录，调用 `install_dxvk`。

```rust
pub struct DxvkSource {
    pub root: PathBuf, // 用户提供的 DXVK 发布目录，含 x64/ 与 x32/
}

pub async fn install_dxvk(prefix: &Path, arch: Arch, src: &DxvkSource)
    -> Result<(), PrefixError>;
```

`install_dxvk` 规则（离线、不下载）：

1. 校验 `src.root/x64/d3d11.dll`、`dxgi.dll` 存在（`d3d10core.dll`、`d3d9.dll` 可选）。
2. 复制到 prefix 的 `drive_c/windows/system32`（64 位）与 `syswow64`（32 位组件，仅当 arch=Win64 且 x32 存在）。
3. 在 `user.reg` 中写入对应 DLL override（`d3d11`,`dxgi` 等设为 `n,b`）。**实现方式**：用 `wine reg add` 命令而非手改 reg 文件，避免格式错误：
   `wine reg add "HKCU\\Software\\Wine\\DllOverrides" /v d3d11 /d "native,builtin" /f`
4. `PrefixError::DxvkSourceInvalid` 用于目录不符合预期。

**验收**：
- 用系统 Wine（若测试环境有）执行 `default_for` + `initialize`，断言 `system.reg` 存在；无 Wine 环境则该测试 `#[ignore]`。
- `install_dxvk` 用伪造的 DLL 文件测试复制与 `reg add` 命令构造（不实际执行 reg add，或 mock 掉 spawn）。

---

## 8. `display`：会话检测、显示器、gamescope

### 8.1 会话检测

```rust
pub fn detect_session() -> SessionType;  // enum SessionType { X11, Wayland, Unknown }
```

读 `XDG_SESSION_TYPE`：`"x11"` → X11，`"wayland"` → Wayland，否则 Unknown。

### 8.2 X11 显示器枚举

```rust
pub struct X11Output {
    pub name: String,
    pub connected: bool,
    pub active: bool,
    pub current_mode: Option<(u32, u32, Option<u32>)>, // w,h,refresh
}

pub fn list_x11_outputs() -> Vec<X11Output>;
```

实现：`std::process::Command::new("xrandr").arg("--query")`，解析输出。解析规则：

- 行首为输出名，含 ` connected ` 且非 `disconnected` → connected。
- 输出名后紧跟的 `1920x1080+0+0` 等表示 active 且当前模式；`(normal ...)` 后提取刷新率。
- 解析失败的行忽略，不 panic。

### 8.3 gamescope 参数生成

```rust
pub fn gamescope_available() -> bool; // which gamescope，缓存到 OnceLock

pub fn build_gamescope_args(cfg: &DisplayConfig, session: SessionType)
    -> Vec<String>;
```

生成规则：

- 若 `cfg.gamescope == Off` → 返回空 vec。
- 若 `Auto` 且 `session == Wayland` 且 `gamescope_available()` → 生成参数；否则空 vec。
- 若 `Forced` → 无论 session 都生成（不可用则让上层报错）。
- 参数：
  - `-W <w> -H <h>`（有 resolution）
  - `-r <rate>`（有 refresh）
  - `-f`（fullscreen）
  - `-O <monitor>`（有 monitor 且 session 为 Wayland 时）
  - `--backend wayland`（session=Wayland）或 `--backend x11`（session=X11 且 forced）
  - `--` 作为分隔符追加到末尾

`launch` 模块会拼接：`[gamescope, ...gamescope_args, --, runner_cmd...]`。注意 `build_gamescope_args` **不包含** `--` 之后的游戏命令，只返回 gamescope 自身的参数并以 `--` 结尾。

### 8.4 X11 显示器绑定（非 gamescope 路径）

当 session=X11 且不走 gamescope 时，在 `launch` 的环境变量中追加：

- `__GL_SYNC_DISPLAY_DEVICE = <monitor_name>`（若 monitor 指定，NVIDIA 专有；其他驱动忽略该变量无害）

此逻辑写在 `launch` 的环境合成函数中，`display` 只提供 `monitor` 值。

**验收**：`list_x11_outputs` 用样例 `xrandr --query` 文本（fixture）单测解析；`build_gamescope_args` 对 Off/Auto/Forced × X11/Wayland 的真值表单测。

---

## 9. `launch`：启动管线

### 9.1 输入

```rust
pub struct LaunchRequest {
    pub game: Game,
    pub runner: Runner,        // 已解析（游戏级或全局默认）
    pub session: SessionType,
}
```

### 9.2 环境变量合成（优先级从低到高）

```rust
pub fn build_env(req: &LaunchRequest) -> BTreeMap<String, String>;
```

顺序：

1. `std::env::vars()` 全量继承。
2. 删除 `WINEDLLOVERRIDES`（确保由结构化 override 决定）。
3. 合入 `WineContext::base_env()`（`WINEPREFIX`/`WINEARCH` 或 Proton 三件套）。
4. 合入全局设置中的环境变量（key `global_env`，值为 JSON object）。
5. 合入游戏 `env_overrides`。
6. 若 session=X11 且 game.display.monitor 有值且未走 gamescope → 注入 `__GL_SYNC_DISPLAY_DEVICE`。
7. 若 `game.env_overrides` 含 `WINEDLLOVERRIDES` → 返回 `Err(LaunchError::DllOverrideConflict)`（禁止手动设置该键）。
8. 若 `game.dll_overrides` 非空 → 拼接并设置 `WINEDLLOVERRIDES`。

`build_dll_overrides`：

```rust
pub fn build_dll_overrides(o: &[DllOverride]) -> String;
```

- `Disabled` 与 `Native` → `n`；`Builtin` → `b`；`NativeBuiltin` → `n,b`。
- 多个以 `;` 连接：`winmm=n,b;steam_api64=n`。

### 9.3 命令构造与 spawn

```rust
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

pub fn build_plan(req: &LaunchRequest) -> Result<LaunchPlan, LaunchError>;

pub async fn spawn_plan(plan: &LaunchPlan) -> Result<LaunchHandle, LaunchError>;

pub struct LaunchHandle {
    pub child: tokio::process::Child,
    pub game_id: Uuid,
    pub started_at: i64,
}

impl LaunchHandle {
    /// 等待子进程退出，返回 exit status
    pub async fn wait(mut self) -> std::io::Result<std::process::ExitStatus>;
}
```

`build_plan` 流程：

1. 若 gamescope 生效 → `program = "gamescope"`，`args = build_gamescope_args(...) ++ ["--"] ++ runner_cmd.program ++ runner_cmd.args`。
2. 否则 `program/args` 直接用 `WineContext::build_command(game.executable_path, game.args)`。
3. `env` = `build_env(req)`。
4. `cwd` = `game.working_dir` 或 `exe` 的父目录。

`spawn_plan`：

- `tokio::process::Command::new(program).args(args).envs(env)`，若 `cwd` 有值 `.current_dir(cwd)`。
- stdout/stderr：继承 stderr 到父进程（便于日志）；stdout 重定向到 `Stdio::null()`（避免污染）。
- `WineContext` 的 `spawn` 方法可在此复用或直接构造 Command。

**验收**：`build_env` 与 `build_dll_overrides` 单测覆盖优先级与冲突检测；`build_plan` 单测 gamescope 开/关两条路径。

---

## 10. `ui`：Iced 应用

### 10.1 入口与 API 风格

Iced 0.13 使用功能式 API：

```rust
fn main() -> iced::Result {
    tracing_subscriber::fmt().init();
    iced::application("Game Launcher", App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .run_with(App::new)
}
```

若 0.13 实际 API 签名与之有差异（如 `boot`/`init` 参数），以 0.13 文档为准，**结构不变**。

### 10.2 状态

```rust
pub struct App {
    db: Arc<Mutex<Db>>,                 // 或用通道封装的 DB 服务
    screen: Screen,
    games: Vec<Game>,
    runners: Vec<Runner>,
    selected_game: Option<Uuid>,
    draft: Option<Game>,                // GameDetail 编辑中的副本
    session: SessionType,
    x11_outputs: Vec<X11Output>,
    global_env: BTreeMap<String, String>,
    pending: PendingState,
    error: Option<String>,
    dialog_intent: Option<DialogIntent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen { Library, GameDetail, RunnerManager, Settings }

#[derive(Default)]
pub struct PendingState {
    pub prefix_creation: HashSet<Uuid>,
    pub launching: HashSet<Uuid>,
    pub runner_validation: HashSet<Uuid>,
}
```

DB 访问：`Db` 非 `Sync`（rusqlite Connection 是 `Send` 非 `Sync`），推荐在 `main` 创建 `tokio::sync::mpsc` 的 DB actor，或 `Arc<Mutex<Db>>` + `spawn_blocking`。**首期用 `Arc<Mutex<Db>>` 配合所有 DB 操作在 `spawn_blocking` 中执行**，简单可靠。

### 10.3 Message

```rust
#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    SelectGame(Uuid),
    DraftChanged(DraftField, String),  // 简单字段编辑
    DisplayConfigChanged(DisplayConfig),

    // 初始化加载
    GamesLoaded(Vec<Game>),
    RunnersLoaded(Vec<Runner>),
    SettingsLoaded(BTreeMap<String, String>),
    SessionDetected(SessionType),

    // 文件对话框
    PickExe,
    PickProtonDir,
    PickPrefixDir,
    PickDxvkDir,
    DialogResult { intent: DialogIntent, path: Option<PathBuf> },

    // 导入游戏
    ImportGame { exe: PathBuf, name: String },

    // Runner 管理
    ImportProton { dir: PathBuf },
    RunnerValidated { result: Result<Runner, String> },
    RemoveRunner(Uuid),

    // Prefix
    CreatePrefix { game_id: Uuid },
    PrefixResult { game_id: Uuid, result: Result<PathBuf, String> },

    // 启动
    Launch(Uuid),
    Launched { game_id: Uuid, result: Result<(), String> },
    GameExited { game_id: Uuid, code: Option<i32> },

    // 保存
    SaveDraft,
    DraftSaved(Result<(), String>),

    DismissError,
    Error(String),
}
```

### 10.4 update 规则

- `PickExe`/`PickProtonDir`/`PickPrefixDir`/`PickDxvkDir` → 返回 `Command::perform` 在 `spawn_blocking` 调 `rfd::FileDialog`，结果发 `DialogResult`。
- `ImportProton` → `Command::perform(spawn_blocking(validate_proton_dir))` → `RunnerValidated`。
- `CreatePrefix` → 插入 `pending.prefix_creation`，`Command::perform` 异步调用 `PrefixPlan::initialize` → `PrefixResult`。
- `Launch` → 插入 `pending.launching`，异步执行 `build_plan` + `spawn_plan` + `spawn` 一个等待任务，先发 `Launched`，进程退出后发 `GameExited`。实现上用 `Command::run` 返回一个在进程退出时产生 `GameExited` 的 future；`Launched` 可在 spawn 成功后立即由 `update` 本地设置（不额外发消息）。
- 所有 `Result` 失败 → `error` 字段 + `pending` 清理。

**约束**：`view` 与 `update` 中不得执行任何阻塞系统调用（xrandr 解析除外，见下）。DB 与 prefix/launch 全部异步。

### 10.5 view 划分

- `Library`：`scrollable(column(game_card...))`，顶栏按钮"导入游戏"、"Runner 管理"、"设置"。
- `GameDetail`：Tab 或纵向分节：基本信息 / 兼容层（Runner 选择 + Prefix 创建按钮 + 状态）/ 显示 / 环境变量（表格编辑）/ DLL Override（行编辑：名称 + 模式下拉 + 删除）/ 启动参数 / 保存 / 启动。
- `RunnerManager`：列表（名称、类型、版本、verified 标识、删除）+ "导入 Proton 目录" + "重新探测系统 Wine"。
- `Settings`：全局环境变量表、默认 Runner 选择、默认 prefix 根目录。

编辑通过 `draft: Option<Game>` 承载，`SaveDraft` 时写回 DB 并更新 `games`。

### 10.6 文件对话框异步包装

```rust
fn file_dialog(intent: DialogIntent) -> Command<Message> {
    Command::perform(
        tokio::task::spawn_blocking(move || {
            let d = rfd::FileDialog::new();
            let p = match intent {
                DialogIntent::PickExe => d.add_filter("exe", &["exe"]).pick_file(),
                _ => d.pick_folder(),
            };
            (intent, p)
        }),
        |r| match r {
            Ok((intent, path)) => Message::DialogResult { intent, path },
            Err(_) => Message::Error("dialog failed".into()),
        },
    )
}
```

### 10.7 主题与字体

使用 Iced 默认主题与内置字体；不引入自定义字体文件（首期）。如中文显示缺字，改用系统字体名（Iced `Font::with_name`）。

**验收**：
- 断网运行，完成"导入 exe → 选择系统 Wine → 创建 prefix → 启动"全流程（Wine 可用时）。
- 所有配置重启后保留。
- `cargo clippy --workspace -- -D warnings` 通过。

---

## 11. 分阶段实施计划

每阶段结束必须 `cargo build --workspace` + `cargo test` 通过，并在 `CHANGELOG.md` 记录。

**P1 — 核心类型与存储**
- 实现 `launcher-core` 全部类型（含 serde/Default）。
- 实现 `storage` 全部方法与迁移。
- 测试：round-trip 测试全覆盖。
- 验收：`cargo test -p storage` 通过。

**P2 — runner-wine**
- `detect_system_wine`、`validate_proton_dir`、`WineContext::{base_env,build_command,spawn}`。
- 测试：env/command 真值表；validate 用临时目录。
- 验收：`cargo test -p runner-wine` 通过。

**P3 — prefix**
- `PrefixPlan::{default_for, initialize}`、`install_dxvk`。
- 测试：路径计算、`system.reg` 存在即跳过、伪造 DLL 复制；真实 wineboot 测试 `#[ignore]`。
- 验收：`cargo test -p prefix` 通过；若有 Wine 环境手动跑一次初始化。

**P4 — display**
- `detect_session`、`list_x11_outputs`、`build_gamescope_args`、`gamescope_available`。
- 测试：xrandr fixture 解析；gamescope 参数真值表。
- 验收：`cargo test -p display` 通过。

**P5 — launch**
- `build_env`、`build_dll_overrides`、`build_plan`、`spawn_plan`、`LaunchHandle::wait`。
- 测试：优先级、冲突检测、gamescope 拼接。
- 验收：`cargo test -p launch` 通过。

**P6 — UI 骨架**
- Iced 应用、`Screen` 导航、Library 列表（读 DB）、GameDetail 静态布局、rfd 文件对话框接入、导入游戏写库。
- 验收：可导入 exe 并显示在列表；重启后仍在。

**P7 — Runner 与 Prefix UI 打通**
- RunnerManager 探测/导入/删除；GameDetail 选择 Runner、创建 Prefix（异步 + pending 禁用按钮）。
- 验收：断网完成导入 Proton + 创建 prefix（有 Proton/系统 Wine 时）。

**P8 — 显示与环境变量/DLL override UI**
- 显示表单（X11 输出下拉、gamescope 开关、分辨率/刷新）；环境变量表编辑；DLL override 行编辑；冲突检测提示。
- 验收：保存后重启保留；冲突时禁止启动并提示。

**P9 — 启动打通与边界**
- Launch 全流程、`GameExited` 更新 `last_played_at`、错误展示；prefix 引用计数与删除保护；残留 wineserver 清理（可选：退出时对每个用过的 prefix 执行 `wineserver -k` 仅在用户确认后）。
- 验收：满足第 12 节离线清单。

---

## 12. 离线验收清单（交付必须全部通过，断网状态）

1. 导入本地 `.exe`，填写名称、工作目录，保存。
2. 探测到系统 Wine 或导入 Proton 目录，出现在 Runner 列表且 `verified=true`。
3. 为游戏创建 prefix（选 arch；可选用户提供 DXVK 目录），UI 不卡顿，完成后状态显示已初始化。
4. 显示配置：X11 选择输出与分辨率；Wayland 启用 gamescope 并选分辨率/全屏；保存。
5. 编辑环境变量与结构化 DLL override；若手动设 `WINEDLLOVERRIDES` 则启动被拒绝并提示。
6. 点击启动：子进程 spawn，游戏启动（或 Wine 报错在 UI 展示）；`last_played_at` 更新。
7. 重启应用，以上所有配置从 SQLite 恢复。
8. 全程无任何网络请求（可用 `strace -f -e trace=network` 抽查启动与导入流程）。
9. `cargo clippy --workspace -- -D warnings` 与 `cargo test --workspace` 通过。
10. `cargo build --release` 产物为单一可执行文件，`ldd` 无 WebKit/Qt/GTK 依赖（rfd 在无 portal 时可能依赖 zenity，属外部可选，需在 README 注明；首选 portal 路径）。

---

## 13. 边界与错误处理要求

- 所有子进程调用检查退出码；stderr 捕获并截断 4KB 存入错误。
- prefix 删除前检查 games 表中 `prefix_path` 引用数；>0 拒绝。
- Runner 删除前检查 games 表中 `runner_id` 引用数；>0 拒绝。
- 游戏 exe 不存在时启动前报错（不 spawn）。
- gamescope `Forced` 但不可用 → 启动前报错 `GamescopeUnavailable`。
- 文件对话框取消（`None`）不报错，直接忽略。
- DB 打开失败（路径不可写）→ 启动时显示错误页，不静默。

---

## 14. 参考项目与明确取舍（供实现时查阅，不引入其代码）

- **Heroic**：prefix 路径可配置、per-game runner 选择的模式可参考；不采用 Electron。
- **onlinefix-linux**：`WINEDLLOVERRIDES` 语义与自动化目标；不采用其 DevelNext 栈。
- **Hydra**：不采用 Electron/Node 架构。
- **Lutris Display Management**：X11 后端抽象、xrandr 解析、gamescope 集成思路；不采用 Python，不实现 Mutter D-Bus 深度。
- **protontool / wincompatlib**：`WineContext` 持有关系与 wine 命令封装模式可参考；**不直接依赖**其 crate，核心逻辑自实现。

---

## 15. 交付物清单

- `Cargo.toml`（workspace）、各 crate 的 `Cargo.toml` 与源码
- `CHANGELOG.md`（按阶段记录）
- `README.md`：构建方式、外部依赖说明（Wine/Proton 需用户自备；rfd 的 portal/zenity 回退说明；DXVK 由用户提供目录）
- 第 12 节验收清单的执行记录（哪项通过、用的环境）

Agent 从 P1 开始，每阶段完成并自测后再进入下一阶段；任何架构级疑问（如 Iced 0.13 API 差异）以本文档决策为准做最小适配，并在 CHANGELOG 记录。
