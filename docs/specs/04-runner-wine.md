# P3 Wine/Proton Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `runner-wine`：系统 Wine 探测、Proton 目录校验、Wine/Proton 环境与命令行构造、可超时的 spawn，以及 runner 健康检查。

**Architecture:** `detect.rs` 负责候选路径与探测；`context.rs` 负责纯环境/命令构造与 spawn；`error.rs` 定义错误。`WineContext` 不做任何 IO 校验，由 `prefix`/`launch` 保证前置条件。

**Tech Stack:** tokio（process/time）、launcher-core、thiserror、tracing。

**前置条件：** P1 完成；`Runner`/`Arch`/`RunnerKind`/`RunnerSource` 已固定。

---

## 固定契约

```rust
pub struct WineContext {
    pub runner: Runner,
    pub prefix: PathBuf,
    pub arch: Arch,
    pub steam_compat_client: PathBuf,
}

impl WineContext {
    pub fn base_env(&self) -> BTreeMap<String, String>;
    pub fn build_command(&self, exe: &Path, args: &[String]) -> (PathBuf, Vec<String>);
    pub async fn spawn(
        &self,
        exe: &Path,
        args: &[String],
        extra_env: &BTreeMap<String, String>,
    ) -> Result<tokio::process::Child, RunnerError>;
}

pub fn candidate_wine_paths(path_env: Option<&OsStr>) -> Vec<PathBuf>;
pub fn candidate_wine_paths_with(
    path_env: Option<&OsStr>,
    common_paths: &[PathBuf],
) -> Vec<PathBuf>;
pub async fn detect_system_wine() -> Vec<Runner>;
pub async fn detect_from_candidates(candidates: &[PathBuf], timeout: Duration) -> Vec<Runner>;
pub fn validate_proton_dir(root: &Path) -> Result<Runner, RunnerError>;
pub fn verify_runner(runner: &Runner) -> Result<(), RunnerError>;
pub fn prefix_has_processes(wine_prefix: &Path) -> bool;
pub fn wineserver_path(runner: &Runner) -> Option<PathBuf>;
pub async fn kill_wine_prefix(runner: &Runner, wine_prefix: &Path) -> Result<(), RunnerError>;
```

---

## Task R1: `WineContext` 环境与命令

**Files:**
- Create: `crates/runner-wine/src/error.rs`
- Create: `crates/runner-wine/src/context.rs`
- Modify: `crates/runner-wine/src/lib.rs`

- [x] **Step 1: 写错误类型**

`crates/runner-wine/src/error.rs`：

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("proton directory is invalid (proton script missing or not executable): {0:?}")]
    InvalidProtonDir(PathBuf),
    #[error("runner binary is missing: {0:?}")]
    MissingBinary(PathBuf),
    #[error("runner binary is not executable: {0:?}")]
    NotExecutable(PathBuf),
    #[error("cannot canonicalize proton directory {path:?}: {source}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("wine version probe failed for {path:?} with code {code:?}: {stderr}")]
    ProbeFailed {
        path: PathBuf,
        code: Option<i32>,
        stderr: String,
    },
    #[error("wine version probe timed out for {0:?}")]
    ProbeTimeout(PathBuf),
    #[error("failed to spawn {program:?}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
