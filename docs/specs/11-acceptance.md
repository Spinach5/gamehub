# 交付验收与手工测试 Implementation Plan

> 历史验收计划与记录（P10，2026-10-27，验收对象 `bf0ecdd`）。Task A1–A10
> 与勾选状态保留原样；v0.2.0–v0.11.0 的增量验收见文末
> 「当前增量验收」。

> **For agentic workers:** 本文件不产出新功能，只定义交付门槛与证据记录方式。所有自动命令必须真实执行并保留输出；无法执行的项必须写明原因，不得标记为通过。

**Goal:** 在断网 Linux（X11/Wayland）环境验证 v2 规格的全部交付项，并记录可复核的证据。

**Architecture:** 自动检查覆盖编译/lint/单测；真实 Wine 集成测试标记 `#[ignore]`，手工执行；UI 端到端使用 Wine 自带 `notepad.exe` 验证，不依赖真实游戏。

**Tech Stack:** 本机 rustc 1.99.1、Wine 10.0、Wayland + XWayland、Iced 0.13.1。

**前置条件：** P0–P9 全部完成；`CHANGELOG.md` 已更新。

---

## Task A1: 自动质量门槛

- [x] **Step 1: 格式检查**

```bash
cargo fmt --all --check
```

Expected: 无输出、退出码 0。失败则运行 `cargo fmt --all` 后重跑。

- [x] **Step 2: lint**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: `Finished`，无 warning。任何 warning 都必须修掉，不要用 `#[allow]` 跳过（除非是外部 API 的已知误报，并在 CHANGELOG 记录原因）。

- [x] **Step 3: 全量单测**

```bash
cargo test --workspace
```

Expected: 全部通过；真实 Wine 测试显示 `ignored`。

- [x] **Step 4: debug 与 release 构建**

```bash
cargo build --workspace
cargo build --release --workspace
```

Expected: 均成功，产物为 `target/release/gamehub`。

---

## Task A2: 真实 Wine 集成测试

- [x] **Step 1: 运行被忽略的 prefix 测试**

```bash
cargo test -p prefix initializes_with_real_system_wine -- --ignored --nocapture
```

Expected: 通过；临时目录中出现 `system.reg`。首次 `wineboot` 可能需要 1–2 分钟。

- [x] **Step 2: 记录环境**

```bash
wine --version
rustc --version
echo "$XDG_SESSION_TYPE"
xrandr --query | head -3
```

把输出写入下方的证据表。

---

## Task A3: UI 端到端（Wine 路径）

本机没有真实游戏，使用 Wine prefix 内的 `notepad.exe` 验证启动链路。

- [x] **Step 1: 启动应用**

```bash
cargo run --release -p ui
```

Expected: 窗口出现，中文正常，无方块；日志文件写入 `~/.local/share/gamehub/logs/`。

- [x] **Step 2: 探测 Runner**

Runner 管理 → “重新探测系统 Wine”。
Expected: 出现 `System Wine`，版本 `wine-10.0`，标记“已验证”。

- [x] **Step 3: 创建测试游戏并保存**

1. 新建游戏 → 选择一个存在的任意文件（例如 `touch /tmp/gamehub-test.exe` 后选择它）。
2. 名称填 `Notepad Test`，架构 win64，保存。
3. 进入详情 → Runner 选择系统 Wine → “创建 / 初始化”。
4. 等待状态变为“已初始化”。

- [x] **Step 4: 切换到 prefix 内的 notepad.exe**

prefix 默认路径：

```text
~/.local/share/gamehub/prefixes/<game-id>/
```

Wine 的 `notepad.exe` 位于：

```text
~/.local/share/gamehub/prefixes/<game-id>/drive_c/windows/system32/notepad.exe
```

在详情页重新选择该 exe，工作目录设为同一 `system32` 目录，保存。
Expected: 保存成功（文件必须存在）。若该文件不存在，改用同一目录下的 `winecfg.exe` 或任意 Wine 内置 exe，并在证据表记录实际文件名。

- [x] **Step 5: 显示配置**

