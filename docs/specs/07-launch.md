# P6 启动管线 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 实现 `launch`：runner 解析、启动前校验、环境变量合成、DLL override 字符串、gamescope 包装、spawn。

**Architecture:** `resolve.rs` 决定用哪个 runner；`env.rs` 纯环境合成；`plan.rs` 校验并构造 `LaunchPlan`；`spawn.rs` 用 `env_clear` + `envs` 启动。`LaunchRequest` 显式携带继承环境、gamescope 路径等所有外部输入，保证可测试。

**Tech Stack:** launcher-core、storage（类型）、prefix、display、runner-wine、tokio::process、thiserror。

**前置条件：** P2/P3/P4/P5 完成。

---

## 固定契约

```rust
#[derive(Debug, Clone, Default)]
pub struct LaunchTuning {
    pub enable_esync: bool,
    pub enable_fsync: bool,
    pub enable_mangohud: bool,
    pub mangohud_path: Option<PathBuf>,
    pub enable_dxvk_nvapi: bool,
}

pub struct LaunchRequest {
    pub game: Game,
    /// 原生 Linux 游戏为 `None`，直接执行可执行文件。
    pub runner: Option<Runner>,
    pub session: SessionType,
    pub global_env: BTreeMap<String, String>,
    pub inherited_env: BTreeMap<String, String>,
    pub steam_compat_client: PathBuf,
    pub default_prefix_root: PathBuf,
    pub gamescope_path: Option<PathBuf>,
    pub tuning: LaunchTuning,
    pub log_path: Option<PathBuf>,
    pub dxvk_nvapi_source: Option<PathBuf>,
}

pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}

pub fn resolve_runner(
    game: &Game,
    runners: &[Runner],
    default_runner_id: Option<Uuid>,
) -> Result<Runner, LaunchError>;

pub fn build_dll_overrides(overrides: &[DllOverride]) -> String;

pub fn build_env(req: &LaunchRequest) -> Result<BTreeMap<String, String>, LaunchError>;

pub fn validate(req: &LaunchRequest) -> Result<(), LaunchError>;

pub fn build_plan(req: &LaunchRequest) -> Result<LaunchPlan, LaunchError>;

pub async fn spawn_plan(
    plan: &LaunchPlan,
) -> Result<tokio::process::Child, LaunchError>;

pub async fn spawn_plan_with_log(
    plan: &LaunchPlan,
    log_path: Option<&Path>,
) -> Result<tokio::process::Child, LaunchError>;
```

`validate` 的当前规则：

- 可执行文件必须存在；`working_dir` 若设置必须是目录。
- 仅非原生游戏要求 `runner: Some`、`verify_runner` 通过、prefix 已初始化且 arch 匹配、DLL 名称合法。原生游戏跳过这些检查，忽略 `prefix_path` 状态与全部 Wine 设置。
- `GamescopeMode::Forced` 需要 `gamescope_path`；启用 MangoHud 需要 `mangohud_path`。
- 全局或游戏环境变量中出现 `WINEDLLOVERRIDES` 一律拒绝（结构化 override 是唯一入口）；环境变量名按 shell 标识符校验。

`build_plan` 的当前行为：

- 原生游戏：`program = executable_path`、`args = game.args`；不构造 `WineContext`。
- Windows 游戏：`WineContext::build_command`（Wine 直接执行、Proton `run`）。
- 随后按需包 gamescope（`--` 分隔）或 MangoHud（`--dlsym`）；`build_env` 再注入调优、显示与 DLL override。
- 环境优先级：继承 → runner base → 调优默认 → 全局 → 游戏 → 显示器 → `WINEDLLOVERRIDES`；调优相关的 8 个变量先清除继承值，避免宿主机环境污染。

`build_plan` 内部先调用 `validate`；调用方不需要再单独校验。

---

## Task L1: Runner 解析

**Files:**
- Create: `crates/launch/src/error.rs`
- Create: `crates/launch/src/resolve.rs`
- Modify: `crates/launch/src/lib.rs`

- [x] **Step 1: 写错误类型**

`crates/launch/src/error.rs`：

```rust
use launcher_core::Arch;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("no runner selected and no default runner configured")]
    NoRunnerSelected,
    #[error("runner not found: {0}")]
    RunnerNotFound(Uuid),
    #[error("default runner not found: {0}")]
    DefaultRunnerUnavailable(Uuid),
    #[error("game executable not found: {0:?}")]
    ExecutableMissing(PathBuf),
    #[error("working directory not found: {0:?}")]
    WorkingDirMissing(PathBuf),
    #[error("runner error: {0}")]
    Runner(#[from] runner_wine::RunnerError),
    #[error("prefix is not initialized: {0:?}")]
    PrefixNotInitialized(PathBuf),
    #[error("prefix arch mismatch: expected {expected:?}, found {existing:?}")]
    PrefixArchMismatch {
        expected: Arch,
        existing: Option<Arch>,
    },
    #[error("gamescope is forced but not available")]
    GamescopeUnavailable,
    #[error("WINEDLLOVERRIDES must be configured through structured dll_overrides")]
    DllOverrideConflict,
    #[error("invalid DLL name: {0:?}")]
    InvalidDllName(String),
    #[error("invalid environment variable key: {0:?}")]
    InvalidEnvKey(String),
    #[error("failed to spawn {program:?}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
```