```

- [x] **Step 2: 写失败测试（环境与命令行真值表）**

`crates/runner-wine/src/context.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, RunnerKind, RunnerSource};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn wine_context() -> WineContext {
        WineContext {
            runner: Runner {
                id: Uuid::new_v4(),
                kind: RunnerKind::Wine,
                name: "System Wine".into(),
                binary_path: PathBuf::from("/usr/bin/wine"),
                root_dir: PathBuf::from("/usr/bin"),
                arch: Arch::Win64,
                source: RunnerSource::System,
                verified: true,
                version_string: Some("wine-10.0".into()),
            },
            prefix: PathBuf::from("/data/prefixes/game"),
            arch: Arch::Win64,
            steam_compat_client: PathBuf::from("/data/steam-compat-client"),
        }
    }

    fn proton_context() -> WineContext {
        WineContext {
            runner: Runner {
                id: Uuid::new_v4(),
                kind: RunnerKind::Proton,
                name: "GE-Proton9-1".into(),
                binary_path: PathBuf::from("/opt/GE-Proton9-1/proton"),
                root_dir: PathBuf::from("/opt/GE-Proton9-1"),
                arch: Arch::Win64,
                source: RunnerSource::Imported {
                    original_path: PathBuf::from("/opt/GE-Proton9-1"),
                },
                verified: true,
                version_string: Some("GE-Proton9-1".into()),
            },
            prefix: PathBuf::from("/data/prefixes/game"),
            arch: Arch::Win64,
            steam_compat_client: PathBuf::from("/data/steam-compat-client"),
        }
    }

    #[test]
    fn wine_base_env_is_exact() {
        let env = wine_context().base_env();
        assert_eq!(env.len(), 3);
        assert_eq!(env["WINEPREFIX"], "/data/prefixes/game");
        assert_eq!(env["WINEARCH"], "win64");
        assert_eq!(env["WINEDEBUG"], "-all");
        assert!(!env.contains_key("PROTONPATH"));
    }

    #[test]
    fn wine_win32_arch_is_reflected() {
        let mut context = wine_context();
        context.arch = Arch::Win32;
        assert_eq!(context.base_env()["WINEARCH"], "win32");
    }

    #[test]
    fn proton_base_env_is_exact() {
        let env = proton_context().base_env();
        assert_eq!(env.len(), 3);
        assert_eq!(
            env["STEAM_COMPAT_DATA_PATH"],
            "/data/prefixes/game"
        );
        assert_eq!(
            env["STEAM_COMPAT_CLIENT_INSTALL_PATH"],
            "/data/steam-compat-client"
        );
        assert_eq!(env["PROTONPATH"], "/opt/GE-Proton9-1");
        assert!(!env.contains_key("WINEPREFIX"));
    }

    #[test]
    fn wine_command_is_binary_plus_exe_and_args() {
        let context = wine_context();
        let (program, args) = context.build_command(
            &PathBuf::from("C:/Games/game.exe"),
            &["--windowed".to_string(), "-skip".to_string()],
        );
        assert_eq!(program, PathBuf::from("/usr/bin/wine"));
        assert_eq!(
            args,
            vec![
                "C:/Games/game.exe".to_string(),
                "--windowed".to_string(),
                "-skip".to_string()
            ]
        );
    }

    #[test]
    fn proton_command_uses_run_subcommand() {
        let context = proton_context();
        let (program, args) =
            context.build_command(&PathBuf::from("C:/Games/game.exe"), &["-skip".to_string()]);
        assert_eq!(program, PathBuf::from("/opt/GE-Proton9-1/proton"));
        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "C:/Games/game.exe".to_string(),
                "-skip".to_string()
            ]
        );
    }

    #[test]
    fn build_command_does_not_touch_the_filesystem() {
        let context = proton_context();
        let (program, _) = context.build_command(&PathBuf::from("/does/not/exist.exe"), &[]);
        assert_eq!(program, PathBuf::from("/opt/GE-Proton9-1/proton"));
    }
}
```

`crates/runner-wine/src/lib.rs` 暂时：

```rust
mod context;
mod error;

