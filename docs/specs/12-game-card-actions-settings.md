# P12 游戏卡片操作与兼容层增强设置设计

**Goal:** 游戏卡片提供“启动 / 设置 / 日志”三个操作，启动状态区分“启动中”和“游玩”；新增游戏日志页；设置页增加 DXVK-NVAPI 自动安装/更新、Esync、Fsync、MangoHud 开关，并把这些配置注入启动流程。

**非目标：**

- 不下载 DXVK、DXVK-NVAPI、MangoHud 或任何运行时资源。
- 不新增“停止游戏”操作。
- 不为这四项设置提供单游戏覆盖，全部是全局设置。
- 不做日志上传、日志搜索或日志归档管理。

**参考：** Heroic Games Launcher 的 GameCard、游戏日志和兼容层设置行为。

---

## 1. 游戏卡片与启动状态

### 1.1 布局

`views/library.rs` 中每张卡片改为左右布局：

- 左侧：游戏名、可执行文件路径、最近游玩时间。
- 右侧：`启动`、`设置`、`日志` 三个按钮。
- 状态徽标显示在游戏名旁：
  - `starting`：`启动中`
  - `playing`：`游玩`
  - 其他：不显示

卡片不再整体作为按钮，避免按钮嵌套。`设置` 复用 `Message::SelectGame(id)` 进入现有游戏详情页；`日志` 使用新的 `Message::OpenLogs(id)`。

### 1.2 状态机

`App` 保留现有 `running: HashMap<Uuid, PathBuf>` 作为“启动中或游玩中”的运行集合，用于重复启动和删除保护；新增：

```rust
pub starting: HashSet<Uuid>,
```

状态规则：

| 状态 | 条件 | 启动按钮 | 卡片/详情徽标 |
|---|---|---|---|
| 空闲 | 不在 `running` | 可用 | 无 |
| 启动中 | `starting` 包含 id | 禁用 | `启动中` |
| 游玩 | `running` 包含 id 且不在 `starting` | 禁用 | `游玩` |

点击启动时立即：

1. 插入 `running`（用于同 prefix 检查和删除保护）。
2. 插入 `starting`。
3. 发起带事件的启动任务。

application 层在 spawn 成功后发送 `Started`；UI 收到后从 `starting` 移除，状态变为 `游玩`。进程退出或启动失败后同时移除 `running`/`starting`。

### 1.3 启动事件

`application` 新增：

```rust
#[derive(Debug, Clone)]
pub enum LaunchEvent {
    Started { game_id: Uuid },
    Finished { game_id: Uuid, outcome: LaunchOutcome },
    Failed { game_id: Uuid, error: String },
}
```

`AppService` 新增：

```rust
pub async fn launch_game_with_events(
    self,
    request: LaunchRequest,
    events: tokio::sync::mpsc::UnboundedSender<LaunchEvent>,
) -> Result<LaunchOutcome, AppError>;
```

保留现有 `launch_game(request)` 作为无事件包装，供现有测试使用。

事件顺序保证：

- 安装/校验失败：只发 `Failed`。
- spawn 失败：只发 `Failed`。
- spawn 成功：先发 `Started`，等待结束后发 `Finished`；等待本身失败时发 `Failed`。

UI 用 `iced::Task::stream` 消费事件，映射为 `Message::LaunchEvent(LaunchEvent)`。流任务内部使用 `iced::stream::channel`，在同一任务中并发运行 `launch_game_with_events` 与事件转发，确保启动 future 被真正驱动。启动任务不依赖当前 `Screen`，切换页面不会中断。

`Message` 追加：

```rust
LaunchEvent(application::LaunchEvent),
OpenLogs(Uuid),
RefreshLogs,
LogsLoaded { game_id: Uuid, result: Result<String, String> },
```

`Launch(Uuid)` 不再直接 `Task::perform`，改为创建事件通道并返回流任务。

---

## 2. 游戏日志

### 2.1 路径与写入

`AppPaths` 新增：

```rust
pub fn game_log_path(&self, game_id: Uuid) -> PathBuf {
    self.logs_dir.join("games").join(format!("{game_id}.log"))
}
```

每次启动前：