- 本机会话为 Wayland：显示器字段是文本输入（gamescope `-O` 用），不出现 X11 下拉；仅当 `XDG_SESSION_TYPE=x11` 时才是 `eDP-1` 等输出下拉。
- gamescope 本机未安装：选择“关闭”。此时分辨率/刷新率输入应被说明文字替代（非 gamescope 路径不修改分辨率）。

- [x] **Step 6: 环境变量与 DLL Override**

- 添加 `DXVK_HUD=fps`。
- 添加 DLL Override：`d3d11` → `原生+内置 (n,b)`。
- 尝试添加环境变量 `WINEDLLOVERRIDES=x=n`，保存应失败并提示使用结构化 DLL override。
- 删除该非法行，保存成功。

- [x] **Step 7: 启动**

点击“启动”。
Expected:

- 出现 Wine notepad 窗口（通过 XWayland）。
- UI 显示“运行中”。
- 关闭 notepad 后 UI 恢复；`最近游玩` 更新为当前时间。
- 退出码为 0，不显示异常错误。

- [x] **Step 8: 重启持久化**

关闭并重启应用。
Expected: Runner、游戏、Prefix、显示配置、环境变量、DLL override 全部保留；`最近游玩` 保留。

- [x] **Step 9: 删除保护**

1. 尝试删除正在使用的 Runner → 被拒绝。
2. 尝试删除默认 Runner → 被拒绝（先设置默认再删）。
3. 为两个游戏选择同一个 Prefix 路径并保存，尝试删除其中一个 → 被拒绝（共享）。
4. 运行中的游戏 → 删除游戏/删除 Prefix 均被拒绝。
5. 退出游戏后删除游戏 → 成功；默认 Prefix 目录可另行删除。

---

## Task A4: Proton 路径（有目录时必做）

本机提供 GE-Proton11-7，本节按实机执行；若没有 GE-Proton/Proton 目录，
本节标记“未执行：无 Proton 目录”，但仍需完成伪造脚本的自动测试。

- [x] **Step 1: 导入 Proton（如有）**

Runner 管理 → 导入 Proton 目录 → 选择包含 `proton` 脚本的 GE-Proton 目录。
Expected: 列表出现 runner，版本来自 `version` 文件或目录名，`verified=true`。

- [x] **Step 2: 创建 Proton prefix（如有）**

为测试游戏选择 Proton runner → 创建 Prefix。
Expected:

- `STEAM_COMPAT_DATA_PATH` 目录下出现 `pfx/`。
- `pfx/system.reg` 存在。
- UI 状态“已初始化”。

- [x] **Step 3: 启动 notepad（如有）**

exe 选择 `.../pfx/drive_c/windows/system32/notepad.exe`，启动。
Expected: notepad 窗口出现；关闭后 `最近游玩` 更新。

- [x] **Step 4: 记录限制**

```markdown
Proton 端到端：已执行（版本：GE-Proton11-7，版本串 1789520217 GE-Proton11-7）
验证：导入后 verified=true；创建 Prefix 生成 pfx/system.reg；
      从 pfx 内 notepad.exe 启动并关闭，最近游玩更新。
说明：测试用 STEAM_COMPAT_DATA_PATH 位于隔离目录，验收后删除测试游戏。
```

---

## Task A5: 离线验证

本机当前**未安装 `strace`**。验收时下载 6.13 的 `.deb` 解压到 `/tmp` 直接
执行，未改动系统包；若完全无法获得 strace，记录“未执行”，可用 `ss -tunp`
在操作过程中抽查没有已建立的 TCP/UDP 连接，但不得把它等同于 strace 证据。

- [x] **Step 1: 运行 strace**

```bash
strace -f -e trace=connect,sendto -o /tmp/gamehub-net.log ./target/release/gamehub
```

在应用中完成：探测 Runner、导入/编辑游戏、创建 Prefix、启动并关闭 notepad、保存设置。然后退出。

- [x] **Step 2: 检查 IPv4/IPv6**

