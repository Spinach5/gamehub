# gameHub 实施方案 v2（入口）

> 原 v1 方案已按实际仓库、工具链与 API 拆分为可执行 specs，并修正了若干会导致实现失败或语义错误的点。原文档存档于 [`docs/specs/archive/plan-v1.md`](specs/archive/plan-v1.md)。

**产品：** Linux（X11/Wayland）离线 Windows 游戏启动器。
**技术栈：** Rust edition 2024、Iced 0.13.1、tokio、rusqlite(bundled)、rfd 0.17、serde、thiserror。
**实施方式：** 按阶段顺序执行；每阶段通过编译、lint、测试后才能进入下一阶段。

---

## 1. 当前仓库实际状态（2026-10-26）

| 项 | 状态 |
|---|---|
| 仓库 | 单 package `gameHub`，只有 `src/main.rs` 的 Hello World，尚无 commit |
| rustc/cargo | 1.99.1 / 1.99.1 |
| 会话 | Wayland（`XDG_SESSION_TYPE=wayland`），有 XWayland `DISPLAY=:0` |
| Wine | `/usr/bin/wine`，`wine-10.0` |
| gamescope | 未安装 |
| Proton | 未安装（需用户自备目录） |
| 中文 | 系统字体可用，Iced 0.13 走 cosmic-text + 系统字体回退 |
| rfd | 0.17.2 默认 `xdg-portal + wayland`，不依赖 GTK |
| 依赖验证 | `iced 0.13.1 + tokio + rusqlite 0.41 + rfd 0.17` 已本机 `cargo check` 通过 |

因此 P0 的第一件事是：把单 package 迁移为 `crates/*` workspace，而不是直接在现有根目录堆业务代码。

---

## 2. 规格文档索引

| 阶段 | 文档 | 内容 |
|---|---|---|
| 总览 | [00-overview.md](specs/00-overview.md) | 环境、仓库结构、依赖锁定、全局约定、v1→v2 修正表 |
| P0 | [01-foundation.md](specs/01-foundation.md) | workspace 迁移、依赖、`AppPaths`、截断、日志、Iced+tokio 中文冒烟 |
| P1 | [02-core-types.md](specs/02-core-types.md) | 领域类型、serde 契约、枚举映射 |
| P2 | [03-storage.md](specs/03-storage.md) | SQLite v1 schema、迁移、CRUD、引用计数、损坏数据错误 |
| P3 | [04-runner-wine.md](specs/04-runner-wine.md) | Wine 探测、Proton 校验、`WineContext`、超时与 spawn |
| P4 | [05-prefix-dxvk.md](specs/05-prefix-dxvk.md) | Wine/Proton prefix 布局、状态/arch、wineboot、DXVK |
| P5 | [06-display.md](specs/06-display.md) | 会话检测、xrandr 解析、gamescope 参数 |
| P6 | [07-launch.md](specs/07-launch.md) | runner 解析、环境合成、DLL override、启动计划与 spawn |
| P7 | [08-ui-shell.md](specs/08-ui-shell.md) | UI lib/bin、启动加载、Library、草稿校验、保存 |
| P8 | [09-ui-runners-prefix-settings.md](specs/09-ui-runners-prefix-settings.md) | Runner 管理、Prefix/DXVK、设置、删除保护 |
| P9 | [10-ui-config-launch.md](specs/10-ui-config-launch.md) | 显示/环境/DLL/参数编辑、启动、运行状态、游戏删除 |
| 验收 | [11-acceptance.md](specs/11-acceptance.md) | 自动检查、真实 Wine、离线、release、证据表 |

---

## 3. 必须知道的修正（否则会做错）

完整对照见 [00-overview.md 第 6 节](specs/00-overview.md)。最重要的 8 条：