pub use context::WineContext;
pub use error::RunnerError;
```

- [x] **Step 3: 运行并确认失败**

Run: `cargo test -p runner-wine`
Expected: 编译失败，`WineContext` 未定义。

- [x] **Step 4: 实现**

```rust
use launcher_core::{Arch, Runner, RunnerKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::process::{Child, Command};

use crate::RunnerError;

#[derive(Debug, Clone)]
pub struct WineContext {
    pub runner: Runner,
    pub prefix: PathBuf,
    pub arch: Arch,
    pub steam_compat_client: PathBuf,
}

impl WineContext {
    pub fn base_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        match self.runner.kind {
            RunnerKind::Wine => {
                env.insert(
                    "WINEPREFIX".into(),
                    self.prefix.to_string_lossy().into_owned(),
                );
                env.insert("WINEARCH".into(), self.arch.as_str().into());
                env.insert("WINEDEBUG".into(), "-all".into());
            }
            RunnerKind::Proton => {
                env.insert(
                    "STEAM_COMPAT_DATA_PATH".into(),
                    self.prefix.to_string_lossy().into_owned(),
                );
                env.insert(
                    "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
                    self.steam_compat_client.to_string_lossy().into_owned(),
                );
                env.insert(
                    "PROTONPATH".into(),
                    self.runner.root_dir.to_string_lossy().into_owned(),
                );
            }
        }
        env
    }

    pub fn build_command(&self, exe: &Path, args: &[String]) -> (PathBuf, Vec<String>) {
        match self.runner.kind {
            RunnerKind::Wine => {
                let mut command_args = Vec::with_capacity(args.len() + 1);
                command_args.push(exe.to_string_lossy().into_owned());
                command_args.extend(args.iter().cloned());
                (self.runner.binary_path.clone(), command_args)
            }
            RunnerKind::Proton => {
                let mut command_args = Vec::with_capacity(args.len() + 2);
                command_args.push("run".into());
                command_args.push(exe.to_string_lossy().into_owned());
                command_args.extend(args.iter().cloned());
                (self.runner.binary_path.clone(), command_args)
            }
        }
    }

    pub async fn spawn(
        &self,
        exe: &Path,
        args: &[String],
        extra_env: &BTreeMap<String, String>,
    ) -> Result<Child, RunnerError> {
        let (program, command_args) = self.build_command(exe, args);
        let mut command = Command::new(&program);
        command
            .args(&command_args)
            .envs(self.base_env())
            .envs(extra_env)
            .kill_on_drop(false);
        command.spawn().map_err(|source| RunnerError::Spawn { program, source })
    }
}
```

- [x] **Step 5: 运行测试**

Run: `cargo test -p runner-wine`
Expected: 7 passed。

- [x] **Step 6: 提交**

```bash
git add crates/runner-wine
git commit -m "feat(runner-wine): add wine context env and command construction"
```

---

## Task R2: Proton 目录校验与 runner 健康检查

**Files:**
- Create: `crates/runner-wine/src/detect.rs`
- Modify: `crates/runner-wine/src/lib.rs`

- [x] **Step 1: 写测试**

`crates/runner-wine/src/detect.rs` 的测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, RunnerKind, RunnerSource};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn write_script(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn proton_directory_is_validated() {
        let dir = tempdir().unwrap();
        write_script(&dir.path().join("proton"), "#!/bin/sh\nexit 0\n");
        fs::write(dir.path().join("version"), "GE-Proton9-1\n").unwrap();

        let runner = validate_proton_dir(dir.path()).unwrap();
        assert_eq!(runner.kind, RunnerKind::Proton);
        assert_eq!(runner.arch, Arch::Win64);
        assert!(runner.verified);
        assert_eq!(runner.version_string.as_deref(), Some("GE-Proton9-1"));
        assert_eq!(runner.binary_path, dir.path().canonicalize().unwrap().join("proton"));
        assert!(matches!(runner.source, RunnerSource::Imported { .. }));
    }

    #[test]
    fn proton_without_version_file_uses_directory_name() {
        let parent = tempdir().unwrap();
        let dir = parent.path().join("GE-Proton9-1");
        fs::create_dir(&dir).unwrap();
        write_script(&dir.join("proton"), "#!/bin/sh\nexit 0\n");

        let runner = validate_proton_dir(&dir).unwrap();
        assert_eq!(runner.version_string.as_deref(), Some("GE-Proton9-1"));
    }

    #[test]
    fn proton_without_script_is_rejected() {
        let dir = tempdir().unwrap();
        assert!(matches!(
            validate_proton_dir(dir.path()),
            Err(RunnerError::InvalidProtonDir(_))
        ));
    }

    #[test]
    fn proton_script_must_be_executable() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("proton"), "#!/bin/sh\n").unwrap();
        let mut permissions = fs::metadata(dir.path().join("proton"))
            .unwrap()
            .permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(dir.path().join("proton"), permissions).unwrap();

        assert!(matches!(
            validate_proton_dir(dir.path()),
            Err(RunnerError::InvalidProtonDir(_))
        ));
    }

    #[test]
    fn verify_runner_checks_binary_presence_and_mode() {
        let dir = tempdir().unwrap();
        let binary = dir.path().join("wine");
        write_script(&binary, "#!/bin/sh\nexit 0\n");
        let runner = Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: "Wine".into(),
            binary_path: binary.clone(),
            root_dir: dir.path().to_path_buf(),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        };
        verify_runner(&runner).unwrap();

        fs::remove_file(&binary).unwrap();
        assert!(matches!(
            verify_runner(&runner),
            Err(RunnerError::MissingBinary(_))
        ));
    }

    #[test]
    fn candidate_paths_include_path_entry_and_injected_common_locations() {
        let path_dir = tempdir().unwrap();
        let common_dir = tempdir().unwrap();
        let wine = path_dir.path().join("wine");
        let common_wine = common_dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho wine-9.0\n");
        write_script(&common_wine, "#!/bin/sh\necho wine-9.0\n");

        let candidates = candidate_wine_paths_with(
            Some(path_dir.path().as_os_str()),
            &[common_wine.clone()],
        );
        assert_eq!(candidates, vec![wine, common_wine]);
    }

    #[test]
    fn candidate_paths_deduplicate_symlinks() {
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho wine-9.0\n");
        let link = dir.path().join("wine-link");
        std::os::unix::fs::symlink(&wine, &link).unwrap();

        let candidates = candidate_wine_paths_with(
            Some(dir.path().as_os_str()),
            &[wine.clone(), link],
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], wine);
    }
}
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p runner-wine`
Expected: 编译失败，`validate_proton_dir` 等未定义。