- [x] **Step 2: 写解析测试**

`crates/launch/src/resolve.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, RunnerKind, RunnerSource};
    use uuid::Uuid;

    fn runner(name: &str) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: name.into(),
            binary_path: PathBuf::from("/usr/bin/wine"),
            root_dir: PathBuf::from("/usr/bin"),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    fn game(runner_id: Option<Uuid>) -> Game {
        Game {
            id: Uuid::new_v4(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id,
            prefix_path: None,
            arch: Arch::Win64,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        }
    }

    #[test]
    fn game_runner_wins_over_default() {
        let game_runner = runner("game");
        let default_runner = runner("default");
        let game = game(Some(game_runner.id));
        let resolved = resolve_runner(
            &game,
            &[game_runner.clone(), default_runner],
            Some(Uuid::new_v4()),
        )
        .unwrap();
        assert_eq!(resolved, game_runner);
    }

    #[test]
    fn default_runner_is_used_when_game_has_none() {
        let default_runner = runner("default");
        let game = game(None);
        let resolved =
            resolve_runner(&game, &[default_runner.clone()], Some(default_runner.id))
                .unwrap();
        assert_eq!(resolved, default_runner);
    }

    #[test]
    fn no_selection_and_no_default_is_an_error() {
        let game = game(None);
        assert!(matches!(
            resolve_runner(&game, &[runner("only")], None),
            Err(LaunchError::NoRunnerSelected)
        ));
    }

    #[test]
    fn missing_game_runner_is_reported() {
        let game = game(Some(Uuid::new_v4()));
        assert!(matches!(
            resolve_runner(&game, &[runner("other")], None),
            Err(LaunchError::RunnerNotFound(_))
        ));
    }

    #[test]
    fn missing_default_runner_is_reported() {
        let game = game(None);
        let id = Uuid::new_v4();
        assert!(matches!(
            resolve_runner(&game, &[runner("other")], Some(id)),
            Err(LaunchError::DefaultRunnerUnavailable(_))
        ));
    }
}
```

`crates/launch/src/lib.rs` 暂时：

```rust
mod error;
mod resolve;

pub use error::LaunchError;
pub use resolve::resolve_runner;
```

- [x] **Step 3: 运行并确认失败**

Run: `cargo test -p launch`
Expected: 编译失败，`resolve_runner` 未定义。

- [x] **Step 4: 实现**

```rust
use launcher_core::{Game, Runner};
use uuid::Uuid;

use crate::LaunchError;

pub fn resolve_runner(
    game: &Game,
    runners: &[Runner],
    default_runner_id: Option<Uuid>,
) -> Result<Runner, LaunchError> {
    if let Some(id) = game.runner_id {
        return runners
            .iter()
            .find(|runner| runner.id == id)
            .cloned()
            .ok_or(LaunchError::RunnerNotFound(id));
    }

    let id = default_runner_id.ok_or(LaunchError::NoRunnerSelected)?;
    runners
        .iter()
        .find(|runner| runner.id == id)
        .cloned()
        .ok_or(LaunchError::DefaultRunnerUnavailable(id))
}
```

- [x] **Step 5: 运行测试**

Run: `cargo test -p launch`
Expected: 5 passed。

- [x] **Step 6: 提交**

```bash
git add crates/launch
git commit -m "feat(launch): resolve game runner or global default"
```

---

## Task L2: DLL override 字符串

**Files:**
- Create: `crates/launch/src/env.rs`
- Modify: `crates/launch/src/lib.rs`

- [x] **Step 1: 写测试**

`crates/launch/src/env.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{DllOverride, OverrideMode};

    #[test]
    fn all_modes_map_to_wine_syntax() {
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "a".into(),
                mode: OverrideMode::Disabled,
            }]),
            "a=disabled"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "b".into(),
                mode: OverrideMode::Native,
            }]),
            "b=n"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "c".into(),
                mode: OverrideMode::Builtin,
            }]),
            "c=b"
        );
        assert_eq!(
            build_dll_overrides(&[DllOverride {
                dll_name: "d".into(),
                mode: OverrideMode::NativeBuiltin,
            }]),
            "d=n,b"
        );
    }

    #[test]
    fn multiple_overrides_are_semicolon_separated_in_input_order() {
        let overrides = vec![
            DllOverride {
                dll_name: "winmm".into(),
                mode: OverrideMode::NativeBuiltin,
            },
            DllOverride {
                dll_name: "steam_api64".into(),
                mode: OverrideMode::Native,
            },
        ];
        assert_eq!(build_dll_overrides(&overrides), "winmm=n,b;steam_api64=n");
    }

    #[test]
    fn empty_names_are_skipped() {
        let overrides = vec![
            DllOverride {
                dll_name: "  ".into(),
                mode: OverrideMode::Native,
            },
            DllOverride {
                dll_name: " winmm ".into(),
                mode: OverrideMode::Builtin,
            },
        ];
        assert_eq!(build_dll_overrides(&overrides), "winmm=b");
    }
}
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p launch`
Expected: 编译失败，`build_dll_overrides` 未定义。

