# P4 Prefix 生命周期与 DXVK Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `prefix`：正确的 Wine/Proton prefix 路径语义、状态与 arch 检测、带超时的 `wineboot` 初始化、从用户目录复制 DXVK 并写入 DLL override、prefix 删除。

**Architecture:** `status.rs` 负责纯路径/状态逻辑；`initialize.rs` 负责 wineboot；`dxvk.rs` 负责文件映射、复制与 `reg add`。所有 Windows 文件系统路径都通过 `windows_root()` 分支：Wine 用 `<prefix>/`，Proton 用 `<prefix>/pfx/`。

**Tech Stack:** tokio（process/fs/time）、runner-wine、launcher-core、thiserror、tracing。

**前置条件：** P3 完成。

---

## 关键语义（不要省略）

- Proton 的 `STEAM_COMPAT_DATA_PATH` 是 **prefix 目录本身**，它下面必须有 `pfx/`。
- Wine prefix 的 `system.reg` 在 `<prefix>/system.reg`；Proton 的在 `<prefix>/pfx/system.reg`。
- Wine 的 `drive_c` 在 `<prefix>/drive_c`；Proton 的在 `<prefix>/pfx/drive_c`。
- `Game.prefix_path = None` 时默认路径是 `<default_prefix_root>/<game_id>`。
- **prefix arch 由 runner 决定**：Wine 使用 `game.arch`；Proton 永远使用 `Arch::Win64`（32 位游戏在 win64 prefix 内通过 WoW64 运行）。DXVK 安装按 prefix arch 选择 x64/x32。
- `prefix_path` 可以是共享目录；引用计数检查在 UI 层做，prefix crate 只负责删除目录。

核心入口：

```rust
pub fn plan_for_game(
    game: &Game,
    runner: Runner,
    default_root: &Path,
    steam_compat_client: PathBuf,
) -> PrefixPlan;
```

`launch` 与 UI 必须通过 `plan_for_game` 构造 prefix，不要自己根据 `game.arch` 设置 Proton prefix 架构。

---

## Task PF1: prefix 路径、状态与 arch 检测

**Files:**
- Create: `crates/prefix/src/error.rs`
- Create: `crates/prefix/src/status.rs`
- Modify: `crates/prefix/src/lib.rs`

- [x] **Step 1: 写错误类型**

`crates/prefix/src/error.rs`：

```rust
use launcher_core::Arch;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrefixError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("runner: {0}")]
    Runner(#[from] runner_wine::RunnerError),
    #[error("background task failed: {0}")]
    Join(String),
    #[error("prefix arch mismatch: expected {expected:?}, found {existing:?}")]
    ArchMismatch {
        expected: Arch,
        existing: Option<Arch>,
    },
    #[error("wineboot failed with code {code:?}: {stderr}")]
    WinebootFailed {
        code: Option<i32>,
        stderr: String,
    },
    #[error("wineboot timed out after {seconds}s")]
    WinebootTimeout { seconds: u64 },
    #[error("prefix initialization finished but system.reg is missing")]
    InitializationIncomplete,
    #[error("prefix is not initialized: {0:?}")]
    PrefixNotInitialized(PathBuf),
    #[error("dxvk source is missing a required file: {path:?}")]
    DxvkSourceInvalid { path: PathBuf },
    #[error("reg add failed for {dll}: code {code:?}: {stderr}")]
    RegAddFailed {
        dll: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("reg add timed out for {dll}")]
    RegAddTimeout { dll: String },
}
```

- [x] **Step 2: 写状态测试**