- [x] **Step 3: 实现**

```rust
use launcher_core::{Arch, Runner, RunnerKind, RunnerSource};
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use uuid::Uuid;

use crate::RunnerError;

const COMMON_WINE_PATHS: &[&str] = &[
    "/usr/bin/wine",
    "/usr/bin/wine-stable",
    "/usr/bin/wine-development",
];

fn is_executable_file(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

pub fn candidate_wine_paths(path_env: Option<&OsStr>) -> Vec<PathBuf> {
    let common_paths: Vec<PathBuf> = COMMON_WINE_PATHS.iter().map(PathBuf::from).collect();
    candidate_wine_paths_with(path_env, &common_paths)
}

pub fn candidate_wine_paths_with(
    path_env: Option<&OsStr>,
    common_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path_env) = path_env {
        for dir in std::env::split_paths(path_env) {
            let candidate = dir.join("wine");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }

    for candidate in common_paths {
        if candidate.is_file() {
            candidates.push(candidate.clone());
        }
    }

    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| {
        let key = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());
        seen.insert(key)
    });
    candidates
}

pub fn validate_proton_dir(root: &Path) -> Result<Runner, RunnerError> {
    let canonical = root.canonicalize().map_err(|source| RunnerError::Canonicalize {
        path: root.to_path_buf(),
        source,
    })?;
    let proton = canonical.join("proton");
    if !is_executable_file(&proton) {
        return Err(RunnerError::InvalidProtonDir(root.to_path_buf()));
    }

    let version_string = std::fs::read_to_string(canonical.join("version"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            canonical
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
        });

    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Proton")
        .to_string();

    Ok(Runner {
        id: Uuid::new_v4(),
        kind: RunnerKind::Proton,
        name,
        binary_path: proton,
        root_dir: canonical,
        arch: Arch::Win64,
        source: RunnerSource::Imported {
            original_path: root.to_path_buf(),
        },
        verified: true,
        version_string,
    })
}

pub fn verify_runner(runner: &Runner) -> Result<(), RunnerError> {
    if !runner.binary_path.is_file() {
        return Err(RunnerError::MissingBinary(runner.binary_path.clone()));
    }
    if !is_executable_file(&runner.binary_path) {
        return Err(RunnerError::NotExecutable(runner.binary_path.clone()));
    }
    Ok(())
}

async fn probe_wine(binary: &Path, timeout: Duration) -> Result<String, RunnerError> {
    let mut command = Command::new(binary);
    command.arg("--version").kill_on_drop(true);
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| RunnerError::ProbeTimeout(binary.to_path_buf()))??;
    if !output.status.success() {
        return Err(RunnerError::ProbeFailed {
            path: binary.to_path_buf(),
            code: output.status.code(),
            stderr: launcher_core::truncate_utf8(
                &String::from_utf8_lossy(&output.stderr),
                4096,
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn detect_from_candidates(
    candidates: &[PathBuf],
    timeout: Duration,
) -> Vec<Runner> {
    let mut runners = Vec::new();
    for candidate in candidates {
        match probe_wine(candidate, timeout).await {
            Ok(version) => runners.push(Runner {
                id: Uuid::new_v4(),
                kind: RunnerKind::Wine,
                name: "System Wine".into(),
                binary_path: candidate.clone(),
                root_dir: candidate
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("/")),
                arch: Arch::Win64,
                source: RunnerSource::System,
                verified: true,
                version_string: Some(version),
            }),
            Err(error) => {
                tracing::warn!(?candidate, %error, "skipping wine candidate");
            }
        }
    }
    runners
}

pub async fn detect_system_wine() -> Vec<Runner> {
    let path_env = std::env::var_os("PATH");
    let candidates = candidate_wine_paths(path_env.as_deref());
    detect_from_candidates(&candidates, Duration::from_secs(5)).await
}
```

