# DLL Override 目录扫描选择器 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在游戏详情的 DLL Override 卡片里提供「从目录添加」，递归扫描目录、去重勾选后批量追加到当前草稿，并走现有 `save_draft` 落库路径。

**Architecture:** 文件系统遍历放在 `application` crate（无 GUI 依赖，阻塞函数 + `spawn_blocking`）；`ui` crate 新增 `DllScanDialog` 状态、模态视图和消息接线。扫描目录通过 settings 表的 `dll_scan_dir.<uuid>` 键按游戏记忆。确认添加把选中的 DLL 追加到 `draft.dll_rows` 后调用现有 `save_draft()`，失败时回滚追加行。

**Tech Stack:** Rust 2024, iced 0.13, tokio `spawn_blocking`, tempfile (dev), iced_runtime (dev).

**约束：** 用户要求只在明确要求时提交；本计划不包含 `git commit` 步骤。

---

## File Structure

- Create: `crates/application/src/dll_scan.rs` — 扫描算法、候选/结果/错误类型、`dll_name_key`。
- Modify: `crates/application/src/lib.rs` — 导出扫描 API 与 settings 辅助函数。
- Modify: `crates/application/src/settings.rs` — 每游戏扫描目录的 settings 键与解析。
- Create: `crates/ui/src/app/dll_scan.rs` — 对话框状态、选择/搜索纯函数、App 的打开/扫描/确认/落库方法、UI 测试。
- Modify: `crates/ui/src/app/mod.rs` — 新字段、模块声明、`apply_loaded` 读取记忆目录、`apply_draft_saved` 处理确认结果。
- Modify: `crates/ui/src/app/message.rs` — 新消息。
- Modify: `crates/ui/src/app/update.rs` — 消息分发。
- Modify: `crates/ui/src/app/view.rs` — 成功提示条与模态层。
- Create: `crates/ui/src/views/dll_scan.rs` — 模态视图。
- Modify: `crates/ui/src/views/mod.rs` — 注册视图模块。
- Modify: `crates/ui/src/views/game_detail.rs` — DLL 卡片新增「从目录添加」。
- Modify: `crates/ui/src/dialogs.rs` — `DialogIntent::PickDllScanDir`。
- Modify: `crates/ui/src/styles.rs` — `styles::checkbox`。
- Modify: `crates/ui/src/widgets/mod.rs` — `success_banner`。
- Modify: `crates/ui/src/i18n/errors.rs` — `dll_scan_error`。
- Modify: `crates/ui/locales/zh-CN.json`, `crates/ui/locales/en-US.json` — 文案。

---

### Task 1: application 扫描核心

**Files:**
- Create: `crates/application/src/dll_scan.rs`
- Modify: `crates/application/src/lib.rs:1-38`

- [ ] **Step 1: 写扫描模块（含实现，测试随后）**

创建 `crates/application/src/dll_scan.rs`：

```rust
//! Recursive, cancellable scan for `.dll` files below a directory.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub const DLL_SCAN_MAX_DEPTH: usize = 16;
pub const DLL_SCAN_MAX_RESULTS: usize = 5_000;
pub const DLL_SCAN_MAX_VISITED_FILES: usize = 200_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllCandidate {
    /// File name without the `.dll` suffix, original case preserved.
    pub name: String,
    /// Lowercased dedupe key produced by [`dll_name_key`].
    pub key: String,
    /// Path relative to the scan root, used to tell same-named files apart.
    pub relative_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllScanOutcome {
    pub entries: Vec<DllCandidate>,
    pub skipped_non_utf8: usize,
    pub unreadable_dirs: usize,
    pub truncated: bool,
    pub depth_limited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DllScanError {
    #[error("directory not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("path is not a directory: {}", .0.display())]
    NotDirectory(PathBuf),
    #[error("permission denied: {}", .0.display())]
    PermissionDenied(PathBuf),
    #[error("failed to read {}: {message}", path.display())]
    Io { path: PathBuf, message: String },
    #[error("scan cancelled")]
    Cancelled,
    #[error("scan task failed: {0}")]
    Join(String),
}

/// `trim` → strip a case-insensitive `.dll` suffix → `trim_end` → lowercase.
pub fn dll_name_key(name: &str) -> String {
    let trimmed = name.trim();
    let stem = if trimmed.len() >= 4
        && trimmed.as_bytes()[trimmed.len() - 4..].eq_ignore_ascii_case(b".dll")
    {
        trimmed[..trimmed.len() - 4].trim_end()
    } else {
        trimmed
    };
    stem.to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy)]
struct ScanLimits {
    max_depth: usize,
    max_results: usize,
    max_visited_files: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_depth: DLL_SCAN_MAX_DEPTH,
            max_results: DLL_SCAN_MAX_RESULTS,
            max_visited_files: DLL_SCAN_MAX_VISITED_FILES,
        }
    }
}

/// Blocking scan; callers must run it inside `spawn_blocking`.
pub fn scan_dlls(root: &Path, cancel: &AtomicBool) -> Result<DllScanOutcome, DllScanError> {
    scan_with_limits(root, cancel, ScanLimits::default())
}

fn scan_with_limits(
    root: &Path,
    cancel: &AtomicBool,
    limits: ScanLimits,
) -> Result<DllScanOutcome, DllScanError> {
    let metadata = fs::metadata(root).map_err(|error| map_io_error(root, error))?;
    if !metadata.is_dir() {
        return Err(DllScanError::NotDirectory(root.to_path_buf()));
    }

    let mut outcome = DllScanOutcome {
        entries: Vec::new(),
        skipped_non_utf8: 0,
        unreadable_dirs: 0,
        truncated: false,
        depth_limited: false,
    };
    let mut seen = HashSet::new();
    let mut visited_files = 0usize;
    let mut stack = vec![(root.to_path_buf(), 0usize)];

    'walk: while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(DllScanError::Cancelled);
        }

        let read = match fs::read_dir(&dir) {
            Ok(read) => read,
            Err(error) if dir == root => {
                return Err(map_io_error(&dir, error));
            }
            Err(_) => {
                // `read_dir` uses the same error for unreadable directories.
                outcome.unreadable_dirs += 1;
                continue;
            }
        };

        let mut children = Vec::new();
        for entry in read {
            match entry {
                Ok(entry) => children.push(entry),
                Err(_) => outcome.unreadable_dirs += 1,
            }
        }
        children.sort_by_key(|entry| entry.file_name());

        for entry in children {
            if cancel.load(Ordering::Relaxed) {
                return Err(DllScanError::Cancelled);
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            // Never follow symlinks: this also makes directory loops harmless.
            if file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            if file_type.is_dir() {
                if depth >= limits.max_depth {
                    outcome.depth_limited = true;
                } else {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            visited_files += 1;
            if visited_files > limits.max_visited_files {
                outcome.truncated = true;
                break 'walk;
            }
            if !is_dll_file(&path) {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
                outcome.skipped_non_utf8 += 1;
                continue;
            };
            let name = file_name[..file_name.len() - 4].to_string();
            if name.is_empty() {
                continue;
            }
            let Ok(relative_path) = path.strip_prefix(root) else {
                continue;
            };
            if relative_path.to_str().is_none() {
                outcome.skipped_non_utf8 += 1;
                continue;
            }

            let key = dll_name_key(&name);
            if !seen.insert(key.clone()) {
                continue;
            }
            if outcome.entries.len() >= limits.max_results {
                outcome.truncated = true;
                break 'walk;
            }
            outcome.entries.push(DllCandidate {
                name,
                key,
                relative_path: relative_path.to_path_buf(),
            });
        }
    }

    outcome.entries.sort_by(|a, b| {
        a.key
            .cmp(&b.key)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    Ok(outcome)
}

fn is_dll_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
}

fn map_io_error(path: &Path, error: std::io::Error) -> DllScanError {
    match error.kind() {
        std::io::ErrorKind::NotFound => DllScanError::NotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => {
            DllScanError::PermissionDenied(path.to_path_buf())
        }
        _ => DllScanError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        },
    }
}
```

- [ ] **Step 2: 导出模块**

`crates/application/src/lib.rs` 顶部加 `mod dll_scan;`，并新增导出：

