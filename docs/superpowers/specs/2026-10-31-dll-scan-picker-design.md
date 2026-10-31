# DLL Override 目录扫描选择器 — 设计

- 日期：2026-10-31
- 状态：已确认设计，待实现
- 范围：GameHub UI 的 DLL Override 管理与 application 扫描逻辑

## 1. 目标

把「手动输入 DLL 名」扩展为「点击添加 → 递归扫描目录 → 列表勾选 → 批量写入 Override」，保留手动输入作为兜底入口。

### 非目标

- 不解析 DLL 的导出表、架构或依赖关系。
- 不修改 DXVK/NVAPI 安装流程。
- 不新增网络请求或运行时下载依赖。

## 2. 用户流程

1. 在游戏详情页的 DLL Override 卡片点击「从目录添加」。
2. 对话框打开，默认扫描该游戏的可执行文件所在目录；若该游戏有上次扫描目录，则优先使用。
3. 扫描完成后按 DLL 文件名去重展示，用户可搜索、排序、全选/反选/清空。
4. 用户选择 Override 模式并勾选 DLL，点击「确认添加」。
5. 勾选项批量追加到当前草稿并立即保存整个草稿（与顶部「保存」同一条落库路径）。
6. 成功后关闭对话框、刷新 DLL 列表并在页面顶部显示成功提示（含新增数量）。

### 交互形态（已确认）

采用全屏模态框（opaque 遮罩 + 居中卡片），沿用现有 `platform_prompt` / `confirm_delete` 的模态模式：

```
┌─ 从目录添加 DLL Override ─────────────────────────────┐
│ 扫描目录  /games/Cyberpunk 2077/bin       [更改] [重扫] │
│ 找到 128 个 DLL · 已选 3 个 · 跳过 0 个非 UTF-8         │
│ [搜索 DLL 名称…]      [名称 ↑] [全选][反选][清空]      │
│ 模式 [原生+内置 (n,b) ▾]                               │
│ ┌───────────────────────────────────────────────────┐ │
│ │ ☑ d3d11        bin/x64/d3d11.dll                  │ │
│ │ ☐ dxgi         bin/x64/dxgi.dll                   │ │
│ │ ☑ winmm        bin/winmm.dll            [已添加]   │ │
│ └───────────────────────────────────────────────────┘ │
│                                          [取消] [确认添加] │
└───────────────────────────────────────────────────────┘
```

## 3. 架构

### 3.1 application crate

新增 `crates/application/src/dll_scan.rs`，无 GUI 依赖，负责全部文件系统遍历。

```rust
pub const DLL_SCAN_MAX_DEPTH: usize = 16;
pub const DLL_SCAN_MAX_RESULTS: usize = 5_000;
pub const DLL_SCAN_MAX_VISITED_FILES: usize = 200_000;

#[derive(Debug, Clone)]
pub struct DllCandidate {
    /// 去掉 .dll 后缀的文件名，保留原始大小写用于展示与写入。
    pub name: String,
    /// 小写化的去重键。
    pub key: String,
    /// 相对扫描根目录的路径，用于区分同名 DLL。
    pub relative_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DllScanOutcome {
    pub entries: Vec<DllCandidate>,
    pub skipped_non_utf8: usize,
    pub unreadable_dirs: usize,
    pub truncated: bool,
    pub depth_limited: bool,
}

#[derive(Debug, Clone)]
pub enum DllScanError {
    NotFound(PathBuf),
    NotDirectory(PathBuf),
    PermissionDenied(PathBuf),
    Io { path: PathBuf, message: String },
    Cancelled,
    Join(String),
}

/// 阻塞函数，调用方负责放进 spawn_blocking。
pub fn scan_dlls(
    root: &Path,
    cancel: &AtomicBool,
) -> Result<DllScanOutcome, DllScanError>;
```

同时提供：

```rust
pub fn dll_name_key(name: &str) -> String;
```

规则：`trim` → 若大小写不敏感地以 `.dll` 结尾则去掉 → 再次 `trim_end` → `to_ascii_lowercase`。扫描去重、已添加判断、勾选集合都使用该函数。

### 3.2 UI crate

- `crates/ui/src/app/dll_scan.rs`：`DllScanDialog`、`DllScanPhase`、`PendingApply` 及搜索/排序/选择等纯函数。
- `crates/ui/src/views/dll_scan.rs`：模态框视图。
- `crates/ui/src/app/message.rs`、`update.rs`、`view.rs`：消息与状态接入。
- `crates/ui/src/dialogs.rs`：新增 `DialogIntent::PickDllScanDir`。
- `crates/ui/src/views/game_detail.rs`：DLL 卡片新增「从目录添加」。
- `crates/ui/src/widgets/controls.rs` 或 `styles.rs`：复选框样式；新增成功提示条。
- `crates/ui/locales/zh-CN.json`、`en-US.json`：新增 `detail.dll_scan.*` 文案。

