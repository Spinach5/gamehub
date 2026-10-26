# 交付验收记录（v2）

日期：2026-10-27
提交：`bf0ecdd`（`feat(ui): finish configuration editors and launch flow`）
验收对象：`docs/specs/11-acceptance.md` A1–A10

## 环境

| 项 | 值 |
|---|---|
| rustc | 1.99.1 (58cf8f9c3 2026-10-13) |
| cargo | 1.99.1 (5f94df478 2026-08-27) |
| Wine | wine-10.0 (Debian 10.0~repack-6) |
| 会话 | Wayland；自动化时 UI 以 `WINIT_UNIX_BACKEND=x11` 运行在 XWayland，`XDG_SESSION_TYPE` 仍为 `wayland` |
| 显示 | XWayland 根 3840×1080，单窗口 958×600，最小尺寸 900×600，缩放因子 1 |
| gamescope | 未安装（`command -v gamescope` 无输出） |
| Proton | GE-Proton11-7，`~/.local/share/Steam/compatibilitytools.d/GE-Proton11-7/proton`，版本串 `1789520217 GE-Proton11-7` |
| 中文字体 | Noto Sans CJK SC（`/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`） |
| strace | 系统未安装；验收时下载 6.13 的 `.deb` 解压到 `/tmp` 直接执行，未安装进系统 |
| 数据隔离 | `XDG_DATA_HOME=/tmp/gh-accept/data`，日志 `data/gamehub/logs/gamehub.log.2026-10-26` |

## 自动检查

- `cargo fmt --all --check`：通过（无输出，退出码 0）。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（`Finished`，无 warning）。
- `cargo test --workspace`：通过，156 passed，0 failed，1 ignored；
  doc-tests 均为 0。
- `cargo test -p ui`：26 passed（其中包含字体/窗口之外的 UI 状态与守卫测试）。
- `cargo build --workspace`、`cargo build --release --workspace`：均通过，
  产物 `target/release/gamehub` 为 14 MB。
- `cargo test -p display gamescope`：10 passed（gamescope 真值表）。
- `cargo test -p prefix initializes_with_real_system_wine -- --ignored --nocapture`：
  通过，耗时 10.22 s，临时 Prefix 生成 `system.reg`。

## 手工检查

### A2 真实 Wine

- Wine 10.0 下 `wineboot -u` 初始化成功，`system.reg` 存在。
- 应用内初始化同一 Prefix 约 12 秒（无 strace 时）；strace 下约 40 秒，
  UI 保持响应并最终刷新为“已初始化”。

### A3 Wine 端到端

- 探测得到 `System Wine / wine-10.0 (Debian 10.0~repack-6) / 已验证`，
  并设为默认 Runner。
- 通过文件对话框导入 exe、填写名称/工作目录、选择 win64 并保存；重启后
  游戏与最近游玩保留。
- 在详情页创建/初始化 Prefix，状态从“未创建”刷新为“已初始化”，
  `prefixes/<game-id>/system.reg` 存在。
- 将可执行文件切换到 Prefix 内
  `drive_c/windows/system32/notepad.exe`（验收用符号链接，MD5 与 Prefix 内
  文件一致），工作目录为同一 `system32`。
- 显示：Wayland 分支显示显示器文本输入；gamescope 未安装时选择“关闭”，
  分辨率/刷新率输入被说明文字替代，保存成功。
- 环境变量 `DXVK_HUD=fps` 与结构化 DLL Override `d3d11=原生+内置 (n,b)`
  保存成功并在重启后保留；手工添加 `WINEDLLOVERRIDES=x=n` 保存被拒绝，
  提示“WINEDLLOVERRIDES 必须通过结构化 DLL override 配置”，删除非法行后
  保存成功。
- 启动 notepad 后详情页显示“运行中”，启动/删除游戏/删除 Prefix 被禁用；
  子进程环境包含全局与游戏环境变量；关闭 notepad 后状态恢复、
  `最近游玩` 更新为当前时间，退出码 0 且无错误提示。
- 删除保护：
  - 正在使用（被游戏引用）的 Runner 删除被拒绝；
  - 默认 Runner 删除被拒绝，更换默认后可删除；
  - 两个游戏共享同一 Prefix 时删除被拒绝（“该 Prefix 被多个游戏共享”）；
  - 运行中删除游戏/Prefix 被拒绝；
  - 退出后删除游戏成功；游戏删除不自动删除 Prefix 目录（按规格另行删除）。

### A4 Proton 端到端

- 导入 GE-Proton11-7，列表显示版本与“已验证”，重启后保留。
- 为测试游戏选择 Proton Runner 创建 Prefix，生成
  `/tmp/gh-accept/proton-prefix/pfx/system.reg`（使用
  `STEAM_COMPAT_DATA_PATH` 指向该目录，Proton 写入 `pfx/`）。
