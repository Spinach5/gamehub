# P5 显示会话、xrandr 与 gamescope Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 实现 `display`：会话类型检测、`xrandr --query` 解析、PATH 中查找 gamescope、按显示配置生成 gamescope 参数。

**Architecture:** 四个纯函数模块 + 两个命令包装。解析与参数生成必须可注入文本/可用性，测试不依赖图形环境；`list_x11_outputs` 与 `gamescope_path` 是唯一接触系统的地方。

**Tech Stack:** launcher-core、std::process、tracing。

**前置条件：** P1 完成。

---

## 固定契约

```rust
pub enum SessionType { X11, Wayland, Unknown }

pub fn session_from(value: Option<&str>) -> SessionType;
pub fn detect_session() -> SessionType;

pub struct MonitorOutput {
    pub name: String,
    pub connected: bool,
    pub active: bool,
    pub primary: bool,
    pub position: Option<(i32, i32)>,
    pub current_mode: Option<(u32, u32, Option<u32>)>,
}

pub fn parse_xrandr_output(text: &str) -> Vec<MonitorOutput>;
pub fn list_outputs() -> Vec<MonitorOutput>;
pub fn parse_wayland_info_outputs(text: &str) -> Vec<MonitorOutput>;

pub fn find_in_path(program: &str, path_env: Option<&OsStr>) -> Option<PathBuf>;
pub fn gamescope_path() -> Option<PathBuf>;
pub fn mangohud_path() -> Option<PathBuf>;

/// 临时把 `connector` 设为主屏；返回原主屏或 `None`（已是主屏）。
pub fn set_primary_output(
    connector: &str,
    session: SessionType,
) -> Result<Option<String>, DisplayError>;

pub fn should_use_gamescope(
    config: &DisplayConfig,
    session: SessionType,
    available: bool,
) -> bool;

pub fn build_gamescope_args(
    config: &DisplayConfig,
    session: SessionType,
    available: bool,
    mangohud: bool,
) -> Vec<String>;
```

> P5 时的 `X11Output` / `list_x11_outputs` 已演进为 `MonitorOutput` / `list_outputs`；Wayland 无 XWayland 时用 `parse_wayland_info_outputs` 解析 GNOME `DisplayConfig` 的输出列表。下文 Task 代码保留 P5 历史签名。

---

## Task D1: 会话检测

**Files:**
- Create: `crates/display/src/session.rs`
- Modify: `crates/display/src/lib.rs`

- [x] **Step 1: 写失败测试**

`crates/display/src/session.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_x11_and_wayland_exactly() {
        assert_eq!(session_from(Some("x11")), SessionType::X11);
        assert_eq!(session_from(Some("wayland")), SessionType::Wayland);
    }

    #[test]
    fn unknown_and_missing_are_unknown() {
        assert_eq!(session_from(None), SessionType::Unknown);
        assert_eq!(session_from(Some("")), SessionType::Unknown);
        assert_eq!(session_from(Some("X11")), SessionType::Unknown);
        assert_eq!(session_from(Some("tty")), SessionType::Unknown);
    }
}
```

`crates/display/src/lib.rs` 暂时：

```rust
mod session;

pub use session::{detect_session, session_from, SessionType};
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p display`
Expected: 编译失败，`SessionType` 未定义。

- [x] **Step 3: 实现**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

pub fn session_from(value: Option<&str>) -> SessionType {
    match value {
        Some("x11") => SessionType::X11,
        Some("wayland") => SessionType::Wayland,
        _ => SessionType::Unknown,
    }
}

pub fn detect_session() -> SessionType {
    session_from(std::env::var("XDG_SESSION_TYPE").ok().as_deref())
}
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p display`
Expected: 2 passed。

- [x] **Step 5: 提交**

```bash
git add crates/display
git commit -m "feat(display): detect x11 and wayland sessions"
```

---

## Task D2: xrandr 解析

**Files:**
- Create: `crates/display/src/xrandr.rs`
- Modify: `crates/display/src/lib.rs`

- [x] **Step 1: 写解析测试（使用本机真实输出）**

`crates/display/src/xrandr.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"Screen 0: minimum 16 x 16, current 1920 x 1080, maximum 32767 x 32767
eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 340mm x 190mm
   1920x1080    143.88*+
   1440x1080    143.80
   1280x720     143.67
DP-1 disconnected (normal left inverted right x axis y axis)
HDMI-1 connected (normal left inverted right x axis y axis)
   2560x1440    120.00 +