## 4. 扫描算法

1. 校验根路径：不存在 → `NotFound`；`metadata` 表明不是目录 → `NotDirectory`；读取失败且 `PermissionDenied` → `PermissionDenied`。根路径允许是符号链接（跟随到目标目录）。
2. 使用显式栈做深度优先遍历，栈元素为 `(path, depth)`。
3. 每层 `read_dir` 收集条目后按 `file_name` 排序，保证结果稳定可复现。
4. 符号链接（`DirEntry::file_type().is_symlink()`）一律跳过，不进入子目录，因此不会成环。
5. 目录：若 `depth >= DLL_SCAN_MAX_DEPTH`，置 `depth_limited = true` 且不下探；否则入栈。读取失败：根目录返回错误，子目录计入 `unreadable_dirs` 并继续。
6. 文件：累计 `visited_files`；超过 `DLL_SCAN_MAX_VISITED_FILES` 时置 `truncated = true` 并停止。
7. 仅处理扩展名大小写不敏感等于 `dll` 的文件。
8. 相对路径或文件名无法转为 UTF-8 → `skipped_non_utf8 += 1`，跳过。
9. 用 `dll_name_key` 去重；首次出现的候选保留，后续同名文件忽略。
10. 达到 `DLL_SCAN_MAX_RESULTS` → 置 `truncated = true` 并停止。
11. 每个循环迭代检查 `cancel`；为真则返回 `Cancelled`。
12. 结果按 `(key, relative_path)` 排序后返回。

### 上限行为

- `truncated`：达到结果上限或访问文件上限，UI 提示「扫描范围过大，结果可能不完整，请缩小目录」。
- `depth_limited`：存在超过 16 层的目录未扫描，UI 单独提示。
- 两种提示可以同时出现。

## 5. 对话框状态

```rust
pub struct DllScanDialog {
    pub game_id: Uuid,
    /// `None` means no usable starting directory; the dialog shows
    /// `DllScanPhase::NeedDirectory` until the user picks one.
    pub root: Option<PathBuf>,
    pub phase: DllScanPhase,
    pub query: String,
    pub sort_desc: bool,
    pub selected: HashSet<String>,      // dll_name_key
    pub mode: OverrideMode,             // 默认 NativeBuiltin
    pub existing: HashSet<String>,      // 打开时草稿中已有的 key
    pub scan_seq: u64,
    pub cancel: Arc<AtomicBool>,
    pub applying: bool,
    pub apply_error: Option<String>,
    pub pending_apply: Option<PendingApply>,
}

pub enum DllScanPhase {
    /// 没有可用的初始目录（例如新游戏尚未选择可执行文件）。
    NeedDirectory,
    Scanning,
    Ready(DllScanOutcome),
    Failed(DllScanError),
}

pub struct PendingApply {
    pub original_rows: Vec<DllRow>,
    pub added: Vec<(String, OverrideMode)>,
    pub revision: u64,
}
```

`App` 新增：

- `pub dll_scan: Option<DllScanDialog>`
- `pub scan_dirs: HashMap<Uuid, PathBuf>`（从 settings 加载的每游戏上次目录）
- `pub success: Option<String>`（成功提示条）

### 已添加条目

打开对话框时，遍历 `draft.dll_rows`，对每个非空名称取 `dll_name_key` 放入 `existing`。渲染时：

- checkbox 为 checked 且无 `on_toggle`（Disabled）；
- 名称与路径使用 `faint`；
- 追加「已添加」徽章；
- 不进入 `selected`，确认时不会重复写入。

### 选择语义

- `selected` 只包含可添加条目的 key。
- 搜索只按 DLL 名称做大小写不敏感子串匹配，不匹配相对路径。
- 「全选」勾选当前可见、非 `existing` 的全部条目；「反选」对当前可见可添加项取反；「清空」清空所有选择。
- 「已选择 N 个」只统计 `selected`。
- 默认排序按名称升序；排序按钮在升序/降序间切换，仅影响展示顺序，不改变选择。
- 列表最多渲染 300 条（`DLL_SCAN_RENDER_LIMIT`），全选/反选也作用于该上限内的筛选结果；超出时提示使用搜索缩小范围，避免一次布局数千个复选框。
- Override 模式复用 `override_mode_choices`（Disabled / Native / Builtin / NativeBuiltin），默认 `NativeBuiltin`。本次确认的所有 DLL 使用同一模式。

## 6. 消息与副作用

新增消息：

