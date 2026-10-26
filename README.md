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
- **字体**：不打包字体，Iced 0.13（cosmic-text）加载系统字体并做缺字回退；
  需要系统安装中文字体（如 Noto Sans CJK SC、WenQuanYi Zen Hei）。

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

## 已知限制

- X11 非 gamescope 路径**不修改分辨率/刷新率**，只设置
  `__GL_SYNC_DISPLAY_DEVICE`；分辨率与刷新率仅在 gamescope 生效时可用。
- 数据库中的路径必须为 UTF-8，非 UTF-8 路径导入会直接报错。

## 检查

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

详细实施方案见 [`docs/plan.md`](docs/plan.md) 与
[`docs/specs/00-overview.md`](docs/specs/00-overview.md)。