"#;

    #[test]
    fn parses_active_output_with_fractional_refresh() {
        let outputs = parse_xrandr_output(SAMPLE);
        let edp = outputs.iter().find(|output| output.name == "eDP-1").unwrap();
        assert!(edp.connected);
        assert!(edp.active);
        assert_eq!(edp.current_mode, Some((1920, 1080, Some(144))));
    }

    #[test]
    fn includes_disconnected_and_inactive_outputs() {
        let outputs = parse_xrandr_output(SAMPLE);
        let dp = outputs.iter().find(|output| output.name == "DP-1").unwrap();
        assert!(!dp.connected);
        assert!(!dp.active);
        assert_eq!(dp.current_mode, None);

        let hdmi = outputs
            .iter()
            .find(|output| output.name == "HDMI-1")
            .unwrap();
        assert!(hdmi.connected);
        assert!(!hdmi.active);
        assert_eq!(hdmi.current_mode, None);
    }

    #[test]
    fn malformed_lines_are_ignored() {
        let outputs = parse_xrandr_output("garbage\n   123x\n\x00\n");
        assert!(outputs.is_empty());
    }

    #[test]
    fn multiple_connected_outputs_keep_their_own_modes() {
        let text = r#"Screen 0: minimum 16 x 16, current 3840 x 1080, maximum 32767 x 32767
eDP-1 connected primary 1920x1080+0+0 (normal) 340mm x 190mm
   1920x1080    60.00*+
HDMI-1 connected 1920x1080+1920+0 (normal) 500mm x 300mm
   1920x1080    120.00*+
"#;
        let outputs = parse_xrandr_output(text);
        assert_eq!(outputs.len(), 2);
        assert_eq!(
            outputs[0].current_mode,
            Some((1920, 1080, Some(60)))
        );
        assert_eq!(
            outputs[1].current_mode,
            Some((1920, 1080, Some(120)))
        );
    }
}
```

`lib.rs` 增加：

```rust
mod xrandr;

pub use xrandr::{list_x11_outputs, parse_xrandr_output, X11Output};
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p display`
Expected: 编译失败，`X11Output` 未定义。

- [x] **Step 3: 实现**

```rust
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X11Output {
    pub name: String,
    pub connected: bool,
    pub active: bool,
    pub current_mode: Option<(u32, u32, Option<u32>)>,
}