```bash
grep -E 'AF_INET|AF_INET6' /tmp/gamehub-net.log && echo 'NETWORK DETECTED' || echo 'offline ok'
```

Expected: `offline ok`。

只允许：

- `AF_UNIX`：X11、D-Bus（rfd portal）、Wayland。
- `AF_NETLINK`：内核接口枚举；`strace -e trace=network` 会显示，不能据此判定联网。

- [x] **Step 3: 抽查子进程**

```bash
grep -E 'wine|proton|xrandr|gamescope|rfd' /tmp/gamehub-net.log | head -20
```

Expected: 只有本地进程与 unix socket，没有 `AF_INET`。

---

## Task A6: Release 产物与依赖

- [x] **Step 1: 检查禁用技术栈**

```bash
ldd target/release/gamehub | grep -Ei 'gtk|qt|webkit|electron|node' && echo 'FORBIDDEN DEP' || echo 'no web/gtk/qt deps'
```

Expected: `no web/gtk/qt deps`。

`rfd 0.17` 默认使用 `xdg-portal`，不会链接 GTK；若桌面没有 portal，文件对话框会失败并在 UI 显示错误，这是运行环境限制，不是链接依赖问题。

- [x] **Step 2: 检查动态库**

```bash
ldd target/release/gamehub | head -30
```

记录实际依赖（预期包含 wayland-client、xkbcommon、libgcc/libm 等；wgpu 可能在运行时加载 Vulkan，若失败自动回退 tiny-skia）。

- [x] **Step 3: 检查单二进制**

```bash
ls -lh target/release/gamehub
```

Expected: 单个可执行文件；应用不要求额外资源目录（不打包字体/DXVK）。

---

## Task A7: 字体与窗口

- [x] **Step 1: 中文渲染**

启动应用，确认所有中文标签、错误、按钮无 `□□□` 方块。

若出现方块：

1. 运行 `fc-list :lang=zh family | head`。
2. 在 `ui::run()` 的 `iced::application(...)` 后加 `.default_font(iced::Font::with_name("<实际字体名>"))`。
3. 重新验证并写入 `README.md` 的“字体”一节。

- [x] **Step 2: 缩放/最小尺寸**

把窗口缩到最小尺寸（900×600），确认布局不崩溃、可滚动；在高 DPI 下确认文字可读。

---

## Task A8: 验收清单映射（对照 v1 第 12 节）

| v1 项 | v2 验证方式 | 结果 |
|---|---|---|
| 1. 导入本地 exe、填名称/工作目录、保存 | A3 Step 3、Step 8 | 通过 |
| 2. 探测系统 Wine 或导入 Proton，`verified=true` | A3 Step 2；A4 Step 1 | 通过 |
| 3. 创建 prefix（选 arch，可选 DXVK），UI 不卡顿 | A3 Step 3；A3 Step 6 | 通过（DXVK 安装沿用 P8 验证） |
| 4. X11 输出/分辨率；Wayland gamescope；保存 | A3 Step 5；gamescope 参数单测 | 部分通过：Wayland 与参数单测通过；X11 输出、真实 gamescope 未执行（无 X11 会话、无 gamescope） |
| 5. 环境变量与结构化 DLL override；手设 WINEDLLOVERRIDES 拒绝 | A3 Step 6 | 通过 |
| 6. 启动、子进程、错误展示、`last_played_at` | A3 Step 7 | 通过 |
| 7. 重启后全部配置恢复 | A3 Step 8 | 通过 |
| 8. 全程无网络请求 | A5 | 通过（0 条 AF_INET/AF_INET6） |
| 9. clippy/test 通过 | A1 | 通过 |
| 10. release 单文件、无 WebKit/Qt/GTK | A6 | 通过 |

**与 v1 的差异：** v1 第 4 项暗示 X11 非 gamescope 时分辨率也生效；v2 明确不生效（只设置 `__GL_SYNC_DISPLAY_DEVICE`），分辨率/刷新仅在 gamescope 路径使用。验收按 v2 执行。

---

## Task A9: gamescope 验证（有安装时）