1. **Proton 的 `system.reg` 与 `drive_c` 在 `STEAM_COMPAT_DATA_PATH/pfx/` 下**，不是 prefix 根目录。所有状态、初始化、DXVK 路径都走 `prefix::windows_root()`。
2. **Proton prefix 永远是 win64**；`Game.arch = win32` 只表示游戏，prefix 用 `plan_for_game()` 固定 win64。DXVK 按 prefix arch 选择 x64/x32。
3. **Iced 0.13 用 `Task`，不是 `Command`**；`run_with` 返回 `(State, Task)`；必须启用 iced 的 `tokio` feature，否则 `tokio::process` 在运行期 panic。
4. **`OverrideMode::Disabled` 映射为 `disabled`**，不是 `n`。
5. **X11 非 gamescope 路径不改分辨率**，只注入 `__GL_SYNC_DISPLAY_DEVICE`；分辨率/刷新仅在 gamescope 生效时可用，UI 必须说明。
6. **`build_env` 的环境继承通过 `LaunchRequest.inherited_env` 注入**；spawn 时先 `env_clear()`，避免重复/不确定。
7. **启动只发一条终态消息** `LaunchFinished`；用 `running: HashMap<game_id, prefix>` 表示启动中/运行中，避免引入事件通道。
8. **rfd 0.17 没有 GTK/zenity 回退**；它依赖 xdg-desktop-portal。README 与错误提示必须说明这一点。

其他修正：workspace 迁移、`AppPaths`、storage 引用计数、4KB stderr、wineboot 超时、删除保护、离线 strace 只看 `AF_INET` 等，均在 00 总览表中。

---

## 4. 阶段完成定义（DoD）

每阶段结束必须真实执行：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

并满足：

- 该阶段 spec 的验收条目全部勾选。
- `CHANGELOG.md` 记录：阶段、完成内容、与 spec 的偏差、验证命令与结果。
- 没有新增顶层 crate，没有违反依赖方向（见 00 第 3 节）。
- 系统命令错误带退出码与截断 stderr；库层无 `unwrap()` 系统结果。
- 新增 UI 行为有手工验证记录；纯逻辑有单测。

---

## 5. 实施顺序与提交

1. 从 P0 开始，按 [01-foundation.md](specs/01-foundation.md) 执行；P0 结束先提交并确认 `Cargo.lock`。
2. 每个 spec 内按任务 ID 顺序执行（F1…、C1…、S1…、R1…、PF1…、D1…、L1…、U1…）。
3. 每个任务至少一次 commit；提交信息使用 `feat(<crate>)` / `test(<crate>)` / `chore(workspace)`。
4. 发现 API 与 spec 不符时做最小适配，并记录到 CHANGELOG 与对应 spec 的“实施记录”；不要静默改架构。
5. 需要新增依赖、改 crate 边界、引入联网功能时，先停下来确认。

推荐的执行技能：

- 逐任务实施与审查：`superpowers:subagent-driven-development`
- 批量执行并在检查点暂停：`superpowers:executing-plans`

---

## 6. 交付物

- `Cargo.toml`（workspace）、`Cargo.lock`、`crates/*` 源码与测试
- `README.md`：构建、外部依赖、rfd portal、X11 分辨率限制、数据目录
- `CHANGELOG.md`：逐阶段记录
- `docs/specs/*`：本目录
- `docs/specs/11-acceptance.md` 的“证据表”填写完成
- `target/release/gamehub`：单可执行文件，无 GTK/Qt/WebKit 链接

---

## 7. 已知环境限制（验收时必须如实记录）

| 项 | 本机状态 | 替代验证 |
|---|---|---|
| gamescope | 未安装 | 参数真值表单测；安装后补手工测试 |
| Proton | 无目录 | 伪造 proton 脚本测试 `pfx` 布局与 `run reg`；有目录后补端到端 |
| 真实游戏 | 无 | 使用 Wine prefix 内 `notepad.exe` 验证启动/退出/`last_played_at` |
| X11 显示模式修改 | 不实现 | 仅 `__GL_SYNC_DISPLAY_DEVICE`；分辨率只在 gamescope 下生效 |
| UI 自动化 | Iced 无成熟 widget 测试 | 纯逻辑单测 + 11-acceptance 手工清单 |

---

## 8. 快速开始

```bash
# P0 完成后
cargo run -p ui

# 全量检查
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 真实 Wine 集成测试（手工）
cargo test -p prefix initializes_with_real_system_wine -- --ignored --nocapture

# release
cargo build --release --workspace
./target/release/gamehub
```