fn parse_geometry(token: &str) -> Option<(u32, u32)> {
    if !token.contains('+') {
        return None;
    }
    let (width, rest) = token.split_once('x')?;
    let height = rest.split('+').next()?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn parse_refresh(token: &str) -> Option<u32> {
    let rate = token.trim_end_matches(['*', '+']);
    let rate: f32 = rate.parse().ok()?;
    if rate.is_finite() && rate > 0.0 {
        Some(rate.round() as u32)
    } else {
        None
    }
}

pub fn parse_xrandr_output(text: &str) -> Vec<X11Output> {
    let mut outputs: Vec<X11Output> = Vec::new();
    let mut current: Option<usize> = None;

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if !line.chars().next().is_some_and(char::is_whitespace) {
            let mut parts = line.split_whitespace();
            let Some(name) = parts.next() else {
                current = None;
                continue;
            };
            let Some(state) = parts.next() else {
                current = None;
                continue;
            };
            if state != "connected" && state != "disconnected" {
                current = None;
                continue;
            }

            let mut geometry = None;
            for token in parts {
                if let Some(mode) = parse_geometry(token) {
                    geometry = Some(mode);
                    break;
                }
            }

            outputs.push(X11Output {
                name: name.to_string(),
                connected: state == "connected",
                active: geometry.is_some(),
                current_mode: geometry.map(|(width, height)| (width, height, None)),
            });
            current = Some(outputs.len() - 1);
            continue;
        }

        let Some(index) = current else {
            continue;
        };
        let output = &mut outputs[index];
        if !output.active {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(mode_token) = parts.next() else {
            continue;
        };
        let Some((width, height)) = parse_geometry(&format!("{mode_token}+")) else {
            continue;
        };
        let Some((current_width, current_height, refresh)) = output.current_mode else {
            continue;
        };
        if width != current_width || height != current_height {
            continue;
        }

        let is_current = mode_token.contains('*')
            || line.split_whitespace().any(|token| token.contains('*'));
        if !is_current {
            continue;
        }

        let rate = parts
            .find_map(|token| {
                if token.contains('*') {
                    parse_refresh(token)
                } else {
                    None
                }
            });
        output.current_mode = Some((current_width, current_height, rate.or(refresh)));
    }

    outputs
}

pub fn list_x11_outputs() -> Vec<X11Output> {
    let output = match Command::new("xrandr").arg("--query").output() {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(%error, "cannot run xrandr");
            return Vec::new();
        }
    };
    if !output.status.success() {
        tracing::warn!(code = ?output.status.code(), "xrandr exited with failure");
        return Vec::new();
    }
    parse_xrandr_output(&String::from_utf8_lossy(&output.stdout))
}
```

实现说明：

- 输出头行只有当第二个 token 恰好是 `connected`/`disconnected` 时才当作输出行；其他行忽略。
- 活动模式必须带 `+offset`，因此 `1920x1080` 的普通模式行不会被误认为活动几何。
- 模式行的当前标志 `*` 可能在分辨率 token 上（`143.88*+`），也可能单独出现；实现同时检查两者。
- 刷新率取整：`143.88 → 144`；`143.80` 不是当前模式，不采用。
- 无刷新率时 `current_mode = Some((w, h, None))`。

- [x] **Step 4: 运行测试**

Run: `cargo test -p display`
Expected: 6 passed。

- [x] **Step 5: 提交**

```bash
git add crates/display
git commit -m "feat(display): parse xrandr outputs with fractional refresh rates"
```

---

## Task D3: PATH 查找与 gamescope 路径

**Files:**
- Create: `crates/display/src/gamescope.rs`
- Modify: `crates/display/src/lib.rs`

- [x] **Step 1: 写测试**

`crates/display/src/gamescope.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write_executable(path: &std::path::Path) {
        fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn finds_executable_on_path() {
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        let gamescope = second.path().join("gamescope");
        write_executable(&gamescope);

        let path_env = std::env::join_paths([first.path(), second.path()]).unwrap();
        assert_eq!(
            find_in_path("gamescope", Some(path_env.as_os_str())),
            Some(gamescope)
        );
    }

    #[test]
    fn ignores_non_executable_and_missing() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("gamescope");
        fs::write(&file, "not executable").unwrap();

        let path_env = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(find_in_path("gamescope", Some(path_env.as_os_str())), None);
        assert_eq!(find_in_path("missing", Some(path_env.as_os_str())), None);
        assert_eq!(find_in_path("gamescope", None), None);
    }

    #[test]
    fn returns_absolute_paths_from_path_entries() {
        let dir = tempdir().unwrap();
        let gamescope = dir.path().join("gamescope");
        write_executable(&gamescope);
        let path_env = std::env::join_paths([dir.path()]).unwrap();
        let found = find_in_path("gamescope", Some(path_env.as_os_str())).unwrap();
        assert_eq!(found, PathBuf::from(gamescope));
    }
}
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p display`
Expected: 编译失败，`find_in_path` 未定义。

- [x] **Step 3: 实现**

```rust
use launcher_core::{DisplayConfig, GamescopeMode};
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::SessionType;

