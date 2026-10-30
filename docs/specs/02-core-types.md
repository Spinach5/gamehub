# P1 核心类型 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `launcher-core` 中定义所有跨 crate 共享的领域类型、serde 契约与字符串映射；后续所有 crate 的签名必须与本文一致。

**Architecture:** 只有 `crates/launcher-core/src/types.rs` 一个领域类型文件。枚举的 serde 表示与数据库字符串表示保持同一套 `snake_case`；`RunnerSource` 使用内部 tag；不在这里放任何 IO 或业务规则（DLL override 字符串拼接属于 `launch`）。

**Tech Stack:** serde、uuid、thiserror。

**前置条件：** P0 完成，`launcher-core` 已存在并导出 `AppPaths`、`truncate_utf8`。

---

## 固定类型契约（后续文档以此为准）

```rust
pub enum Arch { Win32, Win64 }
pub enum Platform { Windows, Linux }          // #[default] Windows；snake_case；is_native() == Linux
pub enum RunnerKind { Wine, Proton }
pub enum RunnerSource { System, Imported { original_path: PathBuf } }
pub struct Runner {
    pub id: Uuid,
    pub kind: RunnerKind,
    pub name: String,
    pub binary_path: PathBuf,
    pub root_dir: PathBuf,
    pub arch: Arch,
    pub source: RunnerSource,
    pub verified: bool,
    pub version_string: Option<String>,
}
pub enum OverrideMode { Disabled, Native, Builtin, NativeBuiltin }
pub struct DllOverride { pub dll_name: String, pub mode: OverrideMode }
pub enum DisplayBackend { Auto, X11, Wayland }
pub enum GamescopeMode { Off, Auto, Forced }
pub struct DisplayConfig {
    pub backend: DisplayBackend,
    pub monitor: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub refresh_rate: Option<u32>,
    pub fullscreen: bool,
    pub gamescope: GamescopeMode,
}
pub struct Game {
    pub id: Uuid,
    pub name: String,
    pub executable_path: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub runner_id: Option<Uuid>,
    pub prefix_path: Option<PathBuf>,
    pub cover_path: Option<PathBuf>,          // v0.4.0 起
    pub platform: Platform,                   // 未发版：Linux 原生支持
    pub arch: Arch,
    pub display: DisplayConfig,
    pub env_overrides: BTreeMap<String, String>,
    pub dll_overrides: Vec<DllOverride>,
    pub args: Vec<String>,
    pub created_at: i64,
    pub last_played_at: Option<i64>,          // 本次启动时间（v0.6.0 起语义）
    pub playtime_seconds: i64,                // v0.6.0 起
}
```

不要改字段名。可以增加 `PartialEq`/`Eq`/`Hash` 派生。

`Platform` 的映射与 `Arch` 一致：serde 输出 `"windows"` / `"linux"`，`as_str()` 与 serde 一致，`parse()` 只接受小写；`Default` 为 `Windows`，因此 v3 及更早的数据库升级后全部视为 Windows。

> 下文 Task C1–C4 的代码片段是 P1 实施时的历史记录，缺少 `Platform`、`cover_path`、`playtime_seconds`；以本契约与 `crates/launcher-core/src/types.rs` 为准。

---

## Task C1: `Arch`、`RunnerKind`、`RunnerSource`

**Files:**
- Create: `crates/launcher-core/src/types.rs`
- Modify: `crates/launcher-core/src/lib.rs`

- [ ] **Step 1: 写失败测试**