`crates/prefix/src/status.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, Game, RunnerKind};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;
    use uuid::Uuid;

    fn plan(path: &Path, kind: RunnerKind, arch: Arch) -> PrefixPlan {
        PrefixPlan::new(
            path.to_path_buf(),
            test_runner(kind),
            arch,
            PathBuf::from("/data/steam-compat-client"),
        )
    }

    fn test_runner(kind: RunnerKind) -> launcher_core::Runner {
        launcher_core::Runner {
            id: Uuid::new_v4(),
            kind,
            name: "test".into(),
            binary_path: PathBuf::from("/usr/bin/wine"),
            root_dir: PathBuf::from("/usr"),
            arch: Arch::Win64,
            source: launcher_core::RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    #[test]
    fn default_prefix_path_uses_game_id() {
        let root = Path::new("/data/prefixes");
        let id = Uuid::nil();
        assert_eq!(
            default_prefix_path(id, root),
            PathBuf::from("/data/prefixes/00000000-0000-0000-0000-000000000000")
        );
    }

    #[test]
    fn explicit_game_prefix_wins() {
        let game = Game {
            id: Uuid::nil(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: Some(PathBuf::from("/shared/prefix")),
            arch: Arch::Win64,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        };
        assert_eq!(
            resolve_prefix_path(&game, Path::new("/data/prefixes")),
            PathBuf::from("/shared/prefix")
        );
    }

    #[test]
    fn wine_plan_uses_game_arch() {
        let game = Game {
            id: Uuid::nil(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: None,
            arch: Arch::Win32,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        };
        let plan = plan_for_game(
            &game,
            test_runner(RunnerKind::Wine),
            Path::new("/data/prefixes"),
            PathBuf::from("/data/steam-client"),
        );
        assert_eq!(plan.arch, Arch::Win32);
    }

    #[test]
    fn proton_plan_always_uses_win64() {
        let game = Game {
            id: Uuid::nil(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: Some(PathBuf::from("/shared/prefix")),
            arch: Arch::Win32,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        };
        let plan = plan_for_game(
            &game,
            test_runner(RunnerKind::Proton),
            Path::new("/data/prefixes"),
            PathBuf::from("/data/steam-client"),
        );
        assert_eq!(plan.path, PathBuf::from("/shared/prefix"));
        assert_eq!(plan.arch, Arch::Win64);
    }

    #[test]
    fn windows_root_depends_on_runner_kind() {
        let prefix = Path::new("/data/prefixes/game");
        assert_eq!(
            windows_root(prefix, RunnerKind::Wine),
            PathBuf::from("/data/prefixes/game")
        );
        assert_eq!(
            windows_root(prefix, RunnerKind::Proton),
            PathBuf::from("/data/prefixes/game/pfx")
        );
    }

    #[test]
    fn detect_arch_reads_wine_system_reg() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("system.reg"), "#arch=win64\n").unwrap();
        assert_eq!(
            detect_arch(dir.path(), RunnerKind::Wine),
            Some(Arch::Win64)
        );
    }

    #[test]
    fn detect_arch_reads_proton_pfx_system_reg() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("pfx")).unwrap();
        fs::write(dir.path().join("pfx/system.reg"), "#arch=win32\n").unwrap();
        assert_eq!(
            detect_arch(dir.path(), RunnerKind::Proton),
            Some(Arch::Win32)
        );
    }

    #[test]
    fn status_reports_missing_and_uninitialized() {
        let dir = tempdir().unwrap();
        let missing = plan(&dir.path().join("missing"), RunnerKind::Wine, Arch::Win64);
        assert_eq!(status(&missing), PrefixStatus::Missing);

        let uninitialized = plan(dir.path(), RunnerKind::Wine, Arch::Win64);
        assert_eq!(status(&uninitialized), PrefixStatus::NotInitialized);
    }

    #[test]
    fn status_detects_arch_mismatch() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("system.reg"), "#arch=win32\n").unwrap();
        assert_eq!(
            status(&plan(dir.path(), RunnerKind::Wine, Arch::Win64)),
            PrefixStatus::ArchMismatch {
                existing: Some(Arch::Win32)
            }
        );
    }

    #[test]
    fn status_treats_reg_without_arch_as_initialized() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("system.reg"), "WINE REGISTRY Version 2\n").unwrap();
        assert_eq!(
            status(&plan(dir.path(), RunnerKind::Wine, Arch::Win64)),
            PrefixStatus::Initialized
        );
    }
}
```

`crates/prefix/src/lib.rs` 暂时：

```rust
mod error;
mod status;

pub use error::PrefixError;
pub use status::{
    default_prefix_path, detect_arch, plan_for_game, resolve_prefix_path, status, windows_root,
    PrefixPlan, PrefixStatus,
};
```

- [x] **Step 3: 运行并确认失败**

Run: `cargo test -p prefix`
Expected: 编译失败，`PrefixPlan` 未定义。

- [x] **Step 4: 实现**