```rust
pub use dll_scan::{
    DLL_SCAN_MAX_DEPTH, DLL_SCAN_MAX_RESULTS, DLL_SCAN_MAX_VISITED_FILES, DllCandidate,
    DllScanError, DllScanOutcome, dll_name_key, scan_dlls,
};
```

- [ ] **Step 3: 写单元测试**

在 `dll_scan.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"stub").unwrap();
    }

    fn limits(max_depth: usize, max_results: usize, max_visited_files: usize) -> ScanLimits {
        ScanLimits {
            max_depth,
            max_results,
            max_visited_files,
        }
    }

    #[test]
    fn dll_name_key_strips_suffix_and_normalizes_case() {
        assert_eq!(dll_name_key("d3d11.dll"), "d3d11");
        assert_eq!(dll_name_key("D3D11.DLL"), "d3d11");
        assert_eq!(dll_name_key("  WinMM.dll  "), "winmm");
        assert_eq!(dll_name_key("  D3D11  "), "d3d11");
        assert_eq!(dll_name_key(""), "");
    }

    #[test]
    fn collects_recursively_and_dedupes_by_basename() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("a.dll"));
        touch(&root.path().join("notes.txt"));
        touch(&root.path().join("sub/B.DLL"));
        touch(&root.path().join("sub/deep/a.dll"));

        let outcome = scan_dlls(root.path(), &AtomicBool::new(false)).unwrap();
        let keys: Vec<_> = outcome.entries.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(outcome.entries[0].relative_path, PathBuf::from("a.dll"));
        assert_eq!(outcome.entries[1].name, "B");
        assert!(!outcome.truncated);
        assert!(!outcome.depth_limited);
    }

    #[test]
    fn depth_limit_reports_and_skips_deeper_directories() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("top.dll"));
        touch(&root.path().join("sub/inner.dll"));
        touch(&root.path().join("sub/deep/too_deep.dll"));

        let outcome = scan_with_limits(
            root.path(),
            &AtomicBool::new(false),
            limits(1, 100, 100),
        )
        .unwrap();
        let keys: Vec<_> = outcome.entries.iter().map(|entry| entry.key.as_str()).collect();
        assert_eq!(keys, vec!["inner", "top"]);
        assert!(outcome.depth_limited);
    }

    #[test]
    fn result_cap_sets_truncated() {
        let root = TempDir::new().unwrap();
        for name in ["a.dll", "b.dll", "c.dll"] {
            touch(&root.path().join(name));
        }
        let outcome = scan_with_limits(
            root.path(),
            &AtomicBool::new(false),
            limits(16, 2, 100),
        )
        .unwrap();
        assert_eq!(outcome.entries.len(), 2);
        assert!(outcome.truncated);
    }

    #[test]
    fn visited_file_cap_sets_truncated() {
        let root = TempDir::new().unwrap();
        for name in ["a.txt", "b.dll", "c.dll"] {
            touch(&root.path().join(name));
        }
        let outcome = scan_with_limits(
            root.path(),
            &AtomicBool::new(false),
            limits(16, 100, 1),
        )
        .unwrap();
        assert_eq!(outcome.entries.len(), 1);
        assert_eq!(outcome.entries[0].key, "b");
        assert!(outcome.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_file_names_are_skipped_and_counted() {
        use std::os::unix::ffi::OsStringExt;

        let root = TempDir::new().unwrap();
        let name = std::ffi::OsString::from_vec(b"bad\xFF.dll".to_vec());
        fs::write(root.path().join(name), b"stub").unwrap();

        let outcome = scan_dlls(root.path(), &AtomicBool::new(false)).unwrap();
        assert!(outcome.entries.is_empty());
        assert_eq!(outcome.skipped_non_utf8, 1);
    }

    #[test]
    fn missing_and_non_directory_roots_are_reported() {
        let root = TempDir::new().unwrap();
        let missing = root.path().join("missing");
        assert_eq!(
            scan_dlls(&missing, &AtomicBool::new(false)).unwrap_err(),
            DllScanError::NotFound(missing)
        );

        let file = root.path().join("game.exe");
        fs::write(&file, b"stub").unwrap();
        assert_eq!(
            scan_dlls(&file, &AtomicBool::new(false)).unwrap_err(),
            DllScanError::NotDirectory(file)
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlink_loops_do_not_recurse_forever() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("a.dll"));
        std::os::unix::fs::symlink(root.path(), root.path().join("loop")).unwrap();

        let outcome = scan_dlls(root.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(outcome.entries.len(), 1);
    }

    #[test]
    fn cancellation_is_reported_before_walking() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("a.dll"));
        let cancel = AtomicBool::new(true);
        assert_eq!(
            scan_dlls(root.path(), &cancel).unwrap_err(),
            DllScanError::Cancelled
        );
    }
}
```

- [ ] **Step 4: 运行 application 测试**

Run: `cargo test -p application dll_scan`
Expected: PASS（Unix 上共 8 个测试，非 Unix 跳过 2 个）。

---

### Task 2: 每游戏扫描目录的 settings 辅助函数

**Files:**
- Modify: `crates/application/src/settings.rs:1-49`
- Modify: `crates/application/src/lib.rs`

- [ ] **Step 1: 添加键构造与解析**

`crates/application/src/settings.rs` 顶部补充：

```rust
use std::collections::HashMap;
use std::path::PathBuf;
```

在 `SETTING_*` 常量后添加：

```rust
pub const DLL_SCAN_DIR_SETTING_PREFIX: &str = "dll_scan_dir.";

pub fn dll_scan_dir_setting_key(game_id: Uuid) -> String {
    format!("{DLL_SCAN_DIR_SETTING_PREFIX}{game_id}")
}

/// Parses remembered scan directories, ignoring malformed keys and empty values.
pub fn dll_scan_dirs(settings: &BTreeMap<String, String>) -> HashMap<Uuid, PathBuf> {
    settings
        .iter()
        .filter_map(|(key, value)| {
            let raw = key.strip_prefix(DLL_SCAN_DIR_SETTING_PREFIX)?;
            let id = Uuid::parse_str(raw).ok()?;
            (!value.is_empty()).then(|| (id, PathBuf::from(value)))
        })
        .collect()
}
```

- [ ] **Step 2: 导出**

`crates/application/src/lib.rs` 的 settings 导出列表追加：

```rust
DLL_SCAN_DIR_SETTING_PREFIX, dll_scan_dir_setting_key, dll_scan_dirs,
```

- [ ] **Step 3: 测试**

在 `settings.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_dirs_parse_only_prefixed_valid_uuids() {
        let id = Uuid::new_v4();
        let settings = BTreeMap::from([
            (dll_scan_dir_setting_key(id), "/games/one".to_string()),
            ("dll_scan_dir.not-a-uuid".to_string(), "/x".to_string()),
            (dll_scan_dir_setting_key(Uuid::new_v4()), String::new()),
            ("other".to_string(), "/y".to_string()),
        ]);

        let parsed = dll_scan_dirs(&settings);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.get(&id), Some(&PathBuf::from("/games/one")));
    }
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p application scan_dirs`
Expected: PASS。

---

### Task 3: 文案与错误渲染

**Files:**
- Modify: `crates/ui/locales/zh-CN.json`
- Modify: `crates/ui/locales/en-US.json`
- Modify: `crates/ui/src/i18n/errors.rs`

- [ ] **Step 1: 添加中英文键（两文件键集合、占位符必须一致）**

在 `detail.` 命名空间加入（以 `zh-CN.json` 为例，`en-US.json` 同键位）：