1. 创建 `logs/games/`。
2. 若 `<id>.log` 存在，替换为 `<id>.log.old`（旧 `.old` 先删除）。
3. 新日志写入头部：

   ```text
   ===== gamehub launch <RFC3339 timestamp> =====
   Running command: <program> <args...>
   ```

4. 子进程 stdout/stderr 逐行写入同一文件，stderr 行加 `[stderr] ` 前缀。
5. 子进程退出后立即发送 `Finished`；读取任务继续把已到达的数据写完，随后自然结束。

不等待读取任务结束：Wine 可能派生子进程并继承管道，等待读取任务可能永久挂起。日志页的自动刷新会看到后续写入。

日志写入失败不阻断启动：回退为 `stdout=null`、`stderr=inherit`，并通过 `tracing::warn!` 记录。

`launch` 新增：

```rust
pub async fn spawn_plan_with_log(
    plan: &LaunchPlan,
    log_path: Option<&Path>,
) -> Result<tokio::process::Child, LaunchError>;
```

`spawn_plan_with_log` 负责打开/轮转日志、设置管道并启动两个脱离的读取任务；日志读取任务不参与等待。`spawn_plan` 保持现有签名，内部委托为 `spawn_plan_with_log(plan, None)`。

### 2.2 日志页

新增 `Screen::Logs` 和 `views/logs.rs`：

- 顶部：返回、游戏名、`刷新`。
- 内容：等宽字体展示最后 1000 行；文件不存在或为空时提示“该游戏还没有启动日志”。
- 进入页面时异步读取；`Screen::Logs` 时通过 `App::subscription` 每秒触发一次 `RefreshLogs`，并在 `run()` 中给 `iced::application` 挂上 `.subscription(App::subscription)`。
- 读取使用 `tokio::fs::read_to_string`（或 `spawn_blocking`），只保留最后 1000 行。
- 读取期间保留上一次内容，避免闪烁；错误显示在错误横幅。

日志页只展示当前游戏最新一次启动日志，不展示启动器 tracing 日志。

---

## 3. 设置页新增项

### 3.1 持久化字段

`SettingsState` 增加：

```rust
pub enable_esync: bool,              // 默认 true
pub enable_fsync: bool,              // 默认 true
pub enable_mangohud: bool,           // 默认 false
pub auto_install_dxvk_nvapi: bool,   // 默认 false
pub dxvk_nvapi_source: Option<PathBuf>,
```

settings 表键：

```text
enable_esync
enable_fsync
enable_mangohud
auto_install_dxvk_nvapi
dxvk_nvapi_source
```

布尔值写为 `"true"`/`"false"`；读取时接受 `"true"`/`"false"`/`"1"`/`"0"`，缺失用默认值，其他值返回设置读取错误。

`SettingsAction` 追加：

```rust
EnableEsync(bool),
EnableFsync(bool),
EnableMangoHud(bool),
AutoInstallDxvkNvapi(bool),
DxvkNvapiSource(Option<PathBuf>),
```

`DialogIntent` 追加 `PickDxvkNvapiDir`。

### 3.2 设置页 UI

在现有设置页追加“兼容层增强”区块：

- `自动在前缀安装并更新 DXVK-NVAPI` 复选框。
- DXVK-NVAPI 目录文本 + `选择目录` + `清除`。
- 启用但未选择目录时显示提示：`已启用自动安装，但未选择 DXVK-NVAPI 目录；Wine 游戏启动会失败`。
- `启用 Esync`、`启用 Fsync` 复选框。
- `启用 MangoHud` 复选框，并显示检测结果：
  - 已检测到：`已检测到：<path>`
  - 未检测到：`未检测到 mangohud，启用后启动会被拒绝`
- 保存时不强制要求 NVAPI 目录（Proton 游戏不需要复制），但启用且无目录时给出提示。

MangoHud 检测在 `LoadedData` 加载时完成，新增：

```rust
pub mangohud_path: Option<PathBuf>,
```

使用与 gamescope 相同的 `find_in_path("mangohud", PATH)` 逻辑，结果缓存在 `App.mangohud_path`。

---

## 4. 启动注入

### 4.1 LaunchRequest / LaunchTuning

`launch` 新增：