`crates/launcher-core/src/types.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn arch_serializes_as_lowercase() {
        assert_eq!(serde_json::to_value(Arch::Win32).unwrap(), json!("win32"));
        assert_eq!(serde_json::to_value(Arch::Win64).unwrap(), json!("win64"));
    }

    #[test]
    fn arch_parse_is_exact() {
        assert_eq!(Arch::parse("win32"), Some(Arch::Win32));
        assert_eq!(Arch::parse("win64"), Some(Arch::Win64));
        assert_eq!(Arch::parse("Win64"), None);
        assert_eq!(Arch::parse("64"), None);
    }

    #[test]
    fn arch_as_str_matches_serde() {
        for arch in [Arch::Win32, Arch::Win64] {
            assert_eq!(
                serde_json::to_value(arch).unwrap(),
                json!(arch.as_str())
            );
        }
    }

    #[test]
    fn runner_source_shapes_are_fixed() {
        assert_eq!(
            serde_json::to_value(RunnerSource::System).unwrap(),
            json!({ "kind": "system" })
        );
        assert_eq!(
            serde_json::to_value(RunnerSource::Imported {
                original_path: PathBuf::from("/opt/GE-Proton9-1")
            })
            .unwrap(),
            json!({ "kind": "imported", "original_path": "/opt/GE-Proton9-1" })
        );
    }

    #[test]
    fn runner_source_round_trips() {
        let value = RunnerSource::Imported {
            original_path: PathBuf::from("/games/proton"),
        };
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<RunnerSource>(&encoded).unwrap(), value);
    }
}
```

`crates/launcher-core/src/lib.rs` 增加：

```rust
pub mod types;
pub use types::*;
```

- [ ] **Step 2: 运行并确认失败**

Run: `cargo test -p launcher-core types`
Expected: 编译失败，`Arch` 等未定义。

- [ ] **Step 3: 实现**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    Win32,
    Win64,
}