```json
"detail.dll_scan.title": "从目录添加 DLL Override",
"detail.dll_scan.open": "从目录添加",
"detail.dll_scan.scan_dir": "扫描目录：{path}",
"detail.dll_scan.no_dir": "未选择扫描目录",
"detail.dll_scan.change_dir": "更改扫描目录",
"detail.dll_scan.rescan": "重新扫描",
"detail.dll_scan.scanning": "正在扫描目录…",
"detail.dll_scan.need_dir": "请选择要扫描的目录",
"detail.dll_scan.found.one": "找到 {count} 个 DLL",
"detail.dll_scan.found.other": "找到 {count} 个 DLL",
"detail.dll_scan.selected.one": "已选 {count} 个",
"detail.dll_scan.selected.other": "已选 {count} 个",
"detail.dll_scan.skipped_non_utf8.one": "跳过 {count} 个非 UTF-8 路径",
"detail.dll_scan.skipped_non_utf8.other": "跳过 {count} 个非 UTF-8 路径",
"detail.dll_scan.unreadable_dirs.one": "{count} 个子目录无法读取",
"detail.dll_scan.unreadable_dirs.other": "{count} 个子目录无法读取",
"detail.dll_scan.truncated": "扫描范围过大，结果可能不完整，请缩小目录",
"detail.dll_scan.depth_limited": "超过 {depth} 层的目录未扫描",
"detail.dll_scan.search_placeholder": "搜索 DLL 名称…",
"detail.dll_scan.sort_asc": "名称 ↑",
"detail.dll_scan.sort_desc": "名称 ↓",
"detail.dll_scan.select_all": "全选",
"detail.dll_scan.invert": "反选",
"detail.dll_scan.clear": "清空",
"detail.dll_scan.mode": "模式",
"detail.dll_scan.added_badge": "已添加",
"detail.dll_scan.empty": "该目录下未找到 .dll 文件",
"detail.dll_scan.no_matches": "没有匹配的 DLL",
"detail.dll_scan.no_selection": "请先勾选要添加的 DLL",
"detail.dll_scan.confirm": "确认添加",
"detail.dll_scan.applying": "正在保存…",
"detail.dll_scan.cancel_scan": "取消扫描",
"detail.dll_scan.added.one": "已添加 {count} 个 DLL Override",
"detail.dll_scan.added.other": "已添加 {count} 个 DLL Override",
"detail.dll_scan.render_limited": "仅显示前 {count} 条，请使用搜索缩小范围",
"widgets.success": "成功",
"errors.dll_scan.not_found": "目录不存在：{path}",
"errors.dll_scan.not_directory": "路径不是目录：{path}",
"errors.dll_scan.permission_denied": "没有读取权限：{path}",
"errors.dll_scan.io": "读取 {path} 失败：{detail}",
"errors.dll_scan.join": "扫描任务异常终止：{detail}",
"errors.dll_scan.cancelled": "扫描已取消"
```

`en-US.json` 对应：

```json
"detail.dll_scan.title": "Add DLL Overrides from a directory",
"detail.dll_scan.open": "Add from directory",
"detail.dll_scan.scan_dir": "Scan directory: {path}",
"detail.dll_scan.no_dir": "No scan directory selected",
"detail.dll_scan.change_dir": "Change directory",
"detail.dll_scan.rescan": "Rescan",
"detail.dll_scan.scanning": "Scanning directory…",
"detail.dll_scan.need_dir": "Choose a directory to scan",
"detail.dll_scan.found.one": "Found {count} DLL",
"detail.dll_scan.found.other": "Found {count} DLLs",
"detail.dll_scan.selected.one": "{count} selected",
"detail.dll_scan.selected.other": "{count} selected",
"detail.dll_scan.skipped_non_utf8.one": "Skipped {count} non-UTF-8 path",
"detail.dll_scan.skipped_non_utf8.other": "Skipped {count} non-UTF-8 paths",
"detail.dll_scan.unreadable_dirs.one": "{count} subdirectory could not be read",
"detail.dll_scan.unreadable_dirs.other": "{count} subdirectories could not be read",
"detail.dll_scan.truncated": "The scan is too large; results may be incomplete. Narrow the directory.",
"detail.dll_scan.depth_limited": "Directories deeper than {depth} levels were not scanned",
"detail.dll_scan.search_placeholder": "Search DLL name…",
"detail.dll_scan.sort_asc": "Name ↑",
"detail.dll_scan.sort_desc": "Name ↓",
"detail.dll_scan.select_all": "Select all",
"detail.dll_scan.invert": "Invert",
"detail.dll_scan.clear": "Clear",
"detail.dll_scan.mode": "Mode",
"detail.dll_scan.added_badge": "Added",
"detail.dll_scan.empty": "No .dll files found in this directory",
"detail.dll_scan.no_matches": "No DLL matches the search",
"detail.dll_scan.no_selection": "Select at least one DLL first",
"detail.dll_scan.confirm": "Add selected",
"detail.dll_scan.applying": "Saving…",
"detail.dll_scan.cancel_scan": "Cancel scan",
"detail.dll_scan.added.one": "Added {count} DLL override",
"detail.dll_scan.added.other": "Added {count} DLL overrides",
"detail.dll_scan.render_limited": "Showing the first {count} entries; use search to narrow the list",
"widgets.success": "Success",
"errors.dll_scan.not_found": "Directory not found: {path}",
"errors.dll_scan.not_directory": "Path is not a directory: {path}",
"errors.dll_scan.permission_denied": "Permission denied: {path}",
"errors.dll_scan.io": "Failed to read {path}: {detail}",
"errors.dll_scan.join": "The scan task failed: {detail}",
"errors.dll_scan.cancelled": "Scan cancelled"
```

- [ ] **Step 2: 错误渲染函数**

`crates/ui/src/i18n/errors.rs` 追加：

```rust
pub fn dll_scan_error(lang: Lang, error: &application::DllScanError) -> String {
    use application::DllScanError;

    match error {
        DllScanError::NotFound(path) => tr_args(
            lang,
            "errors.dll_scan.not_found",
            &[("path", &path.display().to_string())],
        ),
        DllScanError::NotDirectory(path) => tr_args(
            lang,
            "errors.dll_scan.not_directory",
            &[("path", &path.display().to_string())],
        ),
        DllScanError::PermissionDenied(path) => tr_args(
            lang,
            "errors.dll_scan.permission_denied",
            &[("path", &path.display().to_string())],
        ),
        DllScanError::Io { path, message } => tr_args(
            lang,
            "errors.dll_scan.io",
            &[
                ("path", &path.display().to_string()),
                ("detail", message),
            ],
        ),
        DllScanError::Join(detail) => {
            tr_args(lang, "errors.dll_scan.join", &[("detail", detail)])
        }
        DllScanError::Cancelled => tr(lang, "errors.dll_scan.cancelled").to_owned(),
    }
}
```

- [ ] **Step 3: 错误渲染测试**

在 `errors.rs` 的 `tests` 模块追加：

```rust
#[test]
fn scan_errors_render_paths_in_both_languages() {
    let error = application::DllScanError::NotFound(PathBuf::from("/games/missing"));
    assert_eq!(dll_scan_error(&EN_US, &error), "Directory not found: /games/missing");
    assert_eq!(dll_scan_error(&ZH_CN, &error), "目录不存在：/games/missing");
}
```

- [ ] **Step 4: 运行 i18n 测试（键集合/占位符）**

Run: `cargo test -p ui i18n`
Expected: PASS，`catalogues_share_keys_and_placeholders` 与 `keys_used_in_source_exist` 均通过。

---

### Task 4: UI 对话框状态模块

**Files:**
- Create: `crates/ui/src/app/dll_scan.rs`
- Modify: `crates/ui/src/app/mod.rs`

- [ ] **Step 1: 创建状态与纯函数**

创建 `crates/ui/src/app/dll_scan.rs`：