- [x] **Step 3: 实现**

```rust
use launcher_core::{DllOverride, OverrideMode};

fn mode_value(mode: OverrideMode) -> &'static str {
    match mode {
        OverrideMode::Disabled => "disabled",
        OverrideMode::Native => "n",
        OverrideMode::Builtin => "b",
        OverrideMode::NativeBuiltin => "n,b",
    }
}

pub fn build_dll_overrides(overrides: &[DllOverride]) -> String {
    overrides
        .iter()
        .filter_map(|entry| {
            let name = entry.dll_name.trim();
            if name.is_empty() {
                None
            } else {
                Some(format!("{name}={}", mode_value(entry.mode)))
            }
        })
        .collect::<Vec<_>>()
        .join(";")
}
```

`lib.rs` 增加：

```rust
mod env;

pub use env::build_dll_overrides;
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p launch`
Expected: 8 passed。

- [x] **Step 5: 提交**

```bash
git add crates/launch
git commit -m "feat(launch): build WINEDLLOVERRIDES from structured overrides"
```

---

## Task L3: 环境变量合成

**Files:**
- Modify: `crates/launch/src/env.rs`

- [x] **Step 1: 写测试**

在 `env.rs` 的测试模块追加测试辅助与用例：

```rust
    use launcher_core::{
        Arch, DisplayConfig, Game, GamescopeMode, Runner, RunnerKind, RunnerSource,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn runner() -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: "Wine".into(),
            binary_path: PathBuf::from("/usr/bin/wine"),
            root_dir: PathBuf::from("/usr/bin"),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    fn game() -> Game {
        Game {
            id: Uuid::new_v4(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: Some(PathBuf::from("/data/prefix")),
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            env_overrides: BTreeMap::new(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        }
    }

    fn request() -> LaunchRequest {
        LaunchRequest {
            game: game(),
            runner: runner(),
            session: SessionType::X11,
            global_env: BTreeMap::new(),
            inherited_env: BTreeMap::new(),
            steam_compat_client: PathBuf::from("/data/steam-compat-client"),
            default_prefix_root: PathBuf::from("/data/prefixes"),
            gamescope_path: None,
        }
    }

    #[test]
    fn environment_precedence_is_game_over_global_over_inherited_over_base() {
        let mut request = request();
        request.inherited_env = BTreeMap::from([
            ("A".into(), "inherited".into()),
            ("HOME".into(), "/home/test".into()),
            ("WINEDLLOVERRIDES".into(), "legacy".into()),
        ]);
        request.global_env = BTreeMap::from([
            ("A".into(), "global".into()),
            ("B".into(), "global".into()),
        ]);
        request.game.env_overrides = BTreeMap::from([
            ("A".into(), "game".into()),
            ("C".into(), "game".into()),
            ("WINEDEBUG".into(), "-fixme".into()),
        ]);

        let env = build_env(&request).unwrap();
        assert_eq!(env["A"], "game");
        assert_eq!(env["B"], "global");
        assert_eq!(env["C"], "game");
        assert_eq!(env["HOME"], "/home/test");
        assert_eq!(env["WINEPREFIX"], "/data/prefix");
        assert_eq!(env["WINEARCH"], "win64");
        assert_eq!(env["WINEDEBUG"], "-fixme");
        assert!(!env.contains_key("WINEDLLOVERRIDES"));
    }

    #[test]
    fn manual_winedlloverrides_is_rejected_even_if_inherited() {
        let mut request = request();
        request.inherited_env = BTreeMap::from([("WINEDLLOVERRIDES".into(), "x=n".into())]);
        request.game.env_overrides =
            BTreeMap::from([("WINEDLLOVERRIDES".into(), "y=n".into())]);
        assert!(matches!(
            build_env(&request),
            Err(LaunchError::DllOverrideConflict)
        ));
    }

    #[test]
    fn global_winedlloverrides_is_rejected() {
        let mut request = request();
        request.global_env =
            BTreeMap::from([("WINEDLLOVERRIDES".into(), "x=n".into())]);
        assert!(matches!(
            build_env(&request),
            Err(LaunchError::DllOverrideConflict)
        ));
    }

    #[test]
    fn structured_overrides_replace_the_conflicting_variable() {
        let mut request = request();
        request.game.dll_overrides = vec![
            DllOverride {
                dll_name: "d3d11".into(),
                mode: launcher_core::OverrideMode::NativeBuiltin,
            },
            DllOverride {
                dll_name: "winmm".into(),
                mode: launcher_core::OverrideMode::Native,
            },
        ];
        let env = build_env(&request).unwrap();
        assert_eq!(env["WINEDLLOVERRIDES"], "d3d11=n,b;winmm=n");
    }

    #[test]
    fn x11_monitor_is_injected_only_without_gamescope() {
        let mut request = request();
        request.game.display.monitor = Some("eDP-1".into());
        assert_eq!(
            build_env(&request).unwrap()["__GL_SYNC_DISPLAY_DEVICE"],
            "eDP-1"
        );

        request.gamescope_path = Some(PathBuf::from("/usr/bin/gamescope"));
        request.game.display.gamescope = GamescopeMode::Forced;
        assert!(!build_env(&request)
            .unwrap()
            .contains_key("__GL_SYNC_DISPLAY_DEVICE"));
    }

    #[test]
    fn wayland_never_injects_x11_display_variable() {
        let mut request = request();
        request.session = SessionType::Wayland;
        request.game.display.monitor = Some("eDP-1".into());
        assert!(!build_env(&request)
            .unwrap()
            .contains_key("__GL_SYNC_DISPLAY_DEVICE"));
    }

    #[test]
    fn proton_environment_uses_compat_variables() {
        let mut request = request();
        request.runner.kind = RunnerKind::Proton;
        request.runner.binary_path = PathBuf::from("/opt/proton/proton");
        request.runner.root_dir = PathBuf::from("/opt/proton");
        request.game.prefix_path = Some(PathBuf::from("/data/compat"));
        request.game.arch = Arch::Win32;

        let env = build_env(&request).unwrap();
        assert_eq!(env["STEAM_COMPAT_DATA_PATH"], "/data/compat");
        assert_eq!(
            env["STEAM_COMPAT_CLIENT_INSTALL_PATH"],
            "/data/steam-compat-client"
        );
        assert_eq!(env["PROTONPATH"], "/opt/proton");
        assert!(!env.contains_key("WINEPREFIX"));
    }

    #[test]
    fn invalid_env_keys_are_rejected() {
        let mut request = request();
        request.game.env_overrides =
            BTreeMap::from([("BAD KEY".into(), "x".into())]);
        assert!(matches!(
            build_env(&request),
            Err(LaunchError::InvalidEnvKey(_))
        ));
    }

    #[test]
    fn invalid_dll_names_are_rejected() {
        for name in ["", " ", "d3d11.dll", "../evil", "bad name"] {
            let mut request = request();
            request.game.dll_overrides = vec![DllOverride {
                dll_name: name.into(),
                mode: launcher_core::OverrideMode::Native,
            }];
            assert!(
                matches!(build_env(&request), Err(LaunchError::InvalidDllName(_))),
                "expected {name:?} to be rejected"
            );
        }
    }
```

