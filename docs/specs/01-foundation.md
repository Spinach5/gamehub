# P0 基线、workspace 与冒烟 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把当前单 package 仓库改造成可编译的 7-crate workspace，锁定并通过依赖解析，建立数据目录与日志，用一个 Iced 冒烟程序验证窗口、中文、tokio 执行器与 rfd 均可用。

**Architecture:** 根目录只保留 workspace 配置；所有代码进入 `crates/*`。P0 结束时除 `launcher-core` 的 `AppPaths`/`truncate_utf8` 与 `ui` 的冒烟窗口外，不实现业务逻辑。

**Tech Stack:** Rust edition 2024、Iced 0.13.1、tokio、rusqlite(bundled)、rfd、tracing。

**前置条件：** 无。当前仓库没有 commit，`src/main.rs` 是未跟踪的 Hello World。

---

## Task F1: 转换为 Cargo workspace

**Files:**
- Create: `Cargo.toml`（覆盖根文件）
- Create: `crates/launcher-core/Cargo.toml`
- Create: `crates/launcher-core/src/lib.rs`
- Create: `crates/storage/Cargo.toml`
- Create: `crates/storage/src/lib.rs`
- Create: `crates/runner-wine/Cargo.toml`
- Create: `crates/runner-wine/src/lib.rs`
- Create: `crates/prefix/Cargo.toml`
- Create: `crates/prefix/src/lib.rs`
- Create: `crates/display/Cargo.toml`
- Create: `crates/display/src/lib.rs`
- Create: `crates/launch/Cargo.toml`
- Create: `crates/launch/src/lib.rs`
- Create: `crates/ui/Cargo.toml`
- Move: `src/main.rs` → `crates/ui/src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: 创建目录并移动入口**

```bash
cd /home/zqw/biancheng/Project/gameHub
mkdir -p crates/{launcher-core,storage,runner-wine,prefix,display,launch,ui}/src
mv src/main.rs crates/ui/src/main.rs
rmdir src
```

不要用 `git mv`：当前 `src/main.rs` 是未跟踪文件，`git mv` 会失败。

- [ ] **Step 2: 写根 `Cargo.toml`**

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[workspace.lints.rust]
unsafe_code = "forbid"

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"
```

- [ ] **Step 3: 为 6 个库 crate 写最小 `Cargo.toml`**

以 `crates/launcher-core/Cargo.toml` 为例，其余 5 个仅把 `name` 改成 `storage`、`runner-wine`、`prefix`、`display`、`launch`：

```toml
[package]
name = "launcher-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[lints]
workspace = true

[dependencies]
```

每个库 crate 的 `src/lib.rs` 先写：

```rust
//! gameHub crate. The implementation phase for this crate is listed in docs/specs/00-overview.md.
```

- [ ] **Step 4: 写 `crates/ui/Cargo.toml`**

```toml
[package]
name = "ui"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[lints]
workspace = true

[[bin]]
name = "gamehub"
path = "src/main.rs"

[dependencies]
```

`crates/ui/src/main.rs` 暂时保留：

```rust
fn main() {
    println!("gamehub");
}
```

- [ ] **Step 5: 写 `.gitignore`**

```gitignore
/target
*.db
*.db-wal
*.db-shm
/logs
```

- [ ] **Step 6: 编译验证**

Run: `cargo build --workspace`
Expected: `Finished dev profile`，无错误。首次运行会创建 `Cargo.lock`。

- [ ] **Step 7: 提交**

```bash
git add Cargo.toml Cargo.lock .gitignore crates docs
git commit -m "chore(workspace): convert repository to multi-crate workspace"
```

---

## Task F2: 锁定依赖并验证解析

**Files:**
- Modify: `Cargo.toml`
- Modify: 所有 `crates/*/Cargo.toml`

- [ ] **Step 1: 在根 `Cargo.toml` 增加 `[workspace.dependencies]`**

放在 `[workspace.package]` 之后：