```rust
#[derive(Debug, Clone, Default)]
pub struct LaunchTuning {
    pub enable_esync: bool,
    pub enable_fsync: bool,
    pub enable_mangohud: bool,
    pub mangohud_path: Option<PathBuf>,
    pub enable_dxvk_nvapi: bool,
}
```

`LaunchRequest` 追加：

```rust
pub tuning: LaunchTuning,
pub log_path: Option<PathBuf>,
pub dxvk_nvapi_source: Option<PathBuf>,
```

`dxvk_nvapi_source` 只由 `application` 使用；`launch` 忽略它。

### 4.2 环境变量

`launch::build_env` 先删除继承环境中的冲突变量：

```text
WINEESYNC, WINEFSYNC, PROTON_NO_ESYNC, PROTON_NO_FSYNC,
DXVK_ENABLE_NVAPI, DXVK_NVAPI_ALLOW_OTHER_DRIVERS,
PROTON_ENABLE_NVAPI, PROTON_DISABLE_NVAPI
```

然后按 runner 类型注入：

**Wine**

- `enable_esync` → `WINEESYNC=1`
- `enable_fsync` → `WINEFSYNC=1`
- `enable_dxvk_nvapi` → `DXVK_ENABLE_NVAPI=1`、`DXVK_NVAPI_ALLOW_OTHER_DRIVERS=1`

**Proton**

- `!enable_esync` → `PROTON_NO_ESYNC=1`
- `!enable_fsync` → `PROTON_NO_FSYNC=1`
- `enable_dxvk_nvapi` → `PROTON_ENABLE_NVAPI=1`、`DXVK_NVAPI_ALLOW_OTHER_DRIVERS=1`
- `!enable_dxvk_nvapi` → `PROTON_DISABLE_NVAPI=1`

游戏级/全局环境变量仍在这些默认值之后覆盖，保持现有优先级。

### 4.3 MangoHud

- `enable_mangohud && mangohud_path.is_none()` → `LaunchError::MangohudUnavailable`。
- 使用 gamescope 时：在 `build_gamescope_args` 的 `--` 之前追加 `--mangoapp`，不再套 wrapper。
- 不使用 gamescope 时：把最终命令包装为：

  ```text
  <mangohud_path> --dlsym <program> <args...>
  ```

`display::build_gamescope_args` 增加 `mangoapp: bool` 参数；`display` 新增 `mangohud_path()`。

---

## 5. DXVK-NVAPI 安装/更新

### 5.1 源目录

用户选择解压后的 DXVK-NVAPI 发布目录：

```text
<root>/x64/nvapi64.dll
<root>/x64/nvofapi64.dll      # 可选
<root>/x32/nvapi.dll
<root>/x32/nvofapi.dll        # 可选
```

校验规则：

- Win64 prefix：必须存在 `x64/nvapi64.dll`；`x32` 可选。
- Win32 prefix：必须存在 `x32/nvapi.dll`。
- 只复制 `nvapi*.dll`、`nvofapi*.dll`，忽略测试程序、layer 等文件。

### 5.2 安装行为

`prefix` 新增：

```rust
pub struct DxvkNvapiSource { pub root: PathBuf }
pub async fn install_dxvk_nvapi(
    plan: &PrefixPlan,
    source: &DxvkNvapiSource,
) -> Result<(), PrefixError>;
```

流程：

1. 要求 prefix 已初始化。
2. 按 prefix arch 选择源文件和目标目录：
   - Win64：`x64/*` → `drive_c/windows/system32`，`x32/*` → `drive_c/windows/syswow64`。
   - Win32：`x32/*` → `drive_c/windows/system32`。
3. 比较源文件与目标文件；仅复制缺失或内容不同的文件。
4. 若发生文件复制或标记文件不存在，对所有已安装 DLL 执行：

   ```text
   wine reg add HKCU\Software\Wine\DllOverrides /v <stem> /d native,builtin /f
   ```

   stem 为 `nvapi64`、`nvofapi64`、`nvapi`、`nvofapi`。
5. 在 prefix 根写标记文件 `.gamehub-dxvk-nvapi`。若所有目标文件已存在且内容一致、标记也存在，则跳过复制和 registry 写入。
6. 源目录无效、文件缺失、reg add 失败 → 返回错误，启动失败。