fn is_executable_file(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

pub fn find_in_path(program: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    let path_env = path_env?;
    for dir in std::env::split_paths(path_env) {
        let candidate = dir.join(program);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn gamescope_path() -> Option<PathBuf> {
    static GAMESCOPE: OnceLock<Option<PathBuf>> = OnceLock::new();
    GAMESCOPE
        .get_or_init(|| {
            let path_env = std::env::var_os("PATH");
            find_in_path("gamescope", path_env.as_deref())
        })
        .clone()
}

pub fn should_use_gamescope(
    config: &DisplayConfig,
    session: SessionType,
    available: bool,
) -> bool {
    match config.gamescope {
        GamescopeMode::Off => false,
        GamescopeMode::Auto => session == SessionType::Wayland && available,
        GamescopeMode::Forced => true,
    }
}

pub fn build_gamescope_args(
    config: &DisplayConfig,
    session: SessionType,
    available: bool,
) -> Vec<String> {
    if !should_use_gamescope(config, session, available) {
        return Vec::new();
    }

    let mut args = Vec::new();
    if let Some((width, height)) = config.resolution {
        args.push("-W".into());
        args.push(width.to_string());
        args.push("-H".into());
        args.push(height.to_string());
    }
    if let Some(rate) = config.refresh_rate {
        args.push("-r".into());
        args.push(rate.to_string());
    }
    if config.fullscreen {
        args.push("-f".into());
    }
    if session == SessionType::Wayland {
        if let Some(monitor) = &config.monitor {
            args.push("-O".into());
            args.push(monitor.clone());
        }
    }
    match session {
        SessionType::Wayland => {
            args.push("--backend".into());
            args.push("wayland".into());
        }
        SessionType::X11 => {
            args.push("--backend".into());
            args.push("x11".into());
        }
        SessionType::Unknown => {}
    }
    args.push("--".into());
    args
}
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p display`
Expected: 9 passed。

- [x] **Step 5: 提交**

```bash
git add crates/display
git commit -m "feat(display): add path lookup and gamescope argument builder"
```

---

## Task D4: gamescope 真值表

**Files:**
- Modify: `crates/display/src/gamescope.rs`

- [x] **Step 1: 追加测试**

```rust
    use launcher_core::{DisplayBackend, GamescopeMode};

    fn config(gamescope: GamescopeMode) -> DisplayConfig {
        DisplayConfig {
            backend: DisplayBackend::Auto,
            monitor: Some("eDP-1".into()),
            resolution: Some((2560, 1440)),
            refresh_rate: Some(144),
            fullscreen: true,
            gamescope,
        }
    }

    #[test]
    fn off_never_uses_gamescope() {
        let config = config(GamescopeMode::Off);
        assert!(!should_use_gamescope(&config, SessionType::Wayland, true));
        assert!(build_gamescope_args(&config, SessionType::Wayland, true).is_empty());
    }

    #[test]
    fn auto_requires_wayland_and_availability() {
        let config = config(GamescopeMode::Auto);
        assert!(should_use_gamescope(&config, SessionType::Wayland, true));
        assert!(!should_use_gamescope(&config, SessionType::Wayland, false));
        assert!(!should_use_gamescope(&config, SessionType::X11, true));
        assert!(!should_use_gamescope(&config, SessionType::Unknown, true));
    }

    #[test]
    fn forced_generates_arguments_even_when_unavailable() {
        let config = config(GamescopeMode::Forced);
        assert!(should_use_gamescope(&config, SessionType::X11, false));
        assert_eq!(
            build_gamescope_args(&config, SessionType::X11, false),
            vec![
                "-W", "2560", "-H", "1440", "-r", "144", "-f", "--backend", "x11", "--"
            ]
        );
    }

    #[test]
    fn wayland_arguments_are_exact_and_include_output() {
        let config = config(GamescopeMode::Auto);
        assert_eq!(
            build_gamescope_args(&config, SessionType::Wayland, true),
            vec![
                "-W", "2560", "-H", "1440", "-r", "144", "-f", "-O", "eDP-1",
                "--backend", "wayland", "--"
            ]
        );
    }

    #[test]
    fn empty_options_only_produce_backend_and_separator() {
        let config = DisplayConfig {
            backend: DisplayBackend::Auto,
            monitor: None,
            resolution: None,
            refresh_rate: None,
            fullscreen: false,
            gamescope: GamescopeMode::Auto,
        };
        assert_eq!(
            build_gamescope_args(&config, SessionType::Wayland, true),
            vec!["--backend", "wayland", "--"]
        );
    }

    #[test]
    fn x11_does_not_pass_output_flag() {
        let config = config(GamescopeMode::Forced);
        let args = build_gamescope_args(&config, SessionType::X11, true);
        assert!(!args.contains(&"-O".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("--"));
    }

    #[test]
    fn unknown_session_omits_backend_flag() {
        let config = config(GamescopeMode::Forced);
        assert_eq!(
            build_gamescope_args(&config, SessionType::Unknown, false),
            vec!["-W", "2560", "-H", "1440", "-r", "144", "-f", "--"]
        );
    }
```

- [x] **Step 2: 运行测试**

Run: `cargo test -p display`
Expected: 16 passed。

- [x] **Step 3: 提交**

```bash
git add crates/display
git commit -m "test(display): cover gamescope argument truth table"
```

---

## Task D5: 收尾

**Files:**
- Modify: `crates/display/src/lib.rs`
- Modify: `CHANGELOG.md`

- [x] **Step 1: 确认导出**

```rust
mod gamescope;
mod session;
mod xrandr;

pub use gamescope::{
    build_gamescope_args, find_in_path, gamescope_path, should_use_gamescope,
};
pub use session::{detect_session, session_from, SessionType};
pub use xrandr::{list_x11_outputs, parse_xrandr_output, X11Output};
```

- [x] **Step 2: 全量检查**

Run:

```bash
cargo fmt --all
cargo clippy -p display --all-targets -- -D warnings
cargo test -p display
```

Expected: 全部通过。

- [x] **Step 3: 更新 CHANGELOG**

```markdown
## P5 — 显示

- XDG 会话检测（x11/wayland/unknown）。
- `xrandr --query` 解析，支持小数刷新率与多输出。
- PATH 中查找 gamescope（可执行位检查）。
- gamescope 参数真值表（Off/Auto/Forced × X11/Wayland/Unknown）。

验证：`cargo test -p display`。
```

- [x] **Step 4: 提交**

```bash
git add crates/display CHANGELOG.md
git commit -m "chore(display): finish display phase"
```

---

## P5 验收

- [x] `cargo test -p display` 通过。
- [x] `XDG_SESSION_TYPE=wayland` 时 `detect_session()` 返回 Wayland；大小写不匹配返回 Unknown。
- [x] 解析本机样例得到 `eDP-1` 活跃、`1920x1080`、刷新率 `144`。
- [x] 断开的输出仍在列表中，`connected=false`、`active=false`。
- [x] `Auto` 只在 Wayland 且 gamescope 可用时生效；`Forced` 总是生成参数。
- [x] X11 参数不包含 `-O`；Unknown 不包含 `--backend`。
- [x] 参数总是以 `--` 结尾。

## 实施记录

- 本机环境：Wayland 会话（`XDG_SESSION_TYPE=wayland`，`DISPLAY=:0`），
  `xrandr --query` 仅有一个输出：
  `HDMI-1 connected primary 1920x1080+0+0`，当前模式 `1920x1080 143.88*+`，
  screen 当前 `3840x1080`。规格 D2 的 `eDP-1` 样例为典型笔记本输出，
  本机无该输出，解析逻辑由该样例覆盖（包含断开/非活动输出与多输出场景）。
- gamescope：本机未安装（`command -v gamescope` 无结果）。
  PATH 查找用临时目录 + 伪造可执行文件覆盖；Auto 的可用性由注入的
  `available` 参数覆盖；`gamescope_path()` 返回 `None` 符合预期。
- 测试计数与计划一致：会话 2、xrandr 4（累计 6）、PATH 3（累计 9）、
  真值表 7（累计 16）。`cargo test --workspace` 为 98 passed、1 ignored。
- 偏差：D3 测试的 `PathBuf::from(gamescope)` 触发 clippy
  `useless_conversion`，改为直接比较并删除未使用的 `PathBuf` 导入；
  其余实现与计划一致。

## 当前实现（v0.11.0）

- 输出枚举：
  - X11 / XWayland：解析 `xrandr --query`，`MonitorOutput` 额外记录 `primary` 与 `position`。
  - Wayland 且无 `DISPLAY`：从 GNOME Mutter `DisplayConfig` 读取物理输出，`parse_wayland_info_outputs` 处理其文本形式，`list_outputs()` 对外统一返回 `MonitorOutput`。
  - 设置页展示全部已连接输出（含未激活），默认选中主屏或第一块活动屏。
- 显示器应用方式（v0.3.0 起）：
  - 使用 gamescope 时由 `-O <connector>` 指定输出，不再注入显示器环境变量。
  - 非 gamescope 时：Wayland 有 XWayland 或 X11 会话，临时把目标输出设为主屏（Mutter `ApplyMonitorsConfig` / `xrandr --output <name> --primary`），并设置 `__GL_SYNC_DISPLAY_DEVICE`；Wine/Proton 同时强制 X11 驱动。
  - Wayland 无 XWayland 时，Wine 使用 `WAYLANDDRV_PRIMARY_MONITOR`，Proton 额外启用 `PROTON_ENABLE_WAYLAND`/`PROTON_WAYLAND_MONITOR`。
  - 游戏退出后恢复原主屏；切换失败只记录 warn，不阻止启动。
- MangoHud：`mangohud_path()` 查找系统 `mangohud`；gamescope 路径改用 `--mangoapp`，非 gamescope 路径包装为 `mangohud --dlsym`。未安装但启用时报 `MangohudUnavailable`。
- `build_gamescope_args` 的第四个参数 `mangohud` 控制是否追加 `--mangoapp`；其余真值表不变。
- 已知限制：非 gamescope 路径不修改分辨率/刷新率；X11 只通过主屏与 `__GL_SYNC_DISPLAY_DEVICE` 选屏。