```toml
[workspace.dependencies]
launcher-core = { path = "crates/launcher-core" }
storage = { path = "crates/storage" }
runner-wine = { path = "crates/runner-wine" }
prefix = { path = "crates/prefix" }
display = { path = "crates/display" }
launch = { path = "crates/launch" }

iced = { version = "0.13.1", features = ["tokio", "advanced"] }
tokio = { version = "1.54", features = ["rt-multi-thread", "macros", "process", "sync", "time", "fs"] }
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
tempfile = "3"
```

- [ ] **Step 2: 给 `launcher-core` 加依赖**

```toml
[dependencies]
serde.workspace = true
uuid.workspace = true
thiserror.workspace = true

[dev-dependencies]
serde_json.workspace = true
```

- [ ] **Step 3: 给 `storage` 加依赖**

```toml
[dependencies]
launcher-core.workspace = true
rusqlite.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 4: 给 `runner-wine`、`prefix`、`display`、`launch` 加依赖**

`crates/runner-wine/Cargo.toml`：

```toml
[dependencies]
launcher-core.workspace = true
tokio.workspace = true
thiserror.workspace = true
tracing.workspace = true
uuid.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/prefix/Cargo.toml`：

```toml
[dependencies]
launcher-core.workspace = true
runner-wine.workspace = true
tokio.workspace = true
thiserror.workspace = true
tracing.workspace = true
uuid.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/display/Cargo.toml`：

```toml
[dependencies]
launcher-core.workspace = true
tracing.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`crates/launch/Cargo.toml`：

```toml
[dependencies]
launcher-core.workspace = true
storage.workspace = true
prefix.workspace = true
display.workspace = true
runner-wine.workspace = true
tokio.workspace = true
thiserror.workspace = true
tracing.workspace = true
uuid.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 5: 给 `ui` 加依赖**

```toml
[dependencies]
launcher-core.workspace = true
storage.workspace = true
prefix.workspace = true
display.workspace = true
runner-wine.workspace = true
launch.workspace = true
iced.workspace = true
tokio.workspace = true
rfd.workspace = true
serde_json.workspace = true
uuid.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
tracing-appender.workspace = true
chrono.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [ ] **Step 6: 拉取依赖并编译**

Run: `cargo fetch`
Expected: 成功从 USTC 镜像下载（本机已实测 `iced 0.13.1` 可解析）。

Run: `cargo build --workspace`
Expected: `Finished dev profile`。如果出现 `E0658`/MSRV 错误，升级 rustc，不要降级依赖。

- [ ] **Step 7: 确认解析版本**

Run: `cargo tree -p iced --depth 0 && cargo tree -p rusqlite --depth 0 && cargo tree -p rfd --depth 0`
Expected 至少包含：

```text
iced v0.13.1
rusqlite v0.41.0
rfd v0.17.2
```

- [ ] **Step 8: 提交**

```bash
git add Cargo.toml Cargo.lock crates/*/Cargo.toml
git commit -m "chore(workspace): pin verified dependency set"
```

---

## Task F3: `launcher-core` 的 `AppPaths`

**Files:**
- Create: `crates/launcher-core/src/paths.rs`
- Create: `crates/launcher-core/src/error.rs`
- Modify: `crates/launcher-core/src/lib.rs`

- [ ] **Step 1: 写失败测试**

`crates/launcher-core/src/paths.rs` 先只放测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn xdg_data_home_wins() {
        let paths = AppPaths::from_values(Some(Path::new("/xdg")), Some(Path::new("/home/me")))
            .unwrap();
        assert_eq!(paths.data_root, PathBuf::from("/xdg/gamehub"));
        assert_eq!(paths.db_path, PathBuf::from("/xdg/gamehub/gamehub.db"));
    }

    #[test]
    fn home_fallback_is_used_when_xdg_is_empty() {
        let paths = AppPaths::from_values(Some(Path::new("")), Some(Path::new("/home/me")))
            .unwrap();
        assert_eq!(paths.data_root, PathBuf::from("/home/me/.local/share/gamehub"));
    }

    #[test]
    fn missing_everything_is_an_error() {
        assert_eq!(
            AppPaths::from_values(None, None),
            Err(AppPathsError::NoDataDir)
        );
    }

    #[test]
    fn derived_paths_are_stable() {
        let paths = AppPaths::for_root(PathBuf::from("/data/gamehub"));
        assert_eq!(paths.logs_dir, PathBuf::from("/data/gamehub/logs"));
        assert_eq!(
            paths.default_prefix_root,
            PathBuf::from("/data/gamehub/prefixes")
        );
        assert_eq!(
            paths.steam_compat_client,
            PathBuf::from("/data/gamehub/steam-compat-client")
        );
    }
}
```

`crates/launcher-core/src/lib.rs` 暂时：

```rust
pub mod error;
pub mod paths;

