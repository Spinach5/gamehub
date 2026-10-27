# gameHub

Linux 离线 Windows 游戏启动器（Rust + Iced）。导入本地 exe、管理 Wine/Proton
runner、创建 prefix、注入用户提供的 DXVK、配置显示与 gamescope、启动游戏。
运行期不发起任何网络请求。

## 外部依赖

- **运行器**：系统 Wine（如 `/usr/bin/wine`），或用户自备的 Proton /
  GE-Proton 目录。应用不下载 Wine/Proton。
- **DXVK**：用户提供解压后的发布目录（含 `x64`/`x32` 的 dll）。
  应用只负责复制与注册表写入，不下载。
- **文件对话框**：rfd 0.17 使用 xdg-desktop-portal（D-Bus），不依赖 GTK；
  桌面环境需要提供 portal 服务。
- **显示**：X11 路径使用 `xrandr`；Wayland 可选 `gamescope`（未安装时
  仅能构造参数，启动会给出明确错误）。
- **字体**：不打包字体。ui 将默认字体设为 `Noto Sans CJK SC`（实测 Iced
  0.13 默认 SansSerif 在本机不回落 CJK）；系统需安装该字体，或将其改为
  其他已安装的中文字体。

## 构建与运行

```bash
cargo build --release
./target/release/gamehub
```

开发运行：

```bash
cargo run -p ui
```

## 数据目录

应用数据位于 `$XDG_DATA_HOME/gamehub`，未设置时回退到
`~/.local/share/gamehub`：

| 路径 | 用途 |
|---|---|
| `gamehub.db` | SQLite 数据库 |
| `logs/` | 每日滚动日志 |
| `prefixes/` | 默认 prefix 根（可在设置中覆盖） |
| `steam-compat-client/` | Proton 的 `STEAM_COMPAT_CLIENT_INSTALL_PATH` |
| `covers/` | 游戏封面副本（自定义封面） |

## 已知限制

- X11 非 gamescope 路径**不修改分辨率/刷新率**，只设置
  `__GL_SYNC_DISPLAY_DEVICE`；分辨率与刷新率仅在 gamescope 生效时可用。
- 数据库中的路径必须为 UTF-8，非 UTF-8 路径导入会直接报错。
- 自定义封面在 iced 0.13 中无法做圆角裁剪，显示为直角；默认渐变封面为圆角。
- 删除确认模态框打开时，滚轮仍可能滚动背后的页面（iced 0.13 的 opaque 只拦截点击）。
- 删除封面时按数据库记录的路径直接移除，不校验该路径是否位于 `covers/` 之下；
  请勿手工把 `cover_path` 改成封面目录以外的文件。

## 检查

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 验收记录

2026-10-27 在 Wine 10.0 + Wayland/XWayland 环境完成 v2 验收：自动检查 156
passed / 1 ignored，release 构建 14 MB，Wine 与 GE-Proton11-7 端到端启动
notepad 通过，`strace` 日志无 `AF_INET/AF_INET6`。完整证据、未执行项和偏差
见 [`docs/acceptance.md`](docs/acceptance.md)，门槛定义见
[`docs/specs/11-acceptance.md`](docs/specs/11-acceptance.md)。

本机验收限制：

- 未安装 gamescope，只验证了参数级单测以及未检测到 gamescope 时的提示/
  拒绝；安装后需补做真实启动。
- 无 X11 会话，X11 输出下拉与非 gamescope 分辨率未手工验证。
- 自动化通过 XWayland + XTEST/`import -window` 完成；高 DPI（缩放因子 1）
  未单独验证。
- 系统未安装 `strace`，离线检查使用解压到临时目录的 6.13 版本执行。

详细实施方案见 [`docs/plan.md`](docs/plan.md) 与
[`docs/specs/00-overview.md`](docs/specs/00-overview.md)。