```rust
use super::{App, Message};
use application::editor::DllRow;
use application::{
    DllCandidate, DllScanError, DllScanOutcome, OverrideMode, dll_name_key, scan_dlls,
};
use iced::Task;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

/// Bounds the number of rows laid out at once; iced has no virtualization.
pub const DLL_SCAN_RENDER_LIMIT: usize = 300;

#[derive(Debug)]
pub struct DllScanDialog {
    pub game_id: Uuid,
    pub root: Option<PathBuf>,
    pub phase: DllScanPhase,
    pub query: String,
    pub sort_desc: bool,
    pub selected: HashSet<String>,
    pub mode: OverrideMode,
    pub existing: HashSet<String>,
    pub scan_seq: u64,
    pub cancel: Arc<AtomicBool>,
    pub applying: bool,
    pub apply_error: Option<String>,
    pub pending_apply: Option<PendingApply>,
}

#[derive(Debug)]
pub enum DllScanPhase {
    NeedDirectory,
    Scanning,
    Ready(DllScanOutcome),
    Failed(DllScanError),
}

#[derive(Debug, Clone)]
pub struct PendingApply {
    pub original_rows: Vec<DllRow>,
    pub added: Vec<(String, OverrideMode)>,
    pub revision: u64,
}

pub fn existing_keys(rows: &[DllRow]) -> HashSet<String> {
    rows.iter()
        .map(|row| dll_name_key(&row.name))
        .filter(|key| !key.is_empty())
        .collect()
}

impl DllScanDialog {
    pub fn new(game_id: Uuid, root: Option<PathBuf>, existing: HashSet<String>) -> Self {
        let phase = if root.is_some() {
            DllScanPhase::Scanning
        } else {
            DllScanPhase::NeedDirectory
        };
        Self {
            game_id,
            root,
            phase,
            query: String::new(),
            sort_desc: false,
            selected: HashSet::new(),
            mode: OverrideMode::NativeBuiltin,
            existing,
            scan_seq: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            applying: false,
            apply_error: None,
            pending_apply: None,
        }
    }

    /// Cancels the previous scan and starts a new generation.
    pub fn next_scan(&mut self) -> Option<(u64, PathBuf, Arc<AtomicBool>)> {
        self.cancel.store(true, Ordering::Relaxed);
        let root = self.root.clone()?;
        self.scan_seq = self.scan_seq.wrapping_add(1);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.phase = DllScanPhase::Scanning;
        self.selected.clear();
        self.apply_error = None;
        Some((self.scan_seq, root, Arc::clone(&self.cancel)))
    }

    pub fn cancel_scan(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Filtered, sorted and capped rows used by both the list and the
    /// select-all/invert operations.
    pub fn filtered(&self) -> Vec<&DllCandidate> {
        let DllScanPhase::Ready(outcome) = &self.phase else {
            return Vec::new();
        };
        let needle = self.query.trim().to_ascii_lowercase();
        let mut rows: Vec<&DllCandidate> = outcome
            .entries
            .iter()
            .filter(|entry| needle.is_empty() || entry.name.to_ascii_lowercase().contains(&needle))
            .collect();
        rows.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
        if self.sort_desc {
            rows.reverse();
        }
        rows.truncate(DLL_SCAN_RENDER_LIMIT);
        rows
    }

    pub fn toggle(&mut self, key: String) {
        if self.existing.contains(&key) {
            return;
        }
        if !self.selected.remove(&key) {
            self.selected.insert(key);
        }
    }

    pub fn select_all_filtered(&mut self) {
        let keys: Vec<String> = self
            .filtered()
            .iter()
            .filter(|entry| !self.existing.contains(&entry.key))
            .map(|entry| entry.key.clone())
            .collect();
        self.selected.extend(keys);
    }

    pub fn invert_filtered(&mut self) {
        let keys: Vec<String> = self
            .filtered()
            .iter()
            .filter(|entry| !self.existing.contains(&entry.key))
            .map(|entry| entry.key.clone())
            .collect();
        for key in keys {
            if !self.selected.remove(&key) {
                self.selected.insert(key);
            }
        }
    }
}

impl App {
    pub(crate) fn open_dll_scan(&mut self) -> Task<Message> {
        if self.dll_scan.is_some() {
            return Task::none();
        }
        let Some(draft) = self.draft.as_ref() else {
            return Task::none();
        };
        if draft.platform.is_native() {
            return Task::none();
        }

        let game_id = draft.id;
        let root = self.scan_dirs.get(&game_id).cloned().or_else(|| {
            draft
                .executable_path
                .as_ref()
                .and_then(|path| path.parent())
                .map(Path::to_path_buf)
        });
        let existing = existing_keys(&draft.dll_rows);
        self.dll_scan = Some(DllScanDialog::new(game_id, root, existing));
        self.start_dll_scan()
    }

    fn start_dll_scan(&mut self) -> Task<Message> {
        let Some(dialog) = self.dll_scan.as_mut() else {
            return Task::none();
        };
        let Some((seq, root, cancel)) = dialog.next_scan() else {
            dialog.phase = DllScanPhase::NeedDirectory;
            return Task::none();
        };
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || scan_dlls(&root, &cancel))
                    .await
                    .unwrap_or_else(|error| Err(DllScanError::Join(error.to_string())))
            },
            move |result| Message::DllScanFinished { seq, result },
        )
    }

    pub(crate) fn close_dll_scan(&mut self) {
        if let Some(dialog) = self.dll_scan.as_ref() {
            if dialog.applying {
                return;
            }
            dialog.cancel_scan();
        }
        self.dll_scan = None;
    }

    pub(crate) fn dll_scan_finished(
        &mut self,
        seq: u64,
        result: Result<DllScanOutcome, DllScanError>,
    ) -> Task<Message> {
        let Some(dialog) = self.dll_scan.as_mut() else {
            return Task::none();
        };
        if dialog.scan_seq != seq || !matches!(dialog.phase, DllScanPhase::Scanning) {
            return Task::none();
        }
        match result {
            Ok(outcome) => {
                let game_id = dialog.game_id;
                let root = dialog.root.clone();
                dialog.phase = DllScanPhase::Ready(outcome);
                dialog.apply_error = None;
                match root {
                    Some(root) => {
                        self.scan_dirs.insert(game_id, root.clone());
                        self.persist_dll_scan_dir(game_id, &root)
                    }
                    None => Task::none(),
                }
            }
            Err(DllScanError::Cancelled) => Task::none(),
            Err(error) => {
                dialog.phase = DllScanPhase::Failed(error);
                Task::none()
            }
        }
    }

    fn persist_dll_scan_dir(&self, game_id: Uuid, root: &Path) -> Task<Message> {
        let Some(service) = self.service.clone() else {
            return Task::none();
        };
        let Some(path) = root.to_str().map(str::to_owned) else {
            return Task::none();
        };
        let key = application::dll_scan_dir_setting_key(game_id);
        let values = std::collections::BTreeMap::from([(key, Some(path))]);
        Task::perform(service.save_settings(values), Message::DllScanDirSaved)
    }

    pub(crate) fn confirm_dll_scan(&mut self) -> Task<Message> {
        if self.pending.saving || self.pending.deleting {
            return Task::none();
        }
        let lang = self.lang;
        let Some(dialog) = self.dll_scan.as_ref() else {
            return Task::none();
        };
        if dialog.applying {
            return Task::none();
        }
        let DllScanPhase::Ready(outcome) = &dialog.phase else {
            return Task::none();
        };
        let game_id = dialog.game_id;
        let mode = dialog.mode;
        let mut added: Vec<(String, OverrideMode)> = outcome
            .entries
            .iter()
            .filter(|entry| {
                dialog.selected.contains(&entry.key) && !dialog.existing.contains(&entry.key)
            })
            .map(|entry| (entry.name.clone(), mode))
            .collect();
        added.sort_by(|a, b| dll_name_key(&a.0).cmp(&dll_name_key(&b.0)));

        if added.is_empty() {
            if let Some(dialog) = self.dll_scan.as_mut() {
                dialog.apply_error =
                    Some(crate::i18n::tr(lang, "detail.dll_scan.no_selection").to_owned());
            }
            return Task::none();
        }

        let Some(draft) = self.draft.as_mut() else {
            return Task::none();
        };
        if draft.id != game_id {
            return Task::none();
        }
        let original_rows = draft.dll_rows.clone();
        draft.dll_rows.extend(added.iter().map(|(name, mode)| DllRow {
            name: name.clone(),
            mode: *mode,
        }));
        self.touch_draft();

        let validation = self.draft.as_ref().map(|draft| draft.to_game());
        if let Some(Err(error)) = validation {
            if let Some(draft) = self.draft.as_mut() {
                draft.dll_rows = original_rows;
            }
            self.touch_draft();
            if let Some(dialog) = self.dll_scan.as_mut() {
                dialog.apply_error =
                    Some(crate::i18n::errors::draft_error(lang, &error));
            }
            return Task::none();
        }
        if self.service.is_none() {
            if let Some(draft) = self.draft.as_mut() {
                draft.dll_rows = original_rows;
            }
            self.touch_draft();
            if let Some(dialog) = self.dll_scan.as_mut() {
                dialog.apply_error = Some(
                    crate::i18n::tr(lang, "app.error.database_unavailable").to_owned(),
                );
            }
            return Task::none();
        }

        let revision = self.draft_revision;
        if let Some(dialog) = self.dll_scan.as_mut() {
            dialog.pending_apply = Some(PendingApply {
                original_rows,
                added,
                revision,
            });
            dialog.applying = true;
            dialog.apply_error = None;
        }
        self.save_draft()
    }
}
```