`lib.rs` 增加：

```rust
mod detect;

pub use detect::{
    candidate_wine_paths, candidate_wine_paths_with, detect_from_candidates,
    detect_system_wine, validate_proton_dir, verify_runner,
};
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p runner-wine`
Expected: 13 passed（R1 7 个 + R2 6 个）。

- [x] **Step 5: 提交**

```bash
git add crates/runner-wine
git commit -m "feat(runner-wine): detect system wine and validate proton directories"
```

---

## Task R3: 探测超时与伪造脚本

**Files:**
- Modify: `crates/runner-wine/src/detect.rs`

- [x] **Step 1: 追加异步测试**

```rust
    #[tokio::test]
    async fn detect_reads_version_from_fake_wine() {
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho wine-9.0-test\n");

        let runners = detect_from_candidates(&[wine.clone()], Duration::from_secs(1)).await;
        assert_eq!(runners.len(), 1);
        assert_eq!(runners[0].version_string.as_deref(), Some("wine-9.0-test"));
        assert_eq!(runners[0].binary_path, wine);
        assert!(runners[0].verified);
    }

    #[tokio::test]
    async fn detect_skips_failing_probe() {
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho boom >&2\nexit 3\n");

        let runners = detect_from_candidates(&[wine], Duration::from_secs(1)).await;
        assert!(runners.is_empty());
    }

    #[tokio::test]
    async fn detect_times_out_hung_probe() {
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\nsleep 5\n");

        let started = std::time::Instant::now();
        let runners =
            detect_from_candidates(&[wine], Duration::from_millis(50)).await;
        assert!(runners.is_empty());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
```

- [x] **Step 2: 运行测试**

Run: `cargo test -p runner-wine`
Expected: 16 passed。超时测试应在 2 秒内结束。

- [x] **Step 3: 提交**

```bash
git add crates/runner-wine
git commit -m "test(runner-wine): cover probe success, failure and timeout"
```

---

## Task R4: `spawn` 环境传递

**Files:**
- Modify: `crates/runner-wine/src/context.rs`

- [x] **Step 1: 追加测试**