本机未安装 gamescope，以下只能做参数级验证；安装后必须补做。

- [x] **Step 1: 参数级验证**

```bash
cargo test -p display gamescope
```

Expected: 真值表全部通过。

- [ ] **Step 2: 安装后手工验证（如果后续安装）**

1. `which gamescope` 有输出，重启应用（`OnceLock` 缓存）。
2. Wayland 会话，gamescope = 自动/强制，分辨率 1280×720，全屏。
3. 启动 notepad。
4. 进程命令行应类似：

```text
gamescope -W 1280 -H 720 -f --backend wayland -- <wine> <exe> ...
```

5. 若 gamescope 报 `--backend`/`-O` 不支持，记录实际版本与错误，在 `CHANGELOG` 记录并做最小适配（去掉不支持的标志），不要静默忽略失败。

---

## Task A10: 证据记录与交付物

- [x] **Step 1: 填写证据表**

```markdown
# 交付验收记录

日期：2026-__-__
提交：<git commit>
rustc：1.99.1
Wine：wine-10.0
会话：Wayland + XWayland
gamescope：未安装
Proton：无目录

## 自动检查
- cargo fmt --all --check：通过
- cargo clippy --workspace --all-targets -- -D warnings：通过
- cargo test --workspace：通过（__ passed，__ ignored）
- cargo build --release --workspace：通过

## 手工检查
- A2 真实 wineboot：通过/失败（耗时 __）
- A3 Wine 端到端：通过/失败（notepad 启动、退出码、last_played）
- A4 Proton 端到端：未执行（无目录）
- A5 离线：offline ok
- A6 ldd：无 gtk/qt/webkit
- A7 中文渲染：通过（字体：__）

## 偏差
- 无 / 列出
```

- [x] **Step 2: 更新 README**

`README.md` 必须包含：

- 构建命令与 release 产物路径。
- 外部依赖：Wine/Proton、DXVK 由用户提供；gamescope 可选。
- rfd 0.17 需要 xdg-desktop-portal；无 portal 时文件对话框不可用。
- X11 分辨率限制：非 gamescope 路径不修改显示模式。
- 数据目录与日志位置。
- 本机验收限制：gamescope/Proton 未安装时的手工验证缺口。

- [x] **Step 3: 最终提交**

```bash
git add README.md CHANGELOG.md docs
git commit -m "docs: record v2 acceptance evidence and limitations"
```

---

## 交付门槛（全部满足才算完成）

- [x] A1 全部自动检查通过。
- [x] A2 真实 Wine prefix 初始化通过（本机 Wine 10.0）。
- [x] A3 Wine 端到端通过；配置重启保留；删除保护生效。
- [x] A5 离线检查无 `AF_INET/AF_INET6`。
- [x] A6 无 GTK/Qt/WebKit 链接。
- [x] A7 中文无方块。
- [x] A8 表格每项有“通过/未执行+原因”。
- [x] A10 证据表与 README 已提交。
- [x] 所有“未执行”项都有替代验证或明确环境原因，不得空白。

---

## 实施记录（2026-10-27）

- 验收提交：`bf0ecdd`；详细证据见 [`docs/acceptance.md`](../acceptance.md)。
- 环境：rustc 1.99.1、Wine 10.0、Wayland + XWayland、gamescope 未安装、
  GE-Proton11-7 可用。
- 自动检查：fmt/clippy 通过；`cargo test --workspace` 156 passed、1 ignored；
  debug/release 构建通过（release 14 MB）；`cargo test -p display gamescope`
  10 passed；真实 wineboot 测试 10.22 s 通过。
- 手工检查：A3 Wine 端到端、A4 Proton 端到端、删除保护、重启持久化通过；
  A5 的 `strace` 日志 0 条 `AF_INET/AF_INET6`；A6 无 GTK/Qt/WebKit；A7 中文
  无方块、900×600 正常。