- [ ] **Step 2: 在 app 模块中挂载并新增字段**

`crates/ui/src/app/mod.rs`：

1. 顶部 `mod message;` 前加 `mod dll_scan;`
2. `pub use` 增加：

```rust
pub use dll_scan::{DllScanDialog, DllScanPhase, DLL_SCAN_RENDER_LIMIT};
```

3. `App` 字段（`confirming` 附近）加：

```rust
pub dll_scan: Option<DllScanDialog>,
pub scan_dirs: HashMap<Uuid, PathBuf>,
pub success: Option<String>,
```

4. `App::new` 初始化：

```rust
dll_scan: None,
scan_dirs: HashMap::new(),
success: None,
```

5. `apply_loaded` 在 `self.winetricks_path = ...` 之后加：

```rust
self.scan_dirs = application::dll_scan_dirs(&data.settings_map);
```

- [ ] **Step 3: 修改 `apply_draft_saved` 处理确认结果**

`crates/ui/src/app/mod.rs` 的 `apply_draft_saved`：

在 `self.pending.saving = false;` 之后取出本次保存对应的确认任务：

```rust
let dll_pending = self.dll_scan.as_mut().and_then(|dialog| {
    let pending = dialog.pending_apply.as_ref()?;
    (dialog.game_id == game_id && pending.revision == revision)
        .then(|| dialog.pending_apply.take().expect("checked above"))
});
```

在 `Ok(game) => {` 分支内、`let current = ...` 之前加入：

```rust
if let Some(pending) = dll_pending {
    let count = pending.added.len();
    self.dll_scan = None;
    self.success = Some(self.tr_plural_args(
        "detail.dll_scan.added.one",
        "detail.dll_scan.added.other",
        count as u64,
        &[("count", &count.to_string())],
    ));
}
```

在 `Err(error) => {` 分支开头加入：

```rust
if let Some(pending) = dll_pending {
    if let Some(draft) = self.draft.as_mut() {
        if draft.id == game_id {
            draft.dll_rows = pending.original_rows;
        }
    }
    self.touch_draft();
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.applying = false;
        dialog.pending_apply = None;
        dialog.apply_error = Some(error);
    }
    return Task::none();
}
```

（注意：该分支不再落到原来的全局错误提示。）

- [ ] **Step 4: 添加 UI 状态测试**

在 `app/dll_scan.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::i18n::LanguagePref;
    use application::editor::GameDraft;
    use application::{AppPaths, AppService, Platform};
    use std::collections::BTreeMap;

    fn app() -> App {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-dll-scan-test"));
        let (mut app, _) = App::new(paths, Err(crate::app::ServiceError::DatabaseLocked));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::ZH_CN);
        app.lang = &crate::i18n::ZH_CN;
        app
    }

    fn candidate(name: &str, relative: &str) -> DllCandidate {
        DllCandidate {
            name: name.to_owned(),
            key: dll_name_key(name),
            relative_path: PathBuf::from(relative),
        }
    }

    fn ready_dialog(entries: Vec<DllCandidate>) -> DllScanDialog {
        let mut dialog = DllScanDialog::new(Uuid::new_v4(), Some(PathBuf::from("/games")), HashSet::new());
        dialog.phase = DllScanPhase::Ready(DllScanOutcome {
            entries,
            skipped_non_utf8: 0,
            unreadable_dirs: 0,
            truncated: false,
            depth_limited: false,
        });
        dialog
    }

    #[test]
    fn existing_keys_use_the_shared_normalizer() {
        let rows = vec![
            DllRow { name: "D3D11.dll".into(), mode: OverrideMode::Native },
            DllRow { name: "  ".into(), mode: OverrideMode::Native },
        ];
        let keys = existing_keys(&rows);
        assert!(keys.contains("d3d11"));
        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn search_sort_and_selection_respect_visibility_and_existing() {
        let mut dialog = ready_dialog(vec![
            candidate("dxgi", "a/dxgi.dll"),
            candidate("d3d11", "d3d11.dll"),
            candidate("winmm", "b/winmm.dll"),
        ]);
        dialog.existing.insert("winmm".into());

        assert_eq!(
            dialog.filtered().iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(),
            vec!["d3d11", "dxgi", "winmm"]
        );
        dialog.sort_desc = true;
        assert_eq!(dialog.filtered()[0].name, "winmm");

        dialog.query = "d3".into();
        assert_eq!(dialog.filtered().len(), 1);
        dialog.select_all_filtered();
        assert_eq!(dialog.selected, HashSet::from(["d3d11".to_string()]));

        dialog.query.clear();
        dialog.select_all_filtered();
        assert_eq!(dialog.selected.len(), 2);
        assert!(!dialog.selected.contains("winmm"));

        dialog.invert_filtered();
        assert!(dialog.selected.is_empty());
    }

    #[test]
    fn opening_prefers_the_remembered_directory() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        let id = draft.id;
        app.scan_dirs.insert(id, PathBuf::from("/remembered"));
        app.draft = Some(draft);

        let _ = app.open_dll_scan();

        let dialog = app.dll_scan.as_ref().expect("dialog opens");
        assert_eq!(dialog.root.as_deref(), Some(Path::new("/remembered")));
        assert!(matches!(dialog.phase, DllScanPhase::Scanning));
        assert_eq!(dialog.scan_seq, 1);
    }

    #[test]
    fn opening_without_any_directory_needs_one() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/game.exe"), Platform::Windows);
        draft.executable_path = None;
        app.draft = Some(draft);

        let _ = app.open_dll_scan();

        let dialog = app.dll_scan.as_ref().expect("dialog opens");
        assert_eq!(dialog.root, None);
        assert!(matches!(dialog.phase, DllScanPhase::NeedDirectory));
    }

    #[test]
    fn stale_scan_results_are_ignored() {
        let mut app = app();
        app.dll_scan = Some(ready_dialog(Vec::new()));
        app.dll_scan.as_mut().unwrap().scan_seq = 2;
        app.dll_scan.as_mut().unwrap().phase = DllScanPhase::Scanning;
        let outcome = DllScanOutcome {
            entries: vec![candidate("d3d11", "d3d11.dll")],
            skipped_non_utf8: 0,
            unreadable_dirs: 0,
            truncated: false,
            depth_limited: false,
        };

        let _ = app.dll_scan_finished(1, Ok(outcome));

        assert!(matches!(app.dll_scan.as_ref().unwrap().phase, DllScanPhase::Scanning));
    }

    #[test]
    fn confirm_appends_selected_rows_and_marks_applying() {
        let paths = AppPaths::for_root(PathBuf::from("/tmp/gamehub-dll-scan-confirm"));
        let service = AppService::open_in_memory().unwrap();
        let (mut app, _) = App::new(paths, Ok(service));
        app.settings.language = LanguagePref::Fixed(&crate::i18n::ZH_CN);
        app.lang = &crate::i18n::ZH_CN;

        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        let id = draft.id;
        app.draft = Some(draft);

        let mut dialog = ready_dialog(vec![
            candidate("d3d11", "d3d11.dll"),
            candidate("dxgi", "dxgi.dll"),
        ]);
        dialog.game_id = id;
        dialog.selected = HashSet::from(["d3d11".to_string(), "dxgi".to_string()]);
        dialog.mode = OverrideMode::Native;
        app.dll_scan = Some(dialog);

        let _ = app.confirm_dll_scan();

        let draft = app.draft.as_ref().unwrap();
        assert_eq!(draft.dll_rows.len(), 2);
        let dialog = app.dll_scan.as_ref().unwrap();
        assert!(dialog.applying);
        assert_eq!(dialog.pending_apply.as_ref().unwrap().added.len(), 2);
    }

    #[test]
    fn failed_save_rolls_back_rows_and_keeps_selection() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        let id = draft.id;
        app.draft = Some(draft);
        app.draft_revision = 5;

        let mut dialog = ready_dialog(Vec::new());
        dialog.game_id = id;
        dialog.applying = true;
        dialog.selected = HashSet::from(["d3d11".to_string()]);
        dialog.pending_apply = Some(PendingApply {
            original_rows: Vec::new(),
            added: vec![("d3d11".into(), OverrideMode::Native)],
            revision: 5,
        });
        app.dll_scan = Some(dialog);
        app.draft.as_mut().unwrap().dll_rows.push(DllRow {
            name: "d3d11".into(),
            mode: OverrideMode::Native,
        });

        let _ = app.apply_draft_saved(id, 5, false, Err("db locked".into()));

        assert!(app.draft.as_ref().unwrap().dll_rows.is_empty());
        let dialog = app.dll_scan.as_ref().unwrap();
        assert!(!dialog.applying);
        assert_eq!(dialog.apply_error.as_deref(), Some("db locked"));
        assert!(dialog.selected.contains("d3d11"));
    }

    #[test]
    fn successful_save_closes_the_dialog_and_reports_the_count() {
        let mut app = app();
        let mut draft = GameDraft::new(PathBuf::from("/games/one/game.exe"), Platform::Windows);
        draft.name = "One".into();
        draft.dll_rows.push(DllRow {
            name: "d3d11".into(),
            mode: OverrideMode::Native,
        });
        let id = draft.id;
        let game = draft.to_game().unwrap();
        app.draft = Some(draft);
        app.draft_revision = 5;

        let mut dialog = ready_dialog(Vec::new());
        dialog.game_id = id;
        dialog.applying = true;
        dialog.pending_apply = Some(PendingApply {
            original_rows: Vec::new(),
            added: vec![("d3d11".into(), OverrideMode::Native)],
            revision: 5,
        });
        app.dll_scan = Some(dialog);

        let _ = app.apply_draft_saved(id, 5, false, Ok(game));

        assert!(app.dll_scan.is_none());
        assert_eq!(app.success.as_deref(), Some("已添加 1 个 DLL Override"));
        assert_eq!(app.draft.as_ref().unwrap().dll_rows.len(), 1);
    }

    #[test]
    fn closing_cancels_a_running_scan_but_not_while_saving() {
        let mut app = app();
        let mut dialog = ready_dialog(Vec::new());
        dialog.phase = DllScanPhase::Scanning;
        app.dll_scan = Some(dialog);
        app.close_dll_scan();
        assert!(app.dll_scan.is_none());

        let mut dialog = ready_dialog(Vec::new());
        dialog.applying = true;
        app.dll_scan = Some(dialog);
        app.close_dll_scan();
        assert!(app.dll_scan.is_some());
    }

    #[test]
    fn scan_dir_parser_is_independent_of_loaded_games() {
        let map = BTreeMap::from([(
            application::dll_scan_dir_setting_key(Uuid::nil()),
            "/games".to_string(),
        )]);
        assert_eq!(application::dll_scan_dirs(&map).len(), 1);
    }
}
```