- 从该 Prefix 的 `system32/notepad.exe` 启动，notepad 窗口出现；关闭后
  “最近游玩”更新。验收后删除测试游戏，Proton Runner 保留。

### A5 离线

- 按规格运行 `strace -f -e trace=connect,sendto -o net.log`，完成探测
  Runner、编辑游戏、创建 Prefix、启动/关闭 notepad、保存设置后退出。
- `net.log` 共 462 行，`grep -E 'AF_INET|AF_INET6'` 结果为 0，输出
  `offline ok`。
- 只出现 `AF_UNIX`：X11（`@/tmp/.X11-unix/X0`）、系统 D-Bus
  （`/run/dbus/system_bus_socket`）、nscd、CUPS 以及一次
  `wayland-0`（ENOENT，XWayland 模式下未设置 `WAYLAND_DISPLAY`）。
- 另以 `-e trace=connect,sendto,execve` 复跑一次以识别子进程：跟踪到
  `/usr/bin/wine`、`wine64`、`wineserver`、`notepad.exe`、
  `winemenubuilder.exe`、`xrandr` 等子进程，`AF_INET/AF_INET6` 仍为 0。

### A6 Release 产物

- `ldd target/release/gamehub | grep -Ei 'gtk|qt|webkit|electron|node'`：
  无输出，判定 `no web/gtk/qt deps`。
- 实际动态依赖：`linux-vdso.so.1`、`libgcc_s.so.1`、`libm.so.6`、
  `libc.so.6`、`ld-linux-x86-64.so.2`；其余图形/字体库由运行时按需加载。
- 单个可执行文件 14 MB，不依赖额外资源目录（不打包字体/DXVK）。

### A7 字体与窗口

- 所有中文标签、按钮、错误提示正常渲染，无 `□□□`；使用 Noto Sans CJK SC。
- 窗口缩到 900×600 后布局不崩溃、内容可滚动。
- 本机缩放因子为 1，高 DPI 未单独验证。

### A8 映射

| v1 项 | 结果 |
|---|---|
| 1. 导入本地 exe、填名称/工作目录、保存 | 通过 |
| 2. 探测系统 Wine 或导入 Proton，`verified=true` | 通过 |
| 3. 创建 prefix（选 arch，可选 DXVK），UI 不卡顿 | 通过（Prefix 创建；DXVK 安装沿用 P8 验收） |
| 4. X11 输出/分辨率；Wayland gamescope；保存 | 部分通过：Wayland 显示配置与 gamescope 参数单测通过；X11 输出下拉、真实 gamescope 未执行（本机无 X11 会话、无 gamescope） |
| 5. 环境变量与结构化 DLL override；手设 WINEDLLOVERRIDES 拒绝 | 通过 |
| 6. 启动、子进程、错误展示、`last_played_at` | 通过（另验证退出码 7 的错误展示） |
| 7. 重启后全部配置恢复 | 通过（Runner、游戏、Prefix、显示、环境变量、DLL override、最近游玩） |
| 8. 全程无网络请求 | 通过（A5，0 条 AF_INET/AF_INET6） |
| 9. clippy/test 通过 | 通过（156 passed，1 ignored） |
| 10. release 单文件、无 WebKit/Qt/GTK | 通过 |

### A9 gamescope

- 参数级：`cargo test -p display gamescope` 10 passed。
- 真实 gamescope 未安装，A9 Step 2 未执行；安装后需补做启动命令行与
  `--backend wayland`/`-O` 兼容性验证。

## 未执行项与原因

| 项 | 原因 | 替代验证 |
|---|---|---|
| A9 Step 2 真实 gamescope | 本机未安装 gamescope | 10 项参数单测；Forced/Auto 在未检测到 gamescope 时的提示与拒绝 |
| X11 输出下拉/非 gamescope 分辨率 | 本机仅有 Wayland 会话 | `display` crate 的 xrandr 解析与 `__GL_SYNC_DISPLAY_DEVICE` 单测；Wayland 分支手工验证 |
| 高 DPI 缩放 | 当前缩放因子为 1 | 900×600 最小尺寸与滚动验证 |
| 系统级安装 strace | 未修改系统包管理 | 解压 strace 6.13 后执行规格命令，结果一致 |

## 偏差

- 自动化通过 XWayland + XTEST 完成点击/输入，截图用 `import -window`；
  会话仍按 `XDG_SESSION_TYPE=wayland` 选择显示分支。
- 隔离运行环境未提供桌面向导（session bus），rfd 回退 zenity；正常桌面
  应使用 xdg-desktop-portal，未改变链接依赖。
- 共享 Prefix 与同 Prefix 并发的第二个游戏通过 DB 预置，验证后删除。
- 游戏删除只删 DB 行，不删除 Prefix 目录；与 A3 Step 9 的“可另行删除”
  一致。
- 启动等待在窗口关闭后不会回收子进程，与 P9 已记录的行为一致。