```rust
OpenDllScan
CloseDllScan
DllScanPickDir
DllScanQueryChanged(String)
DllScanToggle(String)
DllScanSelectAll
DllScanInvert
DllScanClear
DllScanSortToggled
DllScanModeChanged(OverrideMode)
DllScanRescan
DllScanCancelScan
DllScanFinished { seq: u64, result: Result<DllScanOutcome, DllScanError> }
DllScanConfirm
DllScanDirSaved(Result<(), String>)
DismissSuccess
```

### 打开

1. 若 `dll_scan.is_some()` 或当前没有草稿 / 草稿为原生平台 → 忽略。
2. `root = scan_dirs.get(game_id)` 或 `executable_path.parent()`。
3. 若没有可用目录：进入 `NeedDirectory` 态，提示先「更改扫描目录」，不发起扫描。
4. `existing` 从当前草稿快照。
5. 有目录时：`scan_seq += 1`，创建新的取消标志，进入 `Scanning`，返回扫描 Task。

### 扫描 Task

```rust
Task::perform(
    async move {
        tokio::task::spawn_blocking(move || scan_dlls(&root, &cancel))
            .await
            .unwrap_or_else(|error| Err(DllScanError::Join(error.to_string())))
    },
    move |result| Message::DllScanFinished { seq, result },
)
```

`update` 收到 `DllScanFinished` 时：

- 对话框不存在或 `seq` 不匹配 → 丢弃；
- `Ok(outcome)` → `Ready`，并把 `root` 记入 `scan_dirs`，异步持久化；
- `Err(Cancelled)` → 丢弃；
- 其他 `Err` → `Failed`。

### 更改目录 / 重扫

- 更改目录：`rfd` 选目录，`DialogResult` 中更新 `root`，清空 `query`、`selected`、重置排序，重新扫描。
- 重扫：保留 `query` 与 `mode`，清空 `selected`，重新扫描。
- 发起新扫描前先 `cancel.store(true)` 旧标志并递增 `scan_seq`。

### 取消 / 关闭

- 扫描中取消：`cancel.store(true)`，关闭对话框，丢弃结果。
- Ready/Failed 态取消：直接关闭。
- `applying == true` 时取消按钮禁用，避免保存中丢失状态。

## 7. 确认与落库

1. 若 `selected` 为空：对话框内提示「请先勾选要添加的 DLL」，不发起保存。
2. 从 `Ready` 结果中取出 `selected` 对应候选，过滤 `existing`；按 `key` 排序生成 `added`。
3. 保存 `original_rows = draft.dll_rows.clone()`。
4. 追加 `DllRow { name, mode }`，`touch_draft()`。
5. `draft.to_game()` 校验：失败则回滚 `dll_rows`，在对话框内显示本地化错误，保持勾选。
6. 无 service：回滚并显示错误。
7. 记录 `revision = draft_revision`，设置 `applying = true`，调用现有 `save_draft()`（顶部保存同一路径）。
8. `DraftSaved` 成功且对话框 `game_id` / `revision` 匹配：
   - 取 `pending_apply.added.len()` 作为新增数量；
   - 关闭对话框、清空 `pending_apply` 与 `apply_error`；
   - `success = Some(tr!("detail.dll_scan.added", count))`。
9. `DraftSaved` 失败且来自本对话框：
   - 回滚 `draft.dll_rows = original_rows`，`touch_draft()`；
   - `applying = false`，`apply_error = Some(本地化错误)`；
   - 对话框保持打开，`selected` 不变。
10. 成功提示条与原错误条并列渲染在页面内容上方，带关闭按钮（`DismissSuccess`）。

> 确认添加会保存整个草稿，包括当前其他未保存编辑。这是已确认的行为。

## 8. 记住扫描目录

- settings 键：`dll_scan_dir.<game_id>`，值为目录路径字符串。
- 启动 `apply_loaded` 时扫描 `settings_map`，解析此前缀到 `App.scan_dirs`。
- 仅在扫描成功（根目录可读）后写入；目录变化后下一次成功扫描覆盖旧值。
- 路径无法转 UTF-8 时不持久化，仅本次会话生效。
- 持久化失败只 `tracing::warn!`，不向用户报错。
- 游戏删除后遗留的键暂不清理，避免扩大本次范围。

## 9. 视图细节