`WineContext` 导入在测试中没有直接使用，删除；上面测试只用 `build_env`。

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p launch`
Expected: 编译失败，`LaunchRequest`/`build_env` 未定义。

- [x] **Step 3: 实现**

在 `env.rs` 加入：

```rust
use crate::{LaunchError, LaunchRequest};
use display::{should_use_gamescope, SessionType};
use prefix::plan_for_game;
use runner_wine::WineContext;
use std::collections::BTreeMap;

pub fn validate_env_key(key: &str) -> Result<(), LaunchError> {
    let mut chars = key.chars();
    let first = chars
        .next()
        .ok_or_else(|| LaunchError::InvalidEnvKey(key.to_string()))?;
    if first != '_' && !first.is_ascii_alphabetic() {
        return Err(LaunchError::InvalidEnvKey(key.to_string()));
    }
    if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        return Err(LaunchError::InvalidEnvKey(key.to_string()));
    }
    Ok(())
}

pub fn validate_dll_name(name: &str) -> Result<(), LaunchError> {
    let name = name.trim();
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_whitespace)
        || name.to_ascii_lowercase().ends_with(".dll")
    {
        return Err(LaunchError::InvalidDllName(name.to_string()));
    }
    Ok(())
}

pub fn build_env(
    request: &LaunchRequest,
) -> Result<BTreeMap<String, String>, LaunchError> {
    let prefix_plan = plan_for_game(
        &request.game,
        request.runner.clone(),
        &request.default_prefix_root,
        request.steam_compat_client.clone(),
    );
    let context = WineContext {
        runner: request.runner.clone(),
        prefix: prefix_plan.path,
        arch: prefix_plan.arch,
        steam_compat_client: request.steam_compat_client.clone(),
    };

    let mut env = request.inherited_env.clone();
    env.remove("WINEDLLOVERRIDES");

    if request.global_env.contains_key("WINEDLLOVERRIDES")
        || request.game.env_overrides.contains_key("WINEDLLOVERRIDES")
    {
        return Err(LaunchError::DllOverrideConflict);
    }

    for key in request
        .global_env
        .keys()
        .chain(request.game.env_overrides.keys())
    {
        validate_env_key(key)?;
    }

    env.extend(context.base_env());
    env.extend(request.global_env.clone());
    env.extend(request.game.env_overrides.clone());

    let gamescope_available = request.gamescope_path.is_some();
    let uses_gamescope = should_use_gamescope(
        &request.game.display,
        request.session,
        gamescope_available,
    );
    if request.session == SessionType::X11 && !uses_gamescope {
        if let Some(monitor) = request
            .game
            .display
            .monitor
            .as_deref()
            .map(str::trim)
            .filter(|monitor| !monitor.is_empty())
        {
            env.insert("__GL_SYNC_DISPLAY_DEVICE".into(), monitor.to_string());
        }
    }

    if !request.game.dll_overrides.is_empty() {
        for entry in &request.game.dll_overrides {
            validate_dll_name(&entry.dll_name)?;
        }
        env.insert(
            "WINEDLLOVERRIDES".into(),
            build_dll_overrides(&request.game.dll_overrides),
        );
    }

    Ok(env)
}
```

`GamescopeMode` 在实现中没有直接使用（通过 `should_use_gamescope`），不要在 `env.rs` 顶部导入它；测试里通过 `launcher_core::GamescopeMode` 使用。把实现中的 `use launcher_core::GamescopeMode;` 删除。

`lib.rs` 增加：

```rust
mod plan;