- [ ] **Step 5: 编译（消息尚未添加，先只检查 application）**

Run: `cargo test -p application`
Expected: PASS。UI 步骤会在 Task 5 后编译。

---

### Task 5: 消息、update、成功提示与目录选择

**Files:**
- Modify: `crates/ui/src/app/message.rs`
- Modify: `crates/ui/src/app/update.rs`
- Modify: `crates/ui/src/app/view.rs`
- Modify: `crates/ui/src/dialogs.rs`
- Modify: `crates/ui/src/widgets/mod.rs`

- [ ] **Step 1: 新增消息**

`crates/ui/src/app/message.rs` 顶部导入：

```rust
use application::{DllScanError, DllScanOutcome, OverrideMode};
```

`Message` 枚举末尾追加：

```rust
OpenDllScan,
CloseDllScan,
DllScanPickDir,
DllScanQueryChanged(String),
DllScanToggle(String),
DllScanSelectAll,
DllScanInvert,
DllScanClear,
DllScanSortToggled,
DllScanModeChanged(OverrideMode),
DllScanRescan,
DllScanCancelScan,
DllScanFinished {
    seq: u64,
    result: Result<DllScanOutcome, DllScanError>,
},
DllScanConfirm,
DllScanDirSaved(Result<(), String>),
DismissSuccess,
```

- [ ] **Step 2: update 分发**

`crates/ui/src/app/update.rs` 在 `Message::DismissError` 之前插入：

```rust
Message::OpenDllScan => self.open_dll_scan(),
Message::CloseDllScan | Message::DllScanCancelScan => {
    self.close_dll_scan();
    Task::none()
}
Message::DllScanPickDir => {
    crate::dialogs::pick(crate::dialogs::DialogIntent::PickDllScanDir, self.lang)
}
Message::DllScanQueryChanged(query) => {
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.query = query;
    }
    Task::none()
}
Message::DllScanToggle(key) => {
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.toggle(key);
    }
    Task::none()
}
Message::DllScanSelectAll => {
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.select_all_filtered();
    }
    Task::none()
}
Message::DllScanInvert => {
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.invert_filtered();
    }
    Task::none()
}
Message::DllScanClear => {
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.selected.clear();
    }
    Task::none()
}
Message::DllScanSortToggled => {
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.sort_desc = !dialog.sort_desc;
    }
    Task::none()
}
Message::DllScanModeChanged(mode) => {
    if let Some(dialog) = self.dll_scan.as_mut() {
        dialog.mode = mode;
    }
    Task::none()
}
Message::DllScanRescan => {
    if self.dll_scan.as_ref().is_some_and(|dialog| dialog.applying) {
        Task::none()
    } else {
        self.start_dll_scan()
    }
}
Message::DllScanFinished { seq, result } => self.dll_scan_finished(seq, result),
Message::DllScanConfirm => self.confirm_dll_scan(),
Message::DllScanDirSaved(result) => {
    if let Err(error) = result {
        tracing::warn!(%error, "failed to persist DLL scan directory");
    }
    Task::none()
}
Message::DismissSuccess => {
    self.success = None;
    Task::none()
}
```

`start_dll_scan` 目前是私有方法，同模块子模块可访问；在 `impl App` 中保持 `fn start_dll_scan` 即可。

- [ ] **Step 3: 目录选择 intent**

`crates/ui/src/dialogs.rs` 的 `DialogIntent` 加 `PickDllScanDir`，并把它加入 `pick_folder()` 的分支组：

```rust
DialogIntent::PickWorkingDir
    | DialogIntent::PickProtonDir
    | DialogIntent::PickPrefixDir
    | DialogIntent::PickDxvkDir
    | DialogIntent::PickDxvkNvapiDir
    | DialogIntent::PickPrefixRoot
    | DialogIntent::PickDllScanDir => dialog.pick_folder(),
```

`crates/ui/src/app/mod.rs` 的 `apply_dialog_result` 加分支：

```rust
DialogIntent::PickDllScanDir => {
    if let Some(path) = path {
        if let Some(dialog) = self.dll_scan.as_mut() {
            dialog.root = Some(path);
            dialog.query.clear();
            dialog.selected.clear();
            dialog.sort_desc = false;
            dialog.apply_error = None;
        }
        return self.start_dll_scan();
    }
}
```

- [ ] **Step 4: 成功提示条**

`crates/ui/src/widgets/mod.rs` 在 `error_banner` 之后添加：

```rust
pub fn success_banner(lang: crate::i18n::Lang, message: &str) -> Element<'_, Message> {
    let content = iced::widget::row![
        badge(crate::i18n::tr(lang, "widgets.success"), Tone::Live),
        text(message.to_owned())
            .size(14)
            .font(font(Weight::Semibold)),
        iced::widget::horizontal_space(),
        quiet_small(crate::i18n::tr(lang, "common.close")).on_press(Message::DismissSuccess),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(|theme| {
            let (_, success) = styles::tone_colors(theme, Tone::Live);
            container::Style {
                background: Some(crate::theme::tokens(theme).surface.into()),
                border: Border {
                    color: success,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}
```

- [ ] **Step 5: 视图层挂载**

`crates/ui/src/app/view.rs`：

1. 在 `if let Some(error) = &self.error` 之后加入成功提示：

```rust
if let Some(success) = &self.success {
    main = main.push(crate::widgets::success_banner(self.lang, success));
}
```

2. 在 `if let Some(target) = &self.confirming` 之后加入：

```rust
if let Some(dialog) = &self.dll_scan {
    layers = layers.push(crate::views::dll_scan::view(self.lang, dialog));
}
```

- [ ] **Step 6: 编译 UI（视图模块尚未创建，先创建空壳）**

创建 `crates/ui/src/views/dll_scan.rs` 暂时只放：