impl Arch {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Win32 => "win32",
            Self::Win64 => "win64",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "win32" => Some(Self::Win32),
            "win64" => Some(Self::Win64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RunnerKind {
    Wine,
    Proton,
}

impl RunnerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wine => "wine",
            Self::Proton => "proton",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "wine" => Some(Self::Wine),
            "proton" => Some(Self::Proton),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunnerSource {
    System,
    Imported { original_path: PathBuf },
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p launcher-core`
Expected: 通过。

- [ ] **Step 5: 提交**

```bash
git add crates/launcher-core
git commit -m "feat(launcher-core): add arch, runner kind and runner source types"
```

---

## Task C2: 显示配置与 DLL override 类型

**Files:**
- Modify: `crates/launcher-core/src/types.rs`

- [ ] **Step 1: 追加失败测试**

在 `tests` 模块中追加：

```rust
    #[test]
    fn display_config_defaults_are_fixed() {
        let config = DisplayConfig::default();
        assert_eq!(config.backend, DisplayBackend::Auto);
        assert_eq!(config.monitor, None);
        assert_eq!(config.resolution, None);
        assert_eq!(config.refresh_rate, None);
        assert!(config.fullscreen);
        assert_eq!(config.gamescope, GamescopeMode::Auto);
    }

    #[test]
    fn display_config_json_shape_is_fixed() {
        let config = DisplayConfig {
            backend: DisplayBackend::Wayland,
            monitor: Some("eDP-1".into()),
            resolution: Some((1920, 1080)),
            refresh_rate: Some(144),
            fullscreen: true,
            gamescope: GamescopeMode::Forced,
        };
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            json!({
                "backend": "wayland",
                "monitor": "eDP-1",
                "resolution": [1920, 1080],
                "refresh_rate": 144,
                "fullscreen": true,
                "gamescope": "forced"
            })
        );
    }

    #[test]
    fn override_modes_serialize_as_snake_case() {
        for (mode, expected) in [
            (OverrideMode::Disabled, "disabled"),
            (OverrideMode::Native, "native"),
            (OverrideMode::Builtin, "builtin"),
            (OverrideMode::NativeBuiltin, "native_builtin"),
        ] {
            assert_eq!(serde_json::to_value(mode).unwrap(), json!(expected));
        }
    }

    #[test]
    fn dll_override_round_trips() {
        let value = DllOverride {
            dll_name: "steam_api64".into(),
            mode: OverrideMode::NativeBuiltin,
        };
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<DllOverride>(&encoded).unwrap(), value);
    }
```

- [ ] **Step 2: 运行并确认失败**

Run: `cargo test -p launcher-core`
Expected: 编译失败，`DisplayConfig` 等未定义。

- [ ] **Step 3: 实现**

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OverrideMode {
    Disabled,
    Native,
    Builtin,
    NativeBuiltin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DllOverride {
    pub dll_name: String,
    pub mode: OverrideMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DisplayBackend {
    Auto,
    X11,
    Wayland,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GamescopeMode {
    Off,
    Auto,
    Forced,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayConfig {
    pub backend: DisplayBackend,
    pub monitor: Option<String>,
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
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p launcher-core`
Expected: 通过。

- [ ] **Step 5: 提交**

```bash
git add crates/launcher-core
git commit -m "feat(launcher-core): add display config and dll override types"
```

---

## Task C3: `Runner` 与 `Game`

**Files:**
- Modify: `crates/launcher-core/src/types.rs`

- [ ] **Step 1: 追加失败测试**

```rust
    fn sample_runner() -> Runner {
        Runner {
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
        }
    }

    fn sample_game() -> Game {
        Game {
            id: Uuid::new_v4(),
            name: "赛博朋克 2077".into(),
            executable_path: PathBuf::from("/games/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe"),
            working_dir: Some(PathBuf::from("/games/Cyberpunk 2077/bin/x64")),
            runner_id: Some(Uuid::new_v4()),
            prefix_path: None,
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            env_overrides: BTreeMap::from([
                ("WINEDEBUG".into(), "-all".into()),
                ("DXVK_HUD".into(), "fps".into()),
            ]),
            dll_overrides: vec![
                DllOverride {
                    dll_name: "d3d11".into(),
                    mode: OverrideMode::NativeBuiltin,
                },
                DllOverride {
                    dll_name: "winmm".into(),
                    mode: OverrideMode::Native,
                },
            ],
            args: vec!["-skipStartScreen".into(), "--windowed".into()],
            created_at: 1_700_000_000,
            last_played_at: Some(1_700_000_100),
        }
    }

    #[test]
    fn runner_round_trips() {
        let value = sample_runner();
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Runner>(&encoded).unwrap(), value);
    }

    #[test]
    fn game_round_trips_with_all_fields() {
        let value = sample_game();
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Game>(&encoded).unwrap(), value);
    }

    #[test]
    fn game_json_uses_fixed_field_names() {
        let value = serde_json::to_value(sample_game()).unwrap();
        for key in [
            "id",
            "name",
            "executable_path",
            "working_dir",
            "runner_id",
            "prefix_path",
            "arch",
            "display",
            "env_overrides",
            "dll_overrides",
            "args",
            "created_at",
            "last_played_at",
        ] {
            assert!(value.get(key).is_some(), "missing field {key}");
        }
    }
```

- [ ] **Step 2: 运行并确认失败**

Run: `cargo test -p launcher-core`
Expected: 编译失败，`Runner`/`Game` 未定义。

- [ ] **Step 3: 实现**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Runner {
    pub id: Uuid,
    pub kind: RunnerKind,
    pub name: String,
    pub binary_path: PathBuf,
    pub root_dir: PathBuf,
    pub arch: Arch,
    pub source: RunnerSource,
    pub verified: bool,
    pub version_string: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Game {
    pub id: Uuid,
    pub name: String,
    pub executable_path: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub runner_id: Option<Uuid>,
    pub prefix_path: Option<PathBuf>,
    pub arch: Arch,
    pub display: DisplayConfig,
    pub env_overrides: BTreeMap<String, String>,
    pub dll_overrides: Vec<DllOverride>,
    pub args: Vec<String>,
    pub created_at: i64,
    pub last_played_at: Option<i64>,
}
```

文件顶部导入：

```rust
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;
```

测试模块需要 `use uuid::Uuid;`。

- [ ] **Step 4: 运行测试**

Run: `cargo test -p launcher-core`
Expected: 通过。

- [ ] **Step 5: 提交**

```bash
git add crates/launcher-core
git commit -m "feat(launcher-core): add runner and game domain types"
```

---

## Task C4: 导出与契约回归

**Files:**
- Modify: `crates/launcher-core/src/lib.rs`
- Create: `crates/launcher-core/tests/contracts.rs`

- [ ] **Step 1: 确认 `lib.rs` 完整导出**

```rust
pub mod error;
pub mod paths;
pub mod text;
pub mod types;

pub use error::AppPathsError;
pub use paths::AppPaths;
pub use text::truncate_utf8;
pub use types::*;
```

- [ ] **Step 2: 写跨模块契约测试**

`crates/launcher-core/tests/contracts.rs`：

```rust
use launcher_core::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn default_game_config_is_usable_without_database() {
    let game = Game {
        id: uuid::Uuid::nil(),
        name: "test".into(),
        executable_path: PathBuf::from("/tmp/test.exe"),
        working_dir: None,
        runner_id: None,
        prefix_path: None,
        arch: Arch::Win64,
        display: DisplayConfig::default(),
        env_overrides: BTreeMap::new(),
        dll_overrides: Vec::new(),
        args: Vec::new(),
        created_at: 0,
        last_played_at: None,
    };

    let encoded = serde_json::to_value(&game).unwrap();
    assert_eq!(encoded["display"]["gamescope"], "auto");
    assert_eq!(encoded["arch"], "win64");
}

#[test]
fn unknown_enum_values_are_rejected() {
    assert!(serde_json::from_str::<Arch>("\"win16\"").is_err());
    assert!(serde_json::from_str::<RunnerKind>("\"umu\"").is_err());
    assert!(serde_json::from_str::<OverrideMode>("\"prefer_native\"").is_err());
    assert!(serde_json::from_str::<DisplayBackend>("\"drm\"").is_err());
    assert!(serde_json::from_str::<GamescopeMode>("\"always\"").is_err());
}
```

`uuid` 只在 `[dev-dependencies]` 里有？P0 只在 dependencies 中加了 uuid（launcher-core 的依赖包含 uuid），所以集成测试可用。`serde_json` 在 dev-dependencies。

- [ ] **Step 3: 运行全部测试**

Run: `cargo test -p launcher-core`
Expected: 全部通过。

- [ ] **Step 4: 提交**

```bash
git add crates/launcher-core
git commit -m "test(launcher-core): lock serialization contracts"
```

---

## P1 验收

- [x] `cargo test -p launcher-core` 通过。
- [x] 所有字段名与本文“固定类型契约”一致。
- [x] `Arch::as_str()` 与 serde 输出一致，`parse` 只接受小写。
- [x] `RunnerSource::System` 的 JSON 是 `{"kind":"system"}`。
- [x] `DisplayConfig::default()` 与 v1 方案一致（Auto/None/None/None/true/Auto）。

## 实施记录

- 2026-10-26 完成。20 个单元测试 + 2 个集成测试通过；无偏差。

## 当前实现（v0.8.0）

- `types.rs` 仍只有领域类型，没有 IO；新增 `Platform` 与上述三个 `Game` 字段。
- 契约测试 `game_json_uses_fixed_field_names` 的 key 列表已包含 `cover_path`、`platform`、`playtime_seconds`；另有 `platform_serializes_as_snake_case`、`platform_parse_is_exact`、`platform_as_str_matches_serde`、`platform_defaults_to_windows_and_reports_native`、`game_round_trips_with_cover_path`。
- `Runner` 未变；runner 的「可停止性」与「prefix 是否有进程」放在 `runner-wine`，不进入领域类型。
- 类型导出经 `application` 再导出给 `ui`；UI 不直接依赖 `storage`/`prefix`/`launch`。