- 偏差与未执行：
  - 系统未安装 strace：下载 6.13 `.deb` 解压到 `/tmp` 执行，未改动系统包；
    另加 `execve` 复跑一次以识别 wine/notepad 子进程，两次均为 0 条网络连接。
  - gamescope 未安装：A9 Step 2 未执行，仅有参数级单测。
  - 本机无 X11 会话：X11 输出下拉与非 gamescope 分辨率未手工验证。
  - 自动化经 XWayland + XTEST/`import -window` 完成；会话按 Wayland 识别。
  - 共享 Prefix 与同 Prefix 并发的第二游戏由 DB 预置，验证后删除。
  - 高 DPI 未验证（缩放因子 1）。
  - rfd 在隔离环境回退 zenity（无 session bus）；正常桌面使用 portal。
- 游戏删除只删 DB 行，不删除 Prefix 目录，符合 A3 Step 9 的“可另行删除”。

---

## 当前增量验收（2026-10-31）

基线：`v0.10.0`（`bdd55cc`）；本次发布的增量为侧边栏版本链接。

### 自动检查（本次实跑）

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：350 passed，0 failed，2 ignored
  （真实 Wine prefix 测试仍为 ignored）。

### v0.2.0–v0.11.0 增量功能

| 版本 | 能力 | 验收方式 |
|---|---|---|
| v0.2.0 | 卡片启动/设置/日志、启动中/游玩状态、启动日志页、Esync/Fsync/MangoHud/DXVK-NVAPI 设置、`LaunchEvent` 事件流 | 应用层事件顺序测试；伪造可执行文件验证 MangoHud/NVAPI；XWayland 冒烟 |
| v0.3.x | 结束游戏（进程组 + `wineserver -k`）、显示器自动扫描、Wayland primary 切换 | 进程组/prefix 集成测试；Mutter DisplayConfig 真实切换；HDMI-1/eDP-1 |
| v0.4.x | 浅色/深色主题、卡片与详情页改版、自定义封面、schema v2 | 主题令牌与封面生命周期测试；像素级控件内部取色 |
| v0.5.0 | 中英文 i18n、`ui_language` 设置、错误本地化 | 目录 key 一致性守卫；英文界面源码无中文字面量 |
| v0.6.0 | 游玩时长、启动时写 `last_played_at`、会话保活、`GameDraft` 迁入 application、schema v3 | `add_playtime`/会话保活测试；真实 v2→v3 迁移 |
| v0.7.0 | 系统 winetricks 组件安装、实时输出、取消、`winetricks.log` 读取 | 组件服务测试；真实 winetricks 20250102 安装 `fontsmooth-rgb` |
| v0.8.0 | 已安装字体切换、字重吸附、`ui_font` 持久化 | 字体解析/切换/保存竞态测试；浅色/深色实机走查 |
| v0.9.0 | `Platform::{Windows,Linux}`、平台选择弹窗、原生启动分支、schema v4、停止原生进程 | 领域/存储/编辑器/启动计划/UI 测试；Linux 原生可执行文件启动 |
| v0.10.0 | DLL Override 目录扫描、去重勾选、批量追加、每游戏扫描目录 | 扫描/设置/UI 状态测试；10,000 个 DLL 与 15 万文件目录实机走查；空目录/不存在/无权限；浅色/深色字迹检查 |
| v0.11.0 | 侧边栏版本号、链接样式与悬停提示、点击打开项目 GitHub | 浅色/深色实机截图与悬停提示；stub `xdg-open` 验证传入 URL |

### 当前限制

- 真实 gamescope 仍未安装，gamescope 启动路径只做参数级测试。
- 原生 Linux 游戏只验证了测试可执行文件，未覆盖真实商业游戏。
- 平台支持随 v0.9.0 发布。
- 扫描完成过快（15 万文件 <0.4s），未能实机点击「取消扫描」；取消语义由单测覆盖。
- 「更改扫描目录」的 Wayland portal 对话框无法用 X11 截图捕获，未截图验证。
- 版本链接用记录参数的 stub `xdg-open` 验证，未在真实桌面会话中拉起浏览器；`xdg-open` 缺失时的错误横幅未实机触发。
- 历史验收中的 X11/高 DPI 缺口仍然存在。