```rust
use crate::app::{DllScanDialog, Message};
use iced::Element;

pub fn view<'a>(_lang: crate::i18n::Lang, _dialog: &'a DllScanDialog) -> Element<'a, Message> {
    iced::widget::text("TODO").into()
}
```

`crates/ui/src/views/mod.rs` 加 `pub mod dll_scan;`。

Run: `cargo build -p ui`
Expected: PASS（成功提示条与消息接线编译通过）。

---

### Task 6: 模态视图、复选框样式、详情页入口

**Files:**
- Modify: `crates/ui/src/views/dll_scan.rs`
- Modify: `crates/ui/src/styles.rs`
- Modify: `crates/ui/src/views/game_detail.rs:681-721`

- [ ] **Step 1: 复选框样式**

`crates/ui/src/styles.rs` 的 `use iced::widget::{...}` 中加入 `checkbox as checkbox_widget`，并追加：

```rust
pub fn checkbox(theme: &Theme, status: checkbox_widget::Status) -> checkbox_widget::Style {
    let t = tokens(theme);
    let (is_checked, is_hovered, is_disabled) = match status {
        checkbox_widget::Status::Active { is_checked } => (is_checked, false, false),
        checkbox_widget::Status::Hovered { is_checked } => (is_checked, true, false),
        checkbox_widget::Status::Disabled { is_checked } => (is_checked, false, true),
    };

    let (background, border_color, icon_color) = if is_disabled {
        (
            t.surface_alt,
            t.border,
            if is_checked { t.text_faint } else { Color::TRANSPARENT },
        )
    } else if is_checked {
        (t.primary, t.primary, t.on_primary)
    } else {
        (
            t.surface,
            if is_hovered { t.border_strong } else { t.border },
            Color::TRANSPARENT,
        )
    };

    checkbox_widget::Style {
        background: background.into(),
        icon_color,
        border: border(border_color, 1.0, 5.0),
        text_color: Some(if is_disabled { t.text_faint } else { t.text }),
    }
}
```

- [ ] **Step 2: 详情页入口**

`crates/ui/src/views/game_detail.rs` 的 `dll_card` 中，把 `rows = rows.push(controls::secondary_small(...))` 改为一行两个按钮：

```rust
rows = rows.push(
    row![
        controls::secondary_small(app.tr("detail.dll_scan.open"))
            .on_press(Message::OpenDllScan),
        controls::secondary_small(app.tr("detail.add_dll"))
            .on_press(Message::Draft(DraftAction::Dll(DllAction::Add))),
    ]
    .spacing(8),
);
```

- [ ] **Step 3: 完整模态视图**

替换 `crates/ui/src/views/dll_scan.rs`：

```rust
use crate::app::{DllScanDialog, DllScanPhase, Message, DLL_SCAN_RENDER_LIMIT};
use crate::styles;
use crate::theme::font;
use crate::widgets::{badge, controls, layout, override_mode_choice, override_mode_choices, Tone};
use iced::font::Weight;
use iced::widget::{
    checkbox, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length};

pub fn view<'a>(lang: crate::i18n::Lang, dialog: &'a DllScanDialog) -> Element<'a, Message> {
    let header = row![
        text(crate::i18n::tr(lang, "detail.dll_scan.title"))
            .font(font(Weight::Bold))
            .size(18),
        iced::widget::horizontal_space(),
        controls::quiet(crate::i18n::tr(lang, "common.close"))
            .on_press(Message::CloseDllScan),
    ]
    .spacing(10)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let directory = match dialog.root.as_deref() {
        Some(root) => row![
            text(crate::i18n::tr_args(
                lang,
                "detail.dll_scan.scan_dir",
                &[("path", &root.display().to_string())],
            ))
            .size(11)
            .font(font(Weight::Normal))
            .width(Length::Fill),
            controls::secondary_small(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                .on_press(Message::DllScanPickDir),
            controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.rescan"))
                .on_press(Message::DllScanRescan),
        ],
        None => row![
            layout::faint(crate::i18n::tr(lang, "detail.dll_scan.no_dir")).size(11),
            iced::widget::horizontal_space(),
            controls::secondary_small(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                .on_press(Message::DllScanPickDir),
        ],
    }
    .spacing(8)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let body: Element<'a, Message> = match &dialog.phase {
        DllScanPhase::NeedDirectory => column![
            layout::muted(crate::i18n::tr(lang, "detail.dll_scan.need_dir")).size(13),
            row![
                controls::secondary(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                    .on_press(Message::DllScanPickDir),
                controls::quiet(crate::i18n::tr(lang, "common.close"))
                    .on_press(Message::CloseDllScan),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into(),
        DllScanPhase::Scanning => column![
            layout::muted(crate::i18n::tr(lang, "detail.dll_scan.scanning")).size(13),
            controls::secondary(crate::i18n::tr(lang, "detail.dll_scan.cancel_scan"))
                .on_press(Message::DllScanCancelScan),
        ]
        .spacing(12)
        .into(),
        DllScanPhase::Failed(error) => column![
            text(crate::i18n::errors::dll_scan_error(lang, error))
                .size(13)
                .font(font(Weight::Normal))
                .style(|theme| iced::widget::text::Style {
                    color: Some(crate::styles::tone_colors(theme, Tone::Danger).1),
                })
                .width(Length::Fill),
            row![
                controls::secondary(crate::i18n::tr(lang, "detail.dll_scan.change_dir"))
                    .on_press(Message::DllScanPickDir),
                controls::quiet(crate::i18n::tr(lang, "detail.dll_scan.rescan"))
                    .on_press(Message::DllScanRescan),
                controls::quiet(crate::i18n::tr(lang, "common.close"))
                    .on_press(Message::CloseDllScan),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into(),
        DllScanPhase::Ready(outcome) => ready_body(lang, dialog, outcome),
    };

    let card = container(
        column![header, directory, body]
            .spacing(12)
            .width(Length::Fill),
    )
    .padding(22)
    .max_width(780)
    .max_height(640)
    .style(styles::modal_card);

    iced::widget::opaque(
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(styles::backdrop),
    )
}

fn ready_body<'a>(
    lang: crate::i18n::Lang,
    dialog: &'a DllScanDialog,
    outcome: &'a application::DllScanOutcome,
) -> Element<'a, Message> {
    let filtered = dialog.filtered();
    let mut summary = row![
        layout::muted(format_count(
            lang,
            "detail.dll_scan.found.one",
            "detail.dll_scan.found.other",
            outcome.entries.len(),
        ))
        .size(11),
        layout::muted(format_count(
            lang,
            "detail.dll_scan.selected.one",
            "detail.dll_scan.selected.other",
            dialog.selected.len(),
        ))
        .size(11),
    ]
    .spacing(14)
    .width(Length::Fill);
    if outcome.skipped_non_utf8 > 0 {
        summary = summary.push(
            layout::faint(format_count(
                lang,
                "detail.dll_scan.skipped_non_utf8.one",
                "detail.dll_scan.skipped_non_utf8.other",
                outcome.skipped_non_utf8,
            ))
            .size(11),
        );
    }

    let mut warnings = column![].spacing(4).width(Length::Fill);
    if outcome.truncated {
        warnings = warnings.push(warning_text(lang, "detail.dll_scan.truncated".to_owned()));
    }
    if outcome.depth_limited {
        warnings = warnings.push(warning_text(
            lang,
            crate::i18n::tr_args(
                lang,
                "detail.dll_scan.depth_limited",
                &[("depth", &application::DLL_SCAN_MAX_DEPTH.to_string())],
            ),
        ));
    }
    if outcome.unreadable_dirs > 0 {
        warnings = warnings.push(warning_text(
            lang,
            format_count(
                lang,
                "detail.dll_scan.unreadable_dirs.one",
                "detail.dll_scan.unreadable_dirs.other",
                outcome.unreadable_dirs,
            ),
        ));
    }

    let toolbar = row![
        text_input(
            crate::i18n::tr(lang, "detail.dll_scan.search_placeholder"),
            dialog.query.as_str()
        )
        .on_input(Message::DllScanQueryChanged)
        .style(styles::text_input)
        .font(font(Weight::Normal))
        .width(Length::Fill),
        controls::quiet_small(if dialog.sort_desc {
            crate::i18n::tr(lang, "detail.dll_scan.sort_desc")
        } else {
            crate::i18n::tr(lang, "detail.dll_scan.sort_asc")
        })
        .on_press(Message::DllScanSortToggled),
        controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.select_all"))
            .on_press(Message::DllScanSelectAll),
        controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.invert"))
            .on_press(Message::DllScanInvert),
        controls::quiet_small(crate::i18n::tr(lang, "detail.dll_scan.clear"))
            .on_press(Message::DllScanClear),
    ]
    .spacing(8)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);

    let mode = row![
        layout::faint(crate::i18n::tr(lang, "detail.dll_scan.mode")).size(11),
        pick_list(
            override_mode_choices(lang),
            Some(override_mode_choice(lang, dialog.mode)),
            Message::DllScanModeChanged,
        )
        .style(styles::pick_list)
        .menu_style(styles::menu)
        .font(font(Weight::Normal)),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let mut list = column![].spacing(2).width(Length::Fill);
    if outcome.entries.is_empty() {
        list = list.push(
            container(
                layout::muted(crate::i18n::tr(lang, "detail.dll_scan.empty")).size(12),
            )
            .padding(20)
            .width(Length::Fill)
            .center_x(Length::Fill),
        );
    } else if filtered.is_empty() {
        list = list.push(
            container(
                layout::muted(crate::i18n::tr(lang, "detail.dll_scan.no_matches")).size(12),
            )
            .padding(20)
            .width(Length::Fill)
            .center_x(Length::Fill),
        );
    } else {
        for entry in &filtered {
            let existing = dialog.existing.contains(&entry.key);
            let checked = existing || dialog.selected.contains(&entry.key);
            let key = entry.key.clone();
            let toggle = (!existing).then(move || {
                move |_value: bool| Message::DllScanToggle(key.clone())
            });
            let name: Element<'a, Message> = if existing {
                layout::faint(entry.name.as_str()).size(13).into()
            } else {
                text(entry.name.as_str())
                    .size(13)
                    .font(font(Weight::Normal))
                    .into()
            };
            let mut line = row![
                checkbox("", checked)
                    .on_toggle_maybe(toggle)
                    .style(styles::checkbox)
                    .size(16.0),
                column![
                    name,
                    layout::faint(entry.relative_path.display().to_string()).size(10),
                ]
                .spacing(1)
                .width(Length::Fill),
            ]
            .spacing(10)
            .width(Length::Fill)
            .align_y(iced::Alignment::Center);
            if existing {
                line = line.push(badge(
                    crate::i18n::tr(lang, "detail.dll_scan.added_badge"),
                    Tone::Info,
                ));
            }
            list = list.push(line);
        }
    }

    let render_limited = filtered.len() >= DLL_SCAN_RENDER_LIMIT
        && outcome.entries.len() > filtered.len();

    let mut footer = row![
        layout::muted(format_count(
            lang,
            "detail.dll_scan.selected.one",
            "detail.dll_scan.selected.other",
            dialog.selected.len(),
        ))
        .size(11),
        iced::widget::horizontal_space(),
        controls::secondary(crate::i18n::tr(lang, "common.cancel"))
            .on_press(Message::CloseDllScan),
    ]
    .spacing(8)
    .width(Length::Fill)
    .align_y(iced::Alignment::Center);
    let confirm = if dialog.applying {
        controls::primary(crate::i18n::tr(lang, "detail.dll_scan.applying"))
    } else {
        controls::primary(crate::i18n::tr(lang, "detail.dll_scan.confirm"))
            .on_press_maybe(
                (!dialog.selected.is_empty()).then_some(Message::DllScanConfirm),
            )
    };
    footer = footer.push(confirm);

    let mut content = column![summary, warnings]
        .spacing(8)
        .width(Length::Fill);
    if render_limited {
        content = content.push(
            layout::faint(format_count(
                lang,
                "detail.dll_scan.render_limited",
                "detail.dll_scan.render_limited",
                DLL_SCAN_RENDER_LIMIT,
            ))
            .size(10),
        );
    }
    content = content.push(toolbar).push(mode);

    let panel = container(
        scrollable(list)
            .height(Length::Fixed(300.0))
            .style(styles::scrollable),
    )
    .padding(8)
    .width(Length::Fill)
    .style(styles::list_panel);

    content = content.push(panel);
    if let Some(error) = &dialog.apply_error {
        content = content.push(
            text(error.clone())
                .size(12)
                .font(font(Weight::Normal))
                .style(|theme| iced::widget::text::Style {
                    color: Some(crate::styles::tone_colors(theme, Tone::Danger).1),
                }),
        );
    }
    content.push(footer).into()
}

fn format_count(
    lang: crate::i18n::Lang,
    one: &'static str,
    other: &'static str,
    count: usize,
) -> String {
    crate::i18n::tr_plural_args(lang, one, other, count as u64, &[("count", &count.to_string())])
}

fn warning_text<'a>(_lang: crate::i18n::Lang, value: String) -> Element<'a, Message> {
    text(value)
        .size(11)
        .font(font(Weight::Normal))
        .style(|theme| iced::widget::text::Style {
            color: Some(crate::styles::tone_colors(theme, Tone::Warning).1),
        })
        .into()
}
```