pub use env::{build_dll_overrides, build_env, validate_dll_name, validate_env_key};
pub use plan::{build_plan, validate, LaunchPlan, LaunchRequest};
```

`LaunchRequest`/`LaunchPlan` 将在 L4 定义；L3 先只测试 `build_env` 时需要它们，必须先创建 `plan.rs` 中的结构定义。在本任务中把结构放到 `plan.rs`，但 `build_plan` 留到 L4：

```rust
use display::SessionType;
use launcher_core::{Game, Runner};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub game: Game,
    pub runner: Runner,
    pub session: SessionType,
    pub global_env: BTreeMap<String, String>,
    pub inherited_env: BTreeMap<String, String>,
    pub steam_compat_client: PathBuf,
    pub default_prefix_root: PathBuf,
    pub gamescope_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
}
```

`pub use plan::{LaunchPlan, LaunchRequest};`（`validate`/`build_plan` 在 L4 加入后再导出）。

- [x] **Step 4: 运行测试**

Run: `cargo test -p launch`
Expected: 17 passed（L1 5 + L2 3 + L3 9）。若某些测试因 `build_env` 未导出失败，检查 `lib.rs`。

- [x] **Step 5: 提交**

```bash
git add crates/launch
git commit -m "feat(launch): compose environment with deterministic precedence"
```

---

## Task L4: 启动前校验与计划构造

**Files:**
- Modify: `crates/launch/src/plan.rs`

- [x] **Step 1: 写校验与计划测试**

在 `plan.rs` 的测试模块中：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{
        Arch, DisplayConfig, GamescopeMode, RunnerKind, RunnerSource,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn request(dir: &tempfile::TempDir) -> LaunchRequest {
        let wine = dir.path().join("wine");
        write_executable(&wine, "#!/bin/sh\nexit 0\n");
        let executable = dir.path().join("game.exe");
        fs::write(&executable, b"MZ").unwrap();

        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        LaunchRequest {
            game: Game {
                id: uuid::Uuid::new_v4(),
                name: "test".into(),
                executable_path: executable.clone(),
                working_dir: Some(dir.path().to_path_buf()),
                runner_id: None,
                prefix_path: Some(prefix),
                arch: Arch::Win64,
                display: DisplayConfig::default(),
                env_overrides: BTreeMap::new(),
                dll_overrides: Vec::new(),
                args: vec!["-skip".into()],
                created_at: 0,
                last_played_at: None,
            },
            runner: Runner {
                id: uuid::Uuid::new_v4(),
                kind: RunnerKind::Wine,
                name: "Wine".into(),
                binary_path: wine,
                root_dir: dir.path().to_path_buf(),
                arch: Arch::Win64,
                source: RunnerSource::System,
                verified: true,
                version_string: None,
            },
            session: SessionType::X11,
            global_env: BTreeMap::new(),
            inherited_env: BTreeMap::new(),
            steam_compat_client: dir.path().join("steam-client"),
            default_prefix_root: dir.path().join("prefixes"),
            gamescope_path: None,
        }
    }

    #[test]
    fn missing_executable_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.executable_path = dir.path().join("missing.exe");
        assert!(matches!(
            validate(&request),
            Err(LaunchError::ExecutableMissing(_))
        ));
    }

    #[test]
    fn missing_runner_binary_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        fs::remove_file(&request.runner.binary_path).unwrap();
        assert!(matches!(
            validate(&request),
            Err(LaunchError::Runner(runner_wine::RunnerError::MissingBinary(_)))
        ));
    }

    #[test]
    fn missing_working_directory_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.working_dir = Some(dir.path().join("missing"));
        assert!(matches!(
            validate(&request),
            Err(LaunchError::WorkingDirMissing(_))
        ));
    }

    #[test]
    fn uninitialized_prefix_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        let prefix = dir.path().join("empty-prefix");
        fs::create_dir(&prefix).unwrap();
        request.game.prefix_path = Some(prefix.clone());
        assert!(matches!(
            validate(&request),
            Err(LaunchError::PrefixNotInitialized(path)) if path == prefix
        ));
    }

    #[test]
    fn arch_mismatch_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        fs::write(
            request.game.prefix_path.as_ref().unwrap().join("system.reg"),
            "#arch=win32\n",
        )
        .unwrap();
        assert!(matches!(
            validate(&request),
            Err(LaunchError::PrefixArchMismatch {
                existing: Some(Arch::Win32),
                ..
            })
        ));
    }

    #[test]
    fn forced_gamescope_without_binary_is_rejected() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.display.gamescope = GamescopeMode::Forced;
        request.gamescope_path = None;
        assert!(matches!(
            validate(&request),
            Err(LaunchError::GamescopeUnavailable)
        ));
    }

    #[test]
    fn manual_dll_override_is_rejected_before_spawn() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.env_overrides =
            BTreeMap::from([("WINEDLLOVERRIDES".into(), "x=n".into())]);
        assert!(matches!(
            validate(&request),
            Err(LaunchError::DllOverrideConflict)
        ));
    }

    #[test]
    fn proton_game_uses_win64_prefix_even_for_win32_game() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.runner.kind = RunnerKind::Proton;
        request.runner.binary_path = dir.path().join("proton");
        write_executable(&request.runner.binary_path, "#!/bin/sh\nexit 0\n");
        request.game.prefix_path = Some(dir.path().join("compat"));
        fs::create_dir(dir.path().join("compat/pfx")).unwrap();
        fs::write(
            dir.path().join("compat/pfx/system.reg"),
            "#arch=win64\n",
        )
        .unwrap();
        request.game.arch = Arch::Win32;

        validate(&request).unwrap();
    }

    #[test]
    fn plan_without_gamescope_uses_runner_command() {
        let dir = tempdir().unwrap();
        let request = request(&dir);
        let wine = request.runner.binary_path.clone();
        let exe = request.game.executable_path.clone();
        let working_dir = request.game.working_dir.clone();

        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.program, wine);
        assert_eq!(plan.args[0], exe.to_string_lossy().into_owned());
        assert_eq!(plan.args[1], "-skip");
        assert_eq!(plan.cwd, working_dir);
    }

    #[test]
    fn plan_with_gamescope_wraps_runner_command() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        let gamescope = dir.path().join("gamescope");
        write_executable(&gamescope, "#!/bin/sh\nexit 0\n");
        request.gamescope_path = Some(gamescope.clone());
        request.session = SessionType::Wayland;
        request.game.display = DisplayConfig {
            resolution: Some((2560, 1440)),
            fullscreen: true,
            gamescope: GamescopeMode::Forced,
            ..DisplayConfig::default()
        };

        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.program, gamescope);
        assert!(plan
            .args
            .windows(2)
            .any(|pair| pair[0] == "--backend" && pair[1] == "wayland"));
        let separator = plan.args.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(
            plan.args[separator + 1],
            request.runner.binary_path.to_string_lossy().into_owned()
        );
        assert_eq!(
            plan.args[separator + 2],
            request.game.executable_path.to_string_lossy().into_owned()
        );
    }

    #[test]
    fn cwd_falls_back_to_executable_parent() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.working_dir = None;
        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.cwd, request.game.executable_path.parent().map(Path::to_path_buf));
    }

    #[test]
    fn x11_monitor_env_is_present_in_plan() {
        let dir = tempdir().unwrap();
        let mut request = request(&dir);
        request.game.display.monitor = Some("eDP-1".into());
        let plan = build_plan(&request).unwrap();
        assert_eq!(plan.env["__GL_SYNC_DISPLAY_DEVICE"], "eDP-1");
    }
}
```


- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p launch`
Expected: 编译失败，`validate`/`build_plan` 未定义。

- [x] **Step 3: 实现**

在 `plan.rs` 追加：

```rust
use crate::env::{validate_dll_name, validate_env_key};
use crate::{build_env, LaunchError};
use display::{build_gamescope_args, should_use_gamescope, SessionType};
use launcher_core::GamescopeMode;
use prefix::{plan_for_game, status, PrefixStatus};
use runner_wine::{verify_runner, WineContext};
use std::path::Path;

pub fn validate(request: &LaunchRequest) -> Result<(), LaunchError> {
    if !request.game.executable_path.is_file() {
        return Err(LaunchError::ExecutableMissing(
            request.game.executable_path.clone(),
        ));
    }

    verify_runner(&request.runner)?;

    if let Some(working_dir) = &request.game.working_dir {
        if !working_dir.is_dir() {
            return Err(LaunchError::WorkingDirMissing(working_dir.clone()));
        }
    }

    let prefix_plan = plan_for_game(
        &request.game,
        request.runner.clone(),
        &request.default_prefix_root,
        request.steam_compat_client.clone(),
    );
    match status(&prefix_plan) {
        PrefixStatus::Initialized => {}
        PrefixStatus::ArchMismatch { existing } => {
            return Err(LaunchError::PrefixArchMismatch {
                expected: prefix_plan.arch,
                existing,
            });
        }
        PrefixStatus::Missing | PrefixStatus::NotInitialized => {
            return Err(LaunchError::PrefixNotInitialized(prefix_plan.path));
        }
    }

    if request.game.display.gamescope == GamescopeMode::Forced
        && request.gamescope_path.is_none()
    {
        return Err(LaunchError::GamescopeUnavailable);
    }

    if request.global_env.contains_key("WINEDLLOVERRIDES")
        || request.game.env_overrides.contains_key("WINEDLLOVERRIDES")
    {
        return Err(LaunchError::DllOverrideConflict);
    }

    for key in request
        .global_env
        .keys()
        .chain(request.game.env_overrides.keys())
    {
        validate_env_key(key)?;
    }
    for entry in &request.game.dll_overrides {
        validate_dll_name(&entry.dll_name)?;
    }

    Ok(())
}