```rust
use launcher_core::{Arch, Game, Runner, RunnerKind};
use std::io::Read;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixPlan {
    pub path: PathBuf,
    pub arch: Arch,
    pub runner: Runner,
    pub steam_compat_client: PathBuf,
}

impl PrefixPlan {
    pub fn new(
        path: PathBuf,
        runner: Runner,
        arch: Arch,
        steam_compat_client: PathBuf,
    ) -> Self {
        Self {
            path,
            arch,
            runner,
            steam_compat_client,
        }
    }

    pub fn default_for(
        game_id: Uuid,
        default_root: &Path,
        runner: Runner,
        arch: Arch,
        steam_compat_client: PathBuf,
    ) -> Self {
        Self::new(
            default_prefix_path(game_id, default_root),
            runner,
            arch,
            steam_compat_client,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixStatus {
    Missing,
    NotInitialized,
    Initialized,
    ArchMismatch { existing: Option<Arch> },
}

pub fn default_prefix_path(game_id: Uuid, default_root: &Path) -> PathBuf {
    default_root.join(game_id.to_string())
}

pub fn resolve_prefix_path(game: &Game, default_root: &Path) -> PathBuf {
    game.prefix_path
        .clone()
        .unwrap_or_else(|| default_prefix_path(game.id, default_root))
}

pub fn plan_for_game(
    game: &Game,
    runner: Runner,
    default_root: &Path,
    steam_compat_client: PathBuf,
) -> PrefixPlan {
    let arch = match runner.kind {
        RunnerKind::Proton => Arch::Win64,
        RunnerKind::Wine => game.arch,
    };
    PrefixPlan::new(
        resolve_prefix_path(game, default_root),
        runner,
        arch,
        steam_compat_client,
    )
}

pub fn windows_root(prefix: &Path, kind: RunnerKind) -> PathBuf {
    match kind {
        RunnerKind::Wine => prefix.to_path_buf(),
        RunnerKind::Proton => prefix.join("pfx"),
    }
}

pub fn detect_arch(prefix: &Path, kind: RunnerKind) -> Option<Arch> {
    let system_reg = windows_root(prefix, kind).join("system.reg");
    let mut file = std::fs::File::open(system_reg).ok()?;
    let mut buffer = Vec::with_capacity(8192);
    file.take(8192).read_to_end(&mut buffer).ok()?;
    let text = String::from_utf8_lossy(&buffer);
    if text.contains("#arch=win64") {
        Some(Arch::Win64)
    } else if text.contains("#arch=win32") {
        Some(Arch::Win32)
    } else {
        None
    }
}

pub fn status(plan: &PrefixPlan) -> PrefixStatus {
    if !plan.path.exists() {
        return PrefixStatus::Missing;
    }
    let system_reg = windows_root(&plan.path, plan.runner.kind).join("system.reg");
    if !system_reg.is_file() {
        return PrefixStatus::NotInitialized;
    }
    match detect_arch(&plan.path, plan.runner.kind) {
        Some(existing) if existing != plan.arch => {
            PrefixStatus::ArchMismatch {
                existing: Some(existing),
            }
        }
        _ => PrefixStatus::Initialized,
    }
}
```

- [x] **Step 5: 运行测试**

Run: `cargo test -p prefix`
Expected: 8 passed。

- [x] **Step 6: 提交**

```bash
git add crates/prefix
git commit -m "feat(prefix): add plan, status and arch detection with proton pfx layout"
```

---

## Task PF2: `wineboot` 初始化（Wine）

**Files:**
- Create: `crates/prefix/src/initialize.rs`
- Modify: `crates/prefix/src/lib.rs`

- [x] **Step 1: 写测试**