> `checkbox::on_toggle_maybe` 接受 `Option<F>`；`(!existing).then(move || move |_| ...)` 产生嵌套闭包，若类型推断失败，改为：
> ```rust
> let toggle = if existing {
>     None
> } else {
>     Some(move |_value: bool| Message::DllScanToggle(key.clone()))
> };
> ```
> 并直接用 `checkbox("", checked).on_toggle_maybe(toggle)`。

- [ ] **Step 4: 编译**

Run: `cargo build -p ui`
Expected: PASS。若 `center_x` 在 iced 0.13 的 container 上不可用，改用 `.center(Length::Fill)`。

- [ ] **Step 5: 运行 UI 状态测试**

Run: `cargo test -p ui dll_scan`
Expected: PASS（Task 4 的 9 个测试）。

---

### Task 7: 全量验证与手动检查

**Files:** 无新增。

- [ ] **Step 1: 格式化**

Run: `cargo fmt --all`
Expected: 无错误。

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets`
Expected: 无 warning（尤其 `derivable_impls`、`redundant_closure`）。

- [ ] **Step 3: 全量测试**

Run: `cargo test --workspace`
Expected: PASS，包含 i18n 键集合、application 扫描、UI 状态测试。

- [ ] **Step 4: 手动验证**

Run: `cargo run -p ui --bin gamehub`

检查：

1. 10,000 文件目录扫描时窗口保持响应，扫描中可取消。
2. 勾选若干 DLL 并确认：Override 列表出现对应条目，模式与选择一致，页面顶部出现成功提示。
3. 再次打开对话框：已添加 DLL 显示「已添加」且复选框禁用。
4. 目录不存在、无权限、空目录：错误/空态文案清晰，应用不崩溃，可更换目录。
5. 手动「添加 DLL Override」入口仍可用。
6. 浅色/深色主题下复选框勾选图标、名称、相对路径、「已添加」徽章、错误文字均可见（不仅检查颜色，实际查看渲染结果）。

- [ ] **Step 5: 文档回填（可选，遵循既有文档风格）**

在 `docs/specs/10-ui-config-launch.md` 末尾追加「## 2026-10-31 当前实现：DLL Override 目录扫描」小节，说明入口、扫描上限、每游戏记忆目录、失败回滚；不修改历史计划与验收记录。

---

## Self-Review

- **Spec coverage:** 扫描算法（Task 1）、上限/取消/非 UTF-8/软链接（Task 1 测试）、每游戏目录记忆（Task 2 + 5）、去重与已添加（Task 4）、搜索/排序/全选/反选/清空（Task 4 + 6）、统一模式（Task 4 + 6）、批量写入复用 `save_draft`（Task 4）、失败回滚（Task 4 测试）、成功提示（Task 5）、模态视图（Task 6）、手动入口保留（Task 6）、i18n（Task 3）。
- **Placeholder scan:** 无 TBD/TODO；Task 5 的临时空壳视图在 Task 6 被完整替换，属于按步骤推进，不是未完成实现。
- **Type consistency:** `DllScanDialog.root: Option<PathBuf>`、`DllScanPhase::{NeedDirectory,Scanning,Ready,Failed}`、`PendingApply { original_rows, added, revision }`、`DllScanDirSaved(Result<(), String>)` 在各任务中一致；`filtered()` 同时供渲染与全选/反选使用。