pub use error::AppPathsError;
pub use paths::AppPaths;
```

`crates/launcher-core/src/error.rs`：

```rust
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppPathsError {
    #[error("neither XDG_DATA_HOME nor HOME is set")]
    NoDataDir,
}
```

- [ ] **Step 2: 运行并确认失败**

Run: `cargo test -p launcher-core paths`
Expected: 编译失败，`AppPaths` 未定义。

- [ ] **Step 3: 实现 `AppPaths`**

在 `paths.rs` 的测试模块之前加：

```rust
use crate::AppPathsError;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub data_root: PathBuf,
    pub db_path: PathBuf,
    pub logs_dir: PathBuf,
    pub default_prefix_root: PathBuf,
    pub steam_compat_client: PathBuf,
}

impl AppPaths {
    pub fn for_root(data_root: PathBuf) -> Self {
        Self {
            db_path: data_root.join("gamehub.db"),
            logs_dir: data_root.join("logs"),
            default_prefix_root: data_root.join("prefixes"),
            steam_compat_client: data_root.join("steam-compat-client"),
            data_root,
        }
    }

    pub fn from_values(
        xdg_data_home: Option<&Path>,
        home: Option<&Path>,
    ) -> Result<Self, AppPathsError> {
        let xdg = xdg_data_home.filter(|p| !p.as_os_str().is_empty());
        let root = match (xdg, home) {
            (Some(xdg), _) => xdg.to_path_buf(),
            (None, Some(home)) => home.join(".local/share"),
            (None, None) => return Err(AppPathsError::NoDataDir),
        };
        Ok(Self::for_root(root.join("gamehub")))
    }

    pub fn discover() -> Result<Self, AppPathsError> {
        let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
        let home = std::env::var_os("HOME").map(PathBuf::from);
        Self::from_values(xdg.as_deref(), home.as_deref())
    }
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p launcher-core`
Expected: 4 passed。

- [ ] **Step 5: 提交**

```bash
git add crates/launcher-core
git commit -m "feat(launcher-core): add app data path resolution"
```

---

## Task F4: `truncate_utf8`

**Files:**
- Create: `crates/launcher-core/src/text.rs`
- Modify: `crates/launcher-core/src/lib.rs`

- [ ] **Step 1: 写失败测试**

`crates/launcher-core/src/text.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_unchanged() {
        assert_eq!(truncate_utf8("hello", 16), "hello");
    }

    #[test]
    fn ascii_is_cut_at_the_limit() {
        let out = truncate_utf8("abcdefghij", 4);
        assert_eq!(out, "abcd");
        assert!(out.len() <= 4);
    }

    #[test]
    fn multibyte_is_never_split() {
        let input = "错误信息：无法启动";
        let out = truncate_utf8(input, 10);
        assert!(out.len() <= 10);
        assert!(input.is_char_boundary(out.len()));
    }