pub fn build_plan(request: &LaunchRequest) -> Result<LaunchPlan, LaunchError> {
    validate(request)?;

    let prefix_plan = plan_for_game(
        &request.game,
        request.runner.clone(),
        &request.default_prefix_root,
        request.steam_compat_client.clone(),
    );
    let context = WineContext {
        runner: request.runner.clone(),
        prefix: prefix_plan.path,
        arch: prefix_plan.arch,
        steam_compat_client: request.steam_compat_client.clone(),
    };

    let (mut program, mut args) = context.build_command(
        &request.game.executable_path,
        &request.game.args,
    );

    let gamescope_available = request.gamescope_path.is_some();
    if should_use_gamescope(
        &request.game.display,
        request.session,
        gamescope_available,
    ) {
        let gamescope = request
            .gamescope_path
            .clone()
            .ok_or(LaunchError::GamescopeUnavailable)?;
        let mut gamescope_args =
            build_gamescope_args(&request.game.display, request.session, gamescope_available);
        gamescope_args.push(program.to_string_lossy().into_owned());
        gamescope_args.extend(args);
        program = gamescope;
        args = gamescope_args;
    }

    let env = build_env(request)?;
    let cwd = request.game.working_dir.clone().or_else(|| {
        request
            .game
            .executable_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
    });

    Ok(LaunchPlan {
        program,
        args,
        env,
        cwd,
    })
}
```

`LaunchError::Arch` import? `validate` doesn't directly name `Arch`; the error variant uses it in error.rs. `use launcher_core::{Arch, GamescopeMode};` only `GamescopeMode` needed. Remove Arch.

`lib.rs` 更新为：

```rust
mod env;
mod error;
mod plan;
mod resolve;

pub use env::{build_dll_overrides, build_env, validate_dll_name, validate_env_key};
pub use error::LaunchError;
pub use plan::{build_plan, validate, LaunchPlan, LaunchRequest};
pub use resolve::resolve_runner;
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p launch`
Expected: 29 passed（5 + 3 + 9 + 12）。

- [x] **Step 5: 提交**

```bash
git add crates/launch
git commit -m "feat(launch): validate launch requests and build launch plans"
```

---

## Task L5: `spawn_plan`

**Files:**
- Create: `crates/launch/src/spawn.rs`
- Modify: `crates/launch/src/lib.rs`

- [x] **Step 1: 写测试**

`crates/launch/src/spawn.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn clears_environment_and_returns_child_status() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("out.txt");
        let plan = LaunchPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec![
                "-c".into(),
                format!(
                    "printf '%s|%s' \"$FOO\" \"$HOME\" > '{}'",
                    output.display()
                ),
            ],
            env: BTreeMap::from([("FOO".into(), "bar".into())]),
            cwd: None,
        };

        let mut child = spawn_plan(&plan).await.unwrap();
        let status = child.wait().await.unwrap();
        assert!(status.success());
        assert_eq!(fs::read_to_string(output).unwrap(), "bar|");
    }

    #[tokio::test]
    async fn propagates_exit_code() {
        let plan = LaunchPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), "exit 7".into()],
            env: BTreeMap::new(),
            cwd: None,
        };
        let mut child = spawn_plan(&plan).await.unwrap();
        assert_eq!(child.wait().await.unwrap().code(), Some(7));
    }

    #[tokio::test]
    async fn missing_program_is_a_spawn_error() {
        let plan = LaunchPlan {
            program: PathBuf::from("/definitely/not/here"),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
        };
        assert!(matches!(
            spawn_plan(&plan).await,
            Err(LaunchError::Spawn { .. })
        ));
    }
}
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p launch`
Expected: 编译失败，`spawn_plan` 未定义。

- [x] **Step 3: 实现**

```rust
use crate::{LaunchError, LaunchPlan};
use std::process::Stdio;
use tokio::process::{Child, Command};