```rust
    #[tokio::test]
    async fn spawn_passes_base_env_and_extra_env() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let binary = dir.path().join("wine");
        let output = dir.path().join("env.txt");
        fs::write(
            &binary,
            format!(
                "#!/bin/sh\nprintf '%s|%s|%s|%s\\n' \"$WINEPREFIX\" \"$WINEARCH\" \"$WINEDEBUG\" \"$EXTRA\" > '{}'\n",
                output.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();

        let mut context = wine_context();
        context.runner.binary_path = binary;
        let mut extra = BTreeMap::new();
        extra.insert("EXTRA".to_string(), "yes".to_string());

        let mut child = context
            .spawn(Path::new("game.exe"), &[], &extra)
            .await
            .unwrap();
        let status = child.wait().await.unwrap();
        assert!(status.success());

        let written = fs::read_to_string(output).unwrap();
        assert_eq!(
            written.trim(),
            "/data/prefixes/game|win64|-all|yes"
        );
    }
```

- [x] **Step 2: 运行测试**

Run: `cargo test -p runner-wine`
Expected: 17 passed。

- [x] **Step 3: 提交**

```bash
git add crates/runner-wine
git commit -m "test(runner-wine): verify spawn environment propagation"
```

---

## P3 验收

- [x] `cargo test -p runner-wine` 通过。
- [x] Wine 的 `base_env` 恰好是 `WINEPREFIX/WINEARCH/WINEDEBUG`；Proton 恰好是 `STEAM_COMPAT_DATA_PATH/STEAM_COMPAT_CLIENT_INSTALL_PATH/PROTONPATH`。
- [x] Wine 命令是 `(wine, [exe, args...])`；Proton 是 `(proton, ["run", exe, args...])`。
- [x] 系统 Wine 探测对每个候选执行 `--version`，5 秒超时，失败不 panic。
- [x] Proton 目录校验检查 `root/proton` 存在且可执行；版本缺失时用目录名。
- [x] `verify_runner` 在启动前能发现二进制丢失/不可执行。
- [x] 伪造脚本测试覆盖成功、非零退出、超时、环境传递。

## 实施记录

- 实际 Wine 版本：`wine-10.0 (Debian 10.0~repack-6)`（`/usr/bin/wine`）。
- 真实环境探测结果：`detect_system_wine()` 返回 1 个 runner（`/usr/bin/wine`，`verified=true`，版本字符串与 `wine --version` 一致）；本机未安装 Proton，`validate_proton_dir` 仅由伪造目录测试覆盖。
- 测试数量：R1 计划预计 7 个，实际测试模块定义 6 个（计划笔误）；R2 后 13 个，R3 后 16 个，R4 后 17 个，均与后续预期一致。
- 偏差 1：并行测试暴露真实 ETXTBSY 竞态——一个测试 fork 时会继承另一测试正在写入的脚本 fd，随后 exec 该脚本报 `ExecutableFileBusy`，复现率 5/40。已用测试专用 `tokio::sync::Mutex`（`crate::test_support::EXEC_LOCK`）串行化所有"写脚本并 spawn"的测试，修复后连续 50 次全量运行 0 失败；生产代码不受影响。
- 偏差 2：R2 实现中 `verify_runner` 同时覆盖 `MissingBinary` 与 `NotExecutable`，计划测试仅断言前者。
- 验证：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（57 passed）、`cargo build --workspace` 全部通过。

## 当前实现（v0.11.0）

- `WineContext`、探测与 Proton 校验保持 P3 契约；新增两个模块：
  - `status.rs`：`prefix_has_processes(prefix)` 扫描 `/proc/*/environ` 的 `WINEPREFIX`，按 `comm` 排除 `wineserver`，用于判断 Wine 进程是否仍在运行（会话保活）。
  - `stop.rs`：`wineserver_path` 在 Wine 旁路、Proton 的 `files/bin`、`dist/bin`、`bin` 或 `PATH` 中查找；`kill_wine_prefix` 执行 `wineserver -k`，失败只记录 debug（找不到服务器属正常）。
- Wine/Proton 的调优环境（`WINEESYNC`、`PROTON_NO_ESYNC` 等）不在这里生成，而是在 `launch::env` 中按 `LaunchTuning` 注入；`WineContext::base_env` 仍只返回基础变量。
- 原生 Linux 游戏不构造 `WineContext`：`LaunchRequest.runner = None`，启动计划直接执行游戏可执行文件。
- 真实环境仍以 Wine 10.0 / GE-Proton 11 系列验证；本机 gamescope 未安装。