    #[test]
    fn zero_limit_is_empty() {
        assert_eq!(truncate_utf8("abc", 0), "");
    }
}
```

`lib.rs` 增加：

```rust
pub mod text;
pub use text::truncate_utf8;
```

- [ ] **Step 2: 运行并确认失败**

Run: `cargo test -p launcher-core text`
Expected: 编译失败，`truncate_utf8` 未定义。

- [ ] **Step 3: 实现**

```rust
pub fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_owned();
    }
    if max_bytes == 0 {
        return String::new();
    }

    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_owned()
}
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p launcher-core`
Expected: 全部通过。

- [ ] **Step 5: 提交**

```bash
git add crates/launcher-core
git commit -m "feat(launcher-core): add UTF-8 safe stderr truncation"
```

---

## Task F5: 日志初始化

**Files:**
- Modify: `crates/ui/src/main.rs`

- [ ] **Step 1: 写实现**

```rust
use std::io::Write;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

fn main() {
    let paths = match launcher_core::AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("gamehub: cannot determine data directory: {error}");
            std::process::exit(1);
        }
    };

    let _log_guard = init_logging(&paths.logs_dir);
    tracing::info!(?paths, "gamehub starting");
    println!("gamehub data root: {}", paths.data_root.display());
}

fn init_logging(logs_dir: &Path) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ui=debug,launch=debug,storage=debug"));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true);

    match std::fs::create_dir_all(logs_dir) {
        Ok(()) => {
            let appender = tracing_appender::rolling::daily(logs_dir, "gamehub.log");
            let (file_writer, guard) = tracing_appender::non_blocking(appender);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer);
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .with(file_layer)
                .init();
            Some(guard)
        }
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "gamehub: cannot create log directory {}: {error}",
                logs_dir.display()
            );
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .init();
            None
        }
    }
}
```

注意变量名必须是 `_log_guard` 或以其他方式让 guard 活到 `main` 结束；提前 drop 会丢失最后一批日志。

- [ ] **Step 2: 运行**

Run: `cargo run -p ui`
Expected: 打印 `gamehub data root: /home/<user>/.local/share/gamehub`；stderr 有一行 `gamehub starting`；`~/.local/share/gamehub/logs/` 下出现 `gamehub.log.*` 文件。

- [ ] **Step 3: 验证日志文件**

Run: `find ~/.local/share/gamehub/logs -type f -maxdepth 1 | head`
Expected: 至少一个 `gamehub.log.YYYY-MM-DD` 文件。

- [ ] **Step 4: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): initialize tracing to stderr and daily log files"
```

---

## Task F6: Iced + tokio + 中文冒烟程序

**Files:**
- Modify: `crates/ui/src/main.rs`

- [ ] **Step 1: 把 `main` 改为运行 Iced 冒烟程序**

保留 F5 的 `init_logging`，把 `main` 的签名改为 `fn main() -> iced::Result`，并把最后的 `println!` 替换为：

```rust
    let result = run_smoke();
    tracing::info!("gamehub exiting");
    result
}

#[derive(Default)]
struct Smoke {
    clicks: u32,
    async_done: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Click,
    AsyncDone,
}

fn update(state: &mut Smoke, message: Message) -> iced::Task<Message> {
    match message {
        Message::Click => {
            state.clicks += 1;
            iced::Task::perform(
                async {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                },
                |_| Message::AsyncDone,
            )
        }
        Message::AsyncDone => {
            state.async_done = true;
            iced::Task::none()
        }
    }
}

fn view(state: &Smoke) -> iced::Element<'_, Message> {
    use iced::widget::{button, column, text};
    column![
        text("游戏启动器 中文渲染测试").size(24),
        text(format!("点击次数：{}", state.clicks)).size(18),
        text(if state.async_done {
            "tokio 异步任务：完成"
        } else {
            "tokio 异步任务：等待点击"
        }),
        button("点击测试").on_press(Message::Click),
    ]
    .spacing(12)
    .padding(24)
    .into()
}

fn run_smoke() -> iced::Result {
    iced::application("gamehub smoke", update, view)
        .theme(|_| iced::Theme::Dark)
        .window(iced::window::Settings {
            size: iced::Size::new(720.0, 480.0),
            ..Default::default()
        })
        .run_with(|| (Smoke::default(), iced::Task::none()))
}
```

`run_smoke` 必须有 `'static` 的 update/view；上面的自由函数满足要求。