pub async fn spawn_plan(plan: &LaunchPlan) -> Result<Child, LaunchError> {
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .env_clear()
        .envs(&plan.env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(false);

    if let Some(cwd) = &plan.cwd {
        command.current_dir(cwd);
    }

    command
        .spawn()
        .map_err(|source| LaunchError::Spawn {
            program: plan.program.clone(),
            source,
        })
}
```

`lib.rs` 增加：

```rust
mod spawn;

pub use spawn::spawn_plan;
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p launch`
Expected: 32 passed。

- [x] **Step 5: 提交**

```bash
git add crates/launch
git commit -m "feat(launch): spawn launch plans with deterministic environment"
```

---

## Task L6: 收尾

**Files:**
- Modify: `CHANGELOG.md`

- [x] **Step 1: 全量检查**

Run:

```bash
cargo fmt --all
cargo clippy -p launch --all-targets -- -D warnings
cargo test -p launch
```

Expected: 全部通过。

- [x] **Step 2: 更新 CHANGELOG**

```markdown
## P6 — 启动管线

- runner 解析：游戏级 → 全局默认 → 明确错误。
- `WINEDLLOVERRIDES` 由结构化 override 生成；`Disabled` 映射为 `disabled`。
- 环境优先级：继承 → base → global → game → monitor → DLL override。
- 启动前校验 exe/runner/working_dir/prefix/arch/gamescope。
- gamescope 包装与 `spawn_plan` 的 `env_clear`。
- Proton 的 prefix arch 固定 win64。

验证：`cargo test -p launch`。
```

- [x] **Step 3: 提交**

```bash
git add crates/launch CHANGELOG.md
git commit -m "chore(launch): finish launch phase"
```

---

## P6 验收

- [x] `cargo test -p launch` 通过。
- [x] `resolve_runner` 不自动挑唯一 runner；没有选择/默认时报 `NoRunnerSelected`。
- [x] `Disabled` 输出 `disabled`，不是 `n`。
- [x] 继承环境中的 `WINEDLLOVERRIDES` 被删除；全局/游戏级手动设置则拒绝启动。
- [x] 优先级测试覆盖 `base < global < game`。
- [x] X11 且未使用 gamescope 时才注入 `__GL_SYNC_DISPLAY_DEVICE`。
- [x] gamescope 生效时参数以 `--` 分隔，后面紧跟 runner 命令。
- [x] `spawn_plan` 使用 `env_clear`，stdin/stdout 为 null，stderr 继承。
- [x] Proton + Win32 游戏仍使用 win64 prefix，并能通过校验。

## 实施记录

- gamescope：本机未安装（`command -v gamescope` 无结果），包装逻辑由
  plan 测试中的伪造可执行文件覆盖；Forced 且缺失二进制的拒绝路径有独立测试。
- spawn：`spawn_plan` 的三个测试连续运行 10 次全部通过（每次 3 passed），
  验证了 `env_clear`（HOME 为空、FOO 保留）、退出码传递与缺失程序错误。
- 测试计数：L1=5、L2=8（累计）、L3=17、L4=29、L5=32，与计划一致；
  workspace 合计 130 passed、1 ignored。
- 偏差：
  - L1 测试片段漏了 `std::path::PathBuf` 导入，已补；
    默认 runner 测试的切片改用 `std::slice::from_ref` 以通过
    `clippy::cloned_ref_to_slice_refs`。
  - L4 Proton 测试的 `fs::create_dir("compat/pfx")` 因父目录不存在会 panic，
    改为 `create_dir_all`。
  - L4 两个测试的 `let mut request` 未发生修改，去掉 `mut`。
  - `validate` 的 working_dir 检查用 `filter` 形式替代嵌套 `if`，
    避免 `clippy::collapsible_if`，行为不变。

## 当前实现（v0.10.0）

- 契约以本文「固定契约」为准：`runner: Option<Runner>`、`LaunchTuning`、`log_path`、`dxvk_nvapi_source` 是 P6 之后新增的字段；Task L3/L4 中的 `LaunchRequest` 构造片段缺少它们，属历史记录。
- 平台分支：
  - `Platform::Windows`：必须有 runner；校验 prefix 状态与 arch；生成 `WINEPREFIX/WINEARCH` 或 Proton 的 `STEAM_COMPAT_*`；合并调优、DLL override。
  - `Platform::Linux`：`runner = None`；直接执行 `executable_path`；不读取 prefix、不要求 wineboot、不注入任何 Wine 变量、不应用 DLL override；显示选择、gamescope、MangoHud、全局/游戏环境变量和参数照常生效。
  - 切回 Windows 时，之前保存的 runner/prefix/DLL 值仍在 `Game` 中并重新生效（UI 在原生平台只隐藏、不清空）。
- 调优环境：
  - Wine：`enable_esync → WINEESYNC=1`、`enable_fsync → WINEFSYNC=1`、`enable_dxvk_nvapi → DXVK_ENABLE_NVAPI=1 + DXVK_NVAPI_ALLOW_OTHER_DRIVERS=1`。
  - Proton：关闭 esync/fsync 时写 `PROTON_NO_ESYNC/PROTON_NO_FSYNC=1`；NVAPI 开启写 `PROTON_ENABLE_NVAPI`，关闭写 `PROTON_DISABLE_NVAPI`；两者都带 `DXVK_NVAPI_ALLOW_OTHER_DRIVERS=1`。
  - 游戏级环境变量可覆盖调优默认值；继承环境中的调优变量先被删除。
- 显示环境：gamescope 生效时不注入显示器变量；非 gamescope 时按会话/XWayland 选择 `__GL_SYNC_DISPLAY_DEVICE`、`WAYLANDDRV_PRIMARY_MONITOR`、`PROTON_*WAYLAND*`，Wine/Proton 走 X11 时同时设置 `WINE_GRAPHICS_DRIVER=x11` 并移除 Wayland 显示器变量。
- `spawn_plan_with_log` 在 `spawn_plan` 基础上把 stdout/stderr 重定向到 `logs/games/<game-id>.log`：新日志带时间戳与完整命令行，上一次日志保留为 `.log.old`；保持 `env_clear`、独立进程组与 `kill_on_drop(false)`。打开日志失败时退回继承 stderr，不阻止启动。
- 应用层使用 `validate` + `build_plan` + `spawn_plan_with_log`；`LaunchOutcome` 增加 `stopped` 与 `session_seconds`，`LaunchEvent` 负责 Started/Finished/Failed。