- 模态层：`opaque(container(card).width(Fill).height(Fill).center(Fill).style(styles::backdrop))`。
- 卡片：`max_width(780)`、`max_height(640)`、`padding(22)`、`styles::modal_card`。
- 顶部：标题 + 关闭按钮。
- 目录行：路径 `faint`（超长由容器裁剪），右侧「更改扫描目录」「重新扫描」。
- 扫描中：显示「正在扫描目录…」与「取消扫描」，隐藏列表与确认。
- 需要目录：显示「请选择要扫描的目录」+「更改扫描目录」「关闭」。
- 失败：显示本地化错误 + 「更改扫描目录」「重新扫描」「关闭」。
- 扫描错误文案集中在 `crates/ui/src/i18n/errors.rs` 新增的 `dll_scan_error(lang, &DllScanError)` 中，按错误类型拼接路径；`Cancelled` 不展示（结果已丢弃）。
- 就绪：
  - 统计行：找到数量、已选数量、跳过非 UTF-8 数量；
  - 警告行：截断、深度限制、不可读子目录（可多条）；
  - 工具栏：搜索框（Fill）、排序按钮、全选/反选/清空；
  - 模式行：标签 + `pick_list`；
  - 列表：`scrollable` + `styles::list_panel`，每行 checkbox + 名称 + 相对路径 + 可选「已添加」徽章；
  - 空态：「该目录下未找到 .dll 文件」；
  - 底部：左侧已选数量，右侧取消 + 确认（无选择时禁用）。
- 保存中：确认按钮显示「正在保存…」并禁用；保存错误以危险色显示在底部上方。
- 复选框新增 `styles::checkbox`：主色选中、`text_faint` 禁用态，保证浅色/深色主题下文字与勾选图标可见。

## 10. 错误与边界

| 情况 | 行为 |
| --- | --- |
| 目录不存在 | `NotFound`，对话框错误态，可重新选目录 |
| 路径不是目录 | `NotDirectory`，同上 |
| 根目录无读权限 | `PermissionDenied`，同上 |
| 子目录无读权限 | 计入 `unreadable_dirs`，继续扫描并提示 |
| 空目录 / 无 DLL | 「该目录下未找到 .dll 文件」 |
| 结果超 5000 或访问文件超 200000 | `truncated`，提示缩小范围 |
| 深度超过 16 | `depth_limited`，提示部分深层目录未扫描 |
| 非 UTF-8 路径 | 跳过并计数提示 |
| 目录软链接成环 | 不跟随软链接，不会递归 |
| 扫描中取消 | 置取消标志、关闭、丢弃结果 |
| 保存失败 | 对话框保持打开、回滚追加行、保留勾选、显示错误 |
| 其他字段校验失败 | 同上，回滚后用户可取消去修复其他字段 |
| 重复 DLL | `existing` 标记且禁用；确认集合过滤，不会产生重复条目 |

## 11. 测试计划

### application 单元测试

- 递归收集多层目录中的 `.dll`，扩展名大小写不敏感。
- 同名不同路径只保留一个；结果按键排序。
- 深层目录超过上限时置 `depth_limited`。
- 结果数量达到上限时置 `truncated`。
- 访问文件数达到上限时置 `truncated`。
- 非 UTF-8 文件名跳过并计数（Unix `OsStringExt`）。
- 根目录不存在/不是目录返回对应错误。
- 目录软链接成环不导致无限递归。
- 取消标志为真时返回 `Cancelled`。
- `dll_name_key` 对大小写、`.dll`、空白的行为。

### UI 状态测试

- 打开对话框：优先使用已记住目录，否则使用可执行文件父目录；`existing` 来自草稿。
- 搜索过滤、排序切换、全选只作用于可见可添加项、反选、清空。
- 确认只追加非 `existing` 的勾选项，并设置 `applying`。
- `DraftSaved` 成功：关闭对话框、成功提示数量正确。
- `DraftSaved` 失败：回滚 `dll_rows`、保留 `selected`、`applying=false`、`apply_error` 有值。
- 扫描取消：设置取消标志并关闭。
- 过期 `scan_seq` 结果被忽略。
- i18n 键集合与占位符测试通过。

### 手动验证

- 10,000 文件目录扫描期间 UI 可交互、不卡顿。
- 勾选若干 DLL 并确认，Override 列表新增条目且模式一致。
- 重复添加同一 DLL 不产生重复条目。
- 权限不足、目录不存在、空目录三种情况提示清晰、应用不崩溃。
- 手动输入入口仍然可用。
- 浅色/深色主题下复选框、名称、徽章、错误文字均可见。

## 12. 验收标准映射

| 验收项 | 实现点 |
| --- | --- |
| 1. 无需手动输入即可看到去重 DLL | 打开即扫描 + `dll_name_key` 去重 |
| 2. 勾选确认后条目与模式正确 | 批量追加 `DllRow` + 单一 `mode` |
| 3. 1 万文件时 UI 可交互 | `spawn_blocking` + 消息回传 |
| 4. 重复添加不产生重复 | `existing` 禁用标记 + 确认过滤 |
| 5. 权限/不存在/空目录提示且不崩溃 | 类型化错误 + 空态文案 |
| 6. 手动入口仍可用 | 原按钮保留 |