`crates/prefix/src/initialize.rs` 的测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fake_runner, write_script};
    use launcher_core::{Arch, RunnerKind};
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn wineboot_creates_system_reg_and_skips_second_run() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("wine");
        write_script(
            &script,
            r#"#!/bin/sh
mkdir -p "$WINEPREFIX"
printf '#arch=win64\n' > "$WINEPREFIX/system.reg"
"#,
        );

        let prefix = dir.path().join("prefix");
        let plan = PrefixPlan::new(
            prefix.clone(),
            fake_runner(RunnerKind::Wine, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(plan
            .initialize_with_timeout(Duration::from_secs(2))
            .await
            .unwrap());
        assert!(prefix.join("system.reg").is_file());
        assert!(!plan
            .initialize_with_timeout(Duration::from_secs(2))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn wineboot_nonzero_exit_reports_stderr() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("wine");
        write_script(&script, "#!/bin/sh\necho 'boom' >&2\nexit 3\n");

        let plan = PrefixPlan::new(
            dir.path().join("prefix"),
            fake_runner(RunnerKind::Wine, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        match plan
            .initialize_with_timeout(Duration::from_secs(2))
            .await
        {
            Err(PrefixError::WinebootFailed {
                code: Some(3),
                stderr,
            }) => assert!(stderr.contains("boom")),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn wineboot_timeout_kills_the_child() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("wine");
        write_script(&script, "#!/bin/sh\nsleep 5\n");

        let plan = PrefixPlan::new(
            dir.path().join("prefix"),
            fake_runner(RunnerKind::Wine, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        let started = std::time::Instant::now();
        let result = plan
            .initialize_with_timeout(Duration::from_millis(50))
            .await;
        assert!(matches!(result, Err(PrefixError::WinebootTimeout { .. })));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn arch_mismatch_is_rejected_before_spawning() {
        let dir = tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win32\n").unwrap();

        let plan = PrefixPlan::new(
            prefix,
            fake_runner(RunnerKind::Wine, PathBuf::from("/does/not/exist")),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(matches!(
            plan.initialize_with_timeout(Duration::from_secs(2)).await,
            Err(PrefixError::ArchMismatch {
                existing: Some(Arch::Win32),
                ..
            })
        ));
    }
}
```

需要测试辅助模块。创建 `crates/prefix/src/test_support.rs`（`#[cfg(test)]`），并在 `lib.rs` 中 `#[cfg(test)] mod test_support;`：

```rust
use launcher_core::{Arch, Runner, RunnerKind, RunnerSource};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(crate) fn write_script(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

pub(crate) fn fake_runner(kind: RunnerKind, binary_path: PathBuf) -> Runner {
    let root_dir = binary_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));
    Runner {
        id: Uuid::new_v4(),
        kind,
        name: "fake".into(),
        binary_path,
        root_dir,
        arch: Arch::Win64,
        source: RunnerSource::System,
        verified: true,
        version_string: None,
    }
}
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p prefix`
Expected: 编译失败，`initialize_with_timeout` 未定义。

- [x] **Step 3: 实现**

```rust
use crate::{status, PrefixError, PrefixPlan, PrefixStatus};
use launcher_core::truncate_utf8;
use runner_wine::{RunnerError, WineContext};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const WINEBOOT_TIMEOUT: Duration = Duration::from_secs(120);

impl PrefixPlan {
    pub async fn initialize(&self) -> Result<bool, PrefixError> {
        self.initialize_with_timeout(WINEBOOT_TIMEOUT).await
    }

    pub async fn initialize_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<bool, PrefixError> {
        match status(self) {
            PrefixStatus::Initialized => return Ok(false),
            PrefixStatus::ArchMismatch { existing } => {
                return Err(PrefixError::ArchMismatch {
                    expected: self.arch,
                    existing,
                });
            }
            PrefixStatus::Missing | PrefixStatus::NotInitialized => {}
        }

        tokio::fs::create_dir_all(&self.path).await?;
        tokio::fs::create_dir_all(&self.steam_compat_client).await?;

        let context = WineContext {
            runner: self.runner.clone(),
            prefix: self.path.clone(),
            arch: self.arch,
            steam_compat_client: self.steam_compat_client.clone(),
        };

        let (program, args) =
            context.build_command(Path::new("wineboot"), &["-u".to_string()]);
        let mut command = Command::new(&program);
        command
            .args(&args)
            .envs(context.base_env())
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command
            .spawn()
            .map_err(|source| PrefixError::Runner(RunnerError::Spawn { program, source }))?;

        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| PrefixError::WinebootTimeout {
                seconds: timeout.as_secs(),
            })??;

        if !output.status.success() {
            return Err(PrefixError::WinebootFailed {
                code: output.status.code(),
                stderr: truncate_utf8(&String::from_utf8_lossy(&output.stderr), 4096),
            });
        }

        match status(self) {
            PrefixStatus::Initialized => Ok(true),
            _ => Err(PrefixError::InitializationIncomplete),
        }
    }
}
```

`lib.rs` 增加：

```rust
#[cfg(test)]
mod test_support;

mod initialize;
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p prefix`
Expected: 12 passed。

- [x] **Step 5: 提交**

```bash
git add crates/prefix
git commit -m "feat(prefix): initialize wine prefixes with timeout and env"
```

---

## Task PF3: Proton 初始化（`pfx/` 布局）

**Files:**
- Modify: `crates/prefix/src/initialize.rs`

- [x] **Step 1: 追加测试**

```rust
    #[tokio::test]
    async fn proton_initialization_uses_pfx_layout_and_run_subcommand() {
        let dir = tempdir().unwrap();
        let script = dir.path().join("proton");
        let args_log = dir.path().join("args.log");
        write_script(
            &script,
            &format!(
                r#"#!/bin/sh
printf '%s\n' "$@" > '{}'
mkdir -p "$STEAM_COMPAT_DATA_PATH/pfx"
printf '#arch=win64\n' > "$STEAM_COMPAT_DATA_PATH/pfx/system.reg"
"#,
                args_log.display()
            ),
        );

        let prefix = dir.path().join("compat-data");
        let plan = PrefixPlan::new(
            prefix.clone(),
            fake_runner(RunnerKind::Proton, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(plan
            .initialize_with_timeout(Duration::from_secs(2))
            .await
            .unwrap());
        assert!(prefix.join("pfx/system.reg").is_file());
        assert!(!prefix.join("system.reg").exists());

        let logged = fs::read_to_string(args_log).unwrap();
        let args: Vec<&str> = logged.lines().collect();
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "wineboot");
        assert_eq!(args[2], "-u");
    }
```

- [x] **Step 2: 运行测试**

Run: `cargo test -p prefix`
Expected: 13 passed。若失败，检查 `status()` 是否对 Proton 使用了 `pfx/system.reg`。

- [x] **Step 3: 提交**

```bash
git add crates/prefix
git commit -m "test(prefix): verify proton pfx initialization"
```

---

## Task PF4: DXVK 源校验与路径映射

**Files:**
- Create: `crates/prefix/src/dxvk.rs`
- Modify: `crates/prefix/src/lib.rs`

- [x] **Step 1: 写测试**

`crates/prefix/src/dxvk.rs` 测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::Arch;
    use std::fs;
    use tempfile::tempdir;

    fn write_dll(root: &Path, dir: &str, name: &str) {
        let dir = root.join(dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), b"dll").unwrap();
    }

    #[test]
    fn win64_maps_x64_to_system32_and_x32_to_syswow64() {
        let dir = tempdir().unwrap();
        for name in ["d3d11.dll", "dxgi.dll", "d3d10core.dll", "d3d9.dll"] {
            write_dll(dir.path(), "x64", name);
            write_dll(dir.path(), "x32", name);
        }

        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        )
        .unwrap();

        assert_eq!(files.len(), 8);
        assert_eq!(
            files
                .iter()
                .filter(|file| file.target == DxvkTarget::System32)
                .count(),
            4
        );
        assert_eq!(
            files
                .iter()
                .filter(|file| file.target == DxvkTarget::SysWow64)
                .count(),
            4
        );
    }

    #[test]
    fn win64_without_x32_is_valid() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x64", "d3d11.dll");
        write_dll(dir.path(), "x64", "dxgi.dll");

        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .all(|file| file.target == DxvkTarget::System32));
    }

    #[test]
    fn win32_maps_x32_to_system32() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x32", "d3d11.dll");
        write_dll(dir.path(), "x32", "dxgi.dll");

        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win32,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .all(|file| file.target == DxvkTarget::System32));
    }

    #[test]
    fn missing_required_dll_is_rejected() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x64", "dxgi.dll");

        match validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        ) {
            Err(PrefixError::DxvkSourceInvalid { path }) => {
                assert!(path.ends_with("x64/d3d11.dll"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn registry_entries_are_unique_and_deterministic() {
        let dir = tempdir().unwrap();
        for name in ["d3d11.dll", "dxgi.dll", "d3d9.dll"] {
            write_dll(dir.path(), "x64", name);
            write_dll(dir.path(), "x32", name);
        }
        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        )
        .unwrap();
        assert_eq!(
            dxvk_registry_entries(&files),
            vec![
                ("d3d11", "native,builtin"),
                ("dxgi", "native,builtin"),
                ("d3d9", "native,builtin"),
            ]
        );
    }
}
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p prefix`
Expected: 编译失败，`DxvkSource` 未定义。

- [x] **Step 3: 实现**

```rust
use crate::{status, windows_root, PrefixError, PrefixPlan, PrefixStatus};
use launcher_core::{truncate_utf8, Arch};
use runner_wine::WineContext;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const DLLS: [(&str, &str); 4] = [
    ("d3d11", "d3d11.dll"),
    ("dxgi", "dxgi.dll"),
    ("d3d10core", "d3d10core.dll"),
    ("d3d9", "d3d9.dll"),
];
const REQUIRED: [(&str, &str); 2] = [("d3d11", "d3d11.dll"), ("dxgi", "dxgi.dll")];
const REG_TIMEOUT: Duration = Duration::from_secs(30);
const DXVK_REG_VALUE: &str = "native,builtin";

#[derive(Debug, Clone)]
pub struct DxvkSource {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxvkTarget {
    System32,
    SysWow64,
}

impl DxvkTarget {
    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::System32 => "system32",
            Self::SysWow64 => "syswow64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxvkFile {
    pub source: PathBuf,
    pub target: DxvkTarget,
    pub dll_stem: &'static str,
}

fn collect_dir(
    dir: &Path,
    target: DxvkTarget,
    required: bool,
    files: &mut Vec<DxvkFile>,
) -> Result<(), PrefixError> {
    if !dir.is_dir() {
        if required {
            return Err(PrefixError::DxvkSourceInvalid {
                path: dir.join(REQUIRED[0].1),
            });
        }
        return Ok(());
    }

    if required {
        for (_, file_name) in REQUIRED {
            let path = dir.join(file_name);
            if !path.is_file() {
                return Err(PrefixError::DxvkSourceInvalid { path });
            }
        }
    }

    for (stem, file_name) in DLLS {
        let path = dir.join(file_name);
        if path.is_file() {
            files.push(DxvkFile {
                source: path,
                target,
                dll_stem: stem,
            });
        }
    }
    Ok(())
}

pub fn validate_dxvk_source(
    source: &DxvkSource,
    arch: Arch,
) -> Result<Vec<DxvkFile>, PrefixError> {
    let mut files = Vec::new();
    match arch {
        Arch::Win64 => {
            collect_dir(&source.root.join("x64"), DxvkTarget::System32, true, &mut files)?;
            collect_dir(
                &source.root.join("x32"),
                DxvkTarget::SysWow64,
                false,
                &mut files,
            )?;
        }
        Arch::Win32 => {
            collect_dir(&source.root.join("x32"), DxvkTarget::System32, true, &mut files)?;
        }
    }
    Ok(files)
}

pub fn dxvk_registry_entries(files: &[DxvkFile]) -> Vec<(&'static str, &'static str)> {
    let mut stems: Vec<&'static str> = Vec::new();
    for file in files {
        if !stems.contains(&file.dll_stem) {
            stems.push(file.dll_stem);
        }
    }
    stems
        .into_iter()
        .map(|stem| (stem, DXVK_REG_VALUE))
        .collect()
}

pub async fn install_dxvk(plan: &PrefixPlan, source: &DxvkSource) -> Result<(), PrefixError> {
    if status(plan) != PrefixStatus::Initialized {
        return Err(PrefixError::PrefixNotInitialized(plan.path.clone()));
    }

    let files = validate_dxvk_source(source, plan.arch)?;
    let windows = windows_root(&plan.path, plan.runner.kind).join("drive_c/windows");
    let files_for_copy = files.clone();

    tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
        for file in &files_for_copy {
            let target_dir = windows.join(file.target.dir_name());
            std::fs::create_dir_all(&target_dir)?;
            let file_name = file.source.file_name().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid dxvk source path: {:?}", file.source),
                )
            })?;
            std::fs::copy(&file.source, target_dir.join(file_name))?;
        }
        Ok(())
    })
    .await
    .map_err(|error| PrefixError::Join(error.to_string()))??;

    let context = WineContext {
        runner: plan.runner.clone(),
        prefix: plan.path.clone(),
        arch: plan.arch,
        steam_compat_client: plan.steam_compat_client.clone(),
    };

    for (stem, value) in dxvk_registry_entries(&files) {
        let args: Vec<String> = [
            "add",
            "HKCU\\Software\\Wine\\DllOverrides",
            "/v",
            stem,
            "/d",
            value,
            "/f",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let (program, command_args) = context.build_command(Path::new("reg"), &args);

        let mut command = Command::new(&program);
        command
            .args(&command_args)
            .envs(context.base_env())
            .kill_on_drop(true)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let output = tokio::time::timeout(REG_TIMEOUT, command.output())
            .await
            .map_err(|_| PrefixError::RegAddTimeout {
                dll: stem.to_string(),
            })??;

        if !output.status.success() {
            return Err(PrefixError::RegAddFailed {
                dll: stem.to_string(),
                code: output.status.code(),
                stderr: truncate_utf8(&String::from_utf8_lossy(&output.stderr), 4096),
            });
        }
    }

    Ok(())
}
```

`RunnerError` 在 `install_dxvk` 中没有直接使用，删除未用导入 `RunnerError`；只在 `initialize.rs` 使用。

`lib.rs` 增加：

```rust
mod dxvk;