Proton 前缀不执行上述复制/注册表操作；`application` 只注入 Proton 的 NVAPI 环境变量。该行为与 Heroic 一致。

### 5.3 application 编排

`launch_game_with_events` 流程：

1. `plan_for_game` 得到 prefix plan。
2. 若 `enable_dxvk_nvapi && runner.kind == Wine`：
   - 未提供 `dxvk_nvapi_source` → 发送 `Failed`，错误“已启用 DXVK-NVAPI 自动安装，但未选择目录”。
   - 调用 `install_dxvk_nvapi`；失败则发送 `Failed`。
3. `build_plan` 并 `spawn_plan_with_log`。
4. 发送 `Started`。
5. 等待子进程和日志读取任务。
6. 更新 `last_played_at`。
7. 发送 `Finished` 或 `Failed`。

---

## 6. 代码落点

| 层 | 改动 |
|---|---|
| `launcher-core` | `AppPaths::game_log_path` |
| `display` | `mangohud_path()`；`build_gamescope_args(..., mangoapp)` |
| `launch` | `LaunchTuning`、`LaunchRequest` 新字段、环境注入、MangoHud 包装、`spawn_plan_with_log` |
| `prefix` | `DxvkNvapiSource`、`install_dxvk_nvapi`、新错误变体 |
| `application` | `LaunchEvent`、`launch_game_with_events`、NVAPI 编排、日志路径、`LoadedData.mangohud_path` |
| `ui` | 卡片按钮/状态、日志页、设置项、持久化、订阅、消息 |
| `Cargo.toml` | tokio 增加 `io-util` feature |

---

## 7. 错误处理

- 启动前错误（NVAPI 源缺失、MangoHud 未安装、prefix 未初始化）通过 `Failed` 事件进入现有错误横幅。
- 日志写入失败只告警，不阻断启动。
- 设置布尔值非法时，加载设置失败并使用默认值 + 错误横幅（沿用现有行为）。
- 日志读取失败显示错误，但不清空已显示的旧内容。
- 进程非零退出仍按现有逻辑提示“游戏异常退出（退出码 ...）”。

---

## 8. 测试

单元/集成测试：

- `launch`：Wine/Proton × Esync/Fsync/NVAPI 开关的环境变量矩阵。
- `launch`：MangoHud wrapper、gamescope `--mangoapp`、MangoHud 缺失错误。
- `launch`：日志文件创建、命令行头部、stdout/stderr 写入、旧日志轮转。
- `prefix`：NVAPI 源校验、Win64/Win32 复制目标、registry 参数、marker 跳过。
- `application`：`Started` → `Finished` / `Failed` 事件顺序；Wine NVAPI 安装失败中止启动；Proton 跳过复制。
- `application`：设置键默认值、布尔解析、序列化往返。
- `launcher-core`：`game_log_path` 稳定性。
- `ui`：状态机纯逻辑测试（如可独立抽出）；至少保证编译和状态更新分支覆盖。

验证命令：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

手工验证：

- 启动 `cargo run -p ui`，检查卡片三按钮、启动中/游玩状态切换、日志页刷新。
- 用假 Wine 脚本验证日志内容和 NVAPI 复制。
- 若当前环境无法完成交互式 UI 验证，在交付说明中明确列出未验证项。

---

## 9. 验收标准

1. 卡片右侧有启动、设置、日志三个按钮；设置进入游戏详情，日志进入该游戏日志页。
2. 点击启动后卡片先显示“启动中”，spawn 成功后显示“游玩”，退出后恢复。
3. 游戏 stdout/stderr 可在日志页查看，且只保留最新一次启动的日志。
4. 设置页出现四个开关和 NVAPI 目录选择，保存后重启应用仍保留。
5. Esync/Fsync/MangoHud/NVAPI 的注入行为符合第 4 节。
6. 启用 NVAPI 自动安装且使用 Wine 时，prefix 内 DLL 和 DllOverrides 被正确安装/更新；源缺失时启动失败并给出明确错误。
7. 启用 MangoHud 但未安装时启动失败并给出明确错误。
8. `cargo fmt --check`、`clippy -D warnings`、`cargo test --workspace` 全部通过。