- [ ] **Step 2: 编译**

Run: `cargo build -p ui`
Expected: 成功。若报 `Task` 不存在，检查 iced 版本是否为 0.13.x（0.13 已把 `Command` 改名为 `Task`）。

- [ ] **Step 3: 人工冒烟**

Run: `cargo run -p ui`

逐项确认：

1. 窗口打开，标题为 `gamehub smoke`。
2. 三行中文都能正常显示，没有 `□□□` 方块。
3. 点击“点击测试”后，“点击次数”加 1。
4. 约 100ms 后第三行变为“tokio 异步任务：完成”。若点击后应用无响应或 stderr 出现 `there is no reactor running`，说明 `iced` 的 `tokio` feature 没启用。

中文出现方块时的最小修复（记录到 CHANGELOG）：用 `fc-list :lang=zh family | head` 选一个已安装字体，在 `iced::application(...)` 后加：

```rust
.default_font(iced::Font::with_name("Noto Sans CJK SC"))
```

把实际使用的字体名写入 `README.md` 的“字体”一节。

- [ ] **Step 4: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add iced tokio smoke application with CJK render check"
```

---

## Task F7: P0 收尾与基线检查

**Files:**
- Create: `CHANGELOG.md`
- Create: `README.md`

- [ ] **Step 1: 写 `CHANGELOG.md`**

```markdown
# Changelog

## P0 — 基线与冒烟

- 转换为 7-crate workspace，锁定依赖并提交 `Cargo.lock`。
- 增加 `AppPaths`（XDG 数据目录）与 UTF-8 安全截断。
- 增加 tracing 双输出（stderr + 每日日志文件）。
- 增加 Iced 0.13 + tokio 执行器 + 中文渲染冒烟程序。

验证：
- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 人工运行 `cargo run -p ui`，确认窗口/中文/异步任务。
```

- [ ] **Step 2: 写最小 `README.md`**

````markdown
# gameHub

Linux 离线 Windows 游戏启动器（Rust + Iced）。

## 外部依赖

- 运行时：Wine 或用户自备的 Proton（含 GE-Proton）目录。
- DXVK：由用户提供解压后的发布目录（应用不下载）。
- 文件对话框：rfd 0.17 使用 xdg-desktop-portal；桌面环境需提供 portal 服务。
- 显示：X11 路径使用 `xrandr`；Wayland 可选 `gamescope`。

## 构建

```bash
cargo build --release
./target/release/gamehub
```

应用数据位于 `$XDG_DATA_HOME/gamehub` 或 `~/.local/share/gamehub`。

详细规格见 `docs/specs/00-overview.md`。
````

外层使用四个反引号，避免内层 bash 代码块提前结束。

- [ ] **Step 3: 全量验证**

Run:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Expected: 全部成功。`fmt --check` 若失败，先运行 `cargo fmt --all` 再重跑。

- [ ] **Step 4: 提交**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: record P0 baseline and external dependencies"
```

---

## P0 验收

- [x] `cargo build --workspace` 通过。
- [x] `cargo test --workspace` 通过。
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 通过。
- [x] `cargo run -p ui` 窗口显示中文，点击后 tokio 异步任务完成。
- [x] `~/.local/share/gamehub/logs/` 有日志文件。
- [x] `Cargo.lock` 已提交，解析到 `iced 0.13.1`、`rusqlite 0.41.0`、`rfd 0.17.2`。

## 实施记录

- 2026-10-26 完成。rustc/cargo 1.99.1；8 tests passed。
- 中文渲染：系统存在 Noto Sans CJK SC 与 WenQuanYi Zen Hei，fontconfig 对
  `U+6E38` 解析到 WenQuanYi Zen Hei，未设置 `default_font`。
  窗口进程运行 6 秒无 panic；GNOME 会话禁止自动截屏，目视确认待用户执行。
- 偏差：仓库已有初始 commit，`src/main.rs` 为已跟踪文件；仍按 spec 使用
  `mv` 而非 `git mv`。