pub use dxvk::{
    dxvk_registry_entries, install_dxvk, validate_dxvk_source, DxvkFile, DxvkSource,
    DxvkTarget,
};
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p prefix`
Expected: 18 passed。

- [x] **Step 5: 提交**

```bash
git add crates/prefix
git commit -m "feat(prefix): validate dxvk source and map dll targets"
```

---

## Task PF5: DXVK 实际复制与 `reg add`

**Files:**
- Modify: `crates/prefix/src/dxvk.rs`

- [x] **Step 1: 追加测试**

```rust
    #[tokio::test]
    async fn install_copies_files_and_invokes_reg_add() {
        use crate::test_support::{fake_runner, write_script};
        use launcher_core::RunnerKind;
        use std::path::PathBuf;

        let dir = tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        let reg_log = dir.path().join("reg.log");
        let wine = dir.path().join("wine");
        write_script(
            &wine,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
                reg_log.display()
            ),
        );

        let source = dir.path().join("dxvk");
        for name in ["d3d11.dll", "dxgi.dll", "d3d9.dll"] {
            write_dll(&source, "x64", name);
        }

        let plan = PrefixPlan::new(
            prefix.clone(),
            fake_runner(RunnerKind::Wine, wine),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        install_dxvk(
            &plan,
            &DxvkSource {
                root: source,
            },
        )
        .await
        .unwrap();

        let system32 = prefix.join("drive_c/windows/system32");
        assert_eq!(fs::read(system32.join("d3d11.dll")).unwrap(), b"dll");
        assert_eq!(fs::read(system32.join("dxgi.dll")).unwrap(), b"dll");
        assert_eq!(fs::read(system32.join("d3d9.dll")).unwrap(), b"dll");

        let logged = fs::read_to_string(reg_log).unwrap();
        assert!(logged.contains("add"));
        assert!(logged.contains("HKCU\\Software\\Wine\\DllOverrides"));
        assert!(logged.contains("d3d11"));
        assert!(logged.contains("native,builtin"));
        assert_eq!(
            logged.lines().filter(|line| *line == "add").count(),
            3
        );
    }

    #[tokio::test]
    async fn install_for_proton_uses_pfx_drive_c_and_run_reg() {
        use crate::test_support::{fake_runner, write_script};
        use launcher_core::RunnerKind;

        let dir = tempdir().unwrap();
        let prefix = dir.path().join("compat-data");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "#arch=win64\n").unwrap();

        let reg_log = dir.path().join("reg.log");
        let proton = dir.path().join("proton");
        write_script(
            &proton,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
                reg_log.display()
            ),
        );

        let source = dir.path().join("dxvk");
        for name in ["d3d11.dll", "dxgi.dll"] {
            write_dll(&source, "x64", name);
        }

        let plan = PrefixPlan::new(
            prefix.clone(),
            fake_runner(RunnerKind::Proton, proton),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        install_dxvk(
            &plan,
            &DxvkSource {
                root: source,
            },
        )
        .await
        .unwrap();

        assert!(prefix
            .join("pfx/drive_c/windows/system32/d3d11.dll")
            .is_file());
        let logged = fs::read_to_string(reg_log).unwrap();
        let args: Vec<&str> = logged.lines().collect();
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "reg");
    }

    #[tokio::test]
    async fn install_requires_initialized_prefix() {
        use crate::test_support::fake_runner;
        use launcher_core::RunnerKind;
        use std::path::PathBuf;

        let dir = tempdir().unwrap();
        let source = dir.path().join("dxvk");
        write_dll(&source, "x64", "d3d11.dll");
        write_dll(&source, "x64", "dxgi.dll");

        let plan = PrefixPlan::new(
            dir.path().join("missing"),
            fake_runner(RunnerKind::Wine, PathBuf::from("/bin/true")),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(matches!(
            install_dxvk(
                &plan,
                &DxvkSource {
                    root: source,
                }
            )
            .await,
            Err(PrefixError::PrefixNotInitialized(_))
        ));
    }
```

注意：`install_for_proton` 中同一脚本会收到每次 reg 调用；日志第一行是 `run`，第二行是 `reg`。因为 `proton run reg add ...`。断言 `args[0]=="run"`、`args[1]=="reg"`。

- [x] **Step 2: 运行测试**

Run: `cargo test -p prefix`
Expected: 21 passed。

- [x] **Step 3: 提交**

```bash
git add crates/prefix
git commit -m "feat(prefix): copy dxvk files and register dll overrides"
```

---

## Task PF6: 删除 prefix 与真实 Wine 集成测试

**Files:**
- Modify: `crates/prefix/src/error.rs`
- Modify: `crates/prefix/src/lib.rs`
- Modify: `crates/prefix/src/initialize.rs`

- [x] **Step 1: 实现删除（带安全护栏）**

先给 `PrefixError` 增加：

```rust
    #[error("refusing to delete a directory that is not an initialized prefix: {0:?}")]
    RefusingToDeleteNonPrefix(PathBuf),
```

在 `lib.rs` 中：

```rust
pub async fn delete_prefix(path: &Path) -> Result<(), PrefixError> {
    if !path.exists() {
        return Ok(());
    }
    let initialized = path.join("system.reg").is_file()
        || path.join("pfx/system.reg").is_file();
    if !initialized {
        return Err(PrefixError::RefusingToDeleteNonPrefix(
            path.to_path_buf(),
        ));
    }
    tokio::fs::remove_dir_all(path)
        .await
        .map_err(PrefixError::Io)
}
```

需要 `use std::path::Path;`。这个护栏确保 UI 不会因为用户误选了 `$HOME` 或 `/` 而递归删除；只有含 `system.reg` 或 `pfx/system.reg` 的目录才允许删除。

- [x] **Step 2: 测试删除**

在 `crates/prefix/src/lib.rs` 末尾加入：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn delete_prefix_removes_tree_and_is_idempotent() {
        let dir = tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "x").unwrap();

        delete_prefix(&prefix).await.unwrap();
        assert!(!prefix.exists());
        delete_prefix(&prefix).await.unwrap();
    }

    #[tokio::test]
    async fn delete_prefix_refuses_arbitrary_directory() {
        let dir = tempdir().unwrap();
        let victim = dir.path().join("not-a-prefix");
        fs::create_dir(&victim).unwrap();
        fs::write(victim.join("precious.txt"), "keep me").unwrap();

        assert!(matches!(
            delete_prefix(&victim).await,
            Err(PrefixError::RefusingToDeleteNonPrefix(_))
        ));
        assert!(victim.exists());
    }
}
```

- [x] **Step 3: 增加真实 Wine 集成测试（`#[ignore]`）**

在 `initialize.rs` 测试模块：

```rust
    #[tokio::test]
    #[ignore = "requires a working system wine; run manually"]
    async fn initializes_with_real_system_wine() {
        let runners = runner_wine::detect_system_wine().await;
        let runner = runners
            .into_iter()
            .next()
            .expect("no system wine found");

        let dir = tempdir().unwrap();
        let plan = PrefixPlan::new(
            dir.path().join("prefix"),
            runner,
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(plan
            .initialize_with_timeout(Duration::from_secs(180))
            .await
            .unwrap());
        assert!(plan.path.join("system.reg").is_file());
    }
```

- [x] **Step 4: 运行常规测试与真实测试**

Run: `cargo test -p prefix`
Expected: 23 passed，1 ignored。

Run（手工，需要本机 Wine 10.0，可能耗时 1–2 分钟）:

```bash
cargo test -p prefix initializes_with_real_system_wine -- --ignored --nocapture
```

Expected: 生成 `system.reg`，测试通过。若 Wine 在无图形环境报错，记录 stderr 到 CHANGELOG，不要修改测试为静默跳过。

- [x] **Step 5: 提交**

```bash
git add crates/prefix
git commit -m "feat(prefix): add prefix deletion and real wine integration test"
```

---

## P4 验收

- [x] `cargo test -p prefix` 通过。
- [x] Wine prefix 的 `system.reg` 在根目录；Proton 在 `pfx/system.reg`。
- [x] `PrefixPlan::initialize` 对已初始化 prefix 返回 `Ok(false)`，不重复执行 wineboot。
- [x] arch 不匹配在 spawn 前报错。
- [x] wineboot 非零退出携带截断 stderr；超时会杀掉子进程。
- [x] DXVK Win64 映射 x64→system32、x32→syswow64；Win32 映射 x32→system32。
- [x] DXVK 复制完成后对每个唯一 DLL 执行一次 `reg add`；Proton 走 `proton run reg add`。
- [x] 真实 Wine 测试用手工 `--ignored` 运行并记录结果。

## 实施记录

- 真实 wineboot 耗时：14.23 s（Wine 10.0 Debian 10.0~repack-6，win64 prefix，180 s 预算）；`system.reg` 正常生成，无图形环境报错。
- DXVK 版本：本机未安装 DXVK，也未引入任何下载逻辑；源校验/复制/`reg add` 均由伪造 DLL 目录与假 runner 脚本覆盖。DXVK 源按官方发布结构（`x64/`、`x32/`）读取，由用户在 UI 中指定。
- 测试数量偏差（计划计数与测试模块实际定义不一致）：PF1 实际 10 个（计划 8），PF2 后 14 个（计划 12），PF3 后 15 个（计划 13），PF4 后 20 个（计划 18），PF5 后 23 个（计划 21），PF6 后 25 passed + 1 ignored（计划 23 + 1）。
- 偏差 1：`test_support` 在计划内容之外增加 `EXEC_LOCK`（tokio Mutex），串行化"写脚本并 spawn"的测试，避免 P3 中已复现的 ETXTBSY 竞态；prefix 全量测试连续 30 次运行 0 失败。
- 偏差 2：`detect_arch` 中 `File::open` 不需要 `mut`（`Read::take` 按值接收），按 clippy 要求去掉。
- 偏差 3：PF5 测试中删除了未使用的 `std::path::PathBuf` 导入。
- 验证：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（82 passed, 1 ignored）、`cargo build --workspace` 全部通过；`cargo test -p prefix initializes_with_real_system_wine -- --ignored` 通过。
