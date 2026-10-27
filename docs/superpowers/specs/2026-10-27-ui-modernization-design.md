# UI 现代化与游戏封面 设计规格

- 日期：2026-10-27
- 状态：已确认（视觉方向 C 浅色 + D1 深靛，封面可手动选择）
- 范围：`crates/ui`、`crates/launcher-core`、`crates/storage`、`crates/application`
- 相关视觉稿：`.superpowers/brainstorm/155478-1793095623/content/`（本地，不入库）

## 1. 背景

当前界面固定使用 `iced::Theme::Dark`，组件只有默认 `text` / `button` / `container`，
唯一的自绘样式是两个内联的容器圆角。结果是黑蓝单一配色、层级不清、状态不可辨、
游戏列表只是文本行。本次改版引入完整设计令牌、浅/深两套主题、侧边栏布局、卡片化
组件，并支持为每个游戏设置自定义封面。

## 2. 目标与非目标

### 目标

1. 默认浅色主题（方案 C · Soft Studio），可在设置中切换深色（D1 · Soft Indigo），
   切换立即生效并持久化。
2. 全局改为「左侧导航 + 右侧内容」布局；游戏库改为封面卡片网格。
3. 建立统一的设计令牌与组件层，替换现有内联样式。
4. 游戏详情（新建与编辑）可选择本地图片作为封面，或恢复默认渐变封面。
5. 保持现有全部功能与行为不变：启动、结束、日志、Prefix、DXVK、显示、环境变量、
   DLL Override、Runner 管理、设置保存、删除确认。

### 非目标

- 不修改 `launch` / `display` / `runner-wine` / `prefix` 的运行时逻辑。
- 不增加网络访问、遥测或在线资源。
- 不做封面裁剪/缩放预处理、拖拽换图、图片编辑器。
- 不做跟随系统主题（仅浅色/深色两档）。
- 不做搜索、筛选、排序选项（游戏仍按名称排序）。
- 不引入图标字体或 SVG 资源。

## 3. 设计令牌

令牌集中定义在 `crates/ui/src/theme.rs`，浅色与深色字段一一对应。

### 3.1 颜色

| 语义 | 浅色 | 深色 |
|---|---|---|
| canvas（页面底） | `#f4f5fb` | `#10121a` |
| sidebar（侧栏底） | `#ffffff` | `#151822` |
| surface（卡片） | `#ffffff` | `#191c28` |
| surface_alt（输入/分段底） | `#f8f9fd` | `#1e2230` |
| border | `#e6e8f2` | `#262b3a` |
| border_strong | `#d9dcec` | `#333a4d` |
| text | `#171a2b` | `#e8eaf3` |
| text_muted | `#5f6678` | `#9aa1b8` |
| text_faint | `#676e85` | `#848a9e` |
| primary | `#5b5bd6` | `#6060e0` |
| primary_hover | `#4f4fc4` | `#5a5ad8` |
| primary_soft | `#ecebff` | `rgba(96,96,224,.18)` |
| primary_text（soft 底上的强调文字） | #4f46c9 | #b9b9ff |
| on_primary | `#ffffff` | `#ffffff` |
| success | `#0a7a55` | `#34d399` |
| success_soft | `#e8f9f1` | `rgba(52,211,153,.13)` |
| warning | `#b45309` | `#fbbf24` |
| warning_soft | `#fef3c7` | `rgba(251,191,36,.14)` |
| danger | `#d11a43` | `#fb7185` |
| danger_soft | `#fff1f2` | `rgba(251,113,133,.13)` |
| shadow | `rgba(23,26,54,.07)` | `rgba(0,0,0,.5)` |
| backdrop（模态遮罩） | `rgba(23,26,54,.45)` | `rgba(0,0,0,.6)` |
| log_surface（日志面板） | `#0f1117` | `#0b0d13` |
| log_text | `#d7dbe8` | `#c9cedd` |

对比度要求：正文与底色、主按钮文字与主色均不低于 4.5:1；状态胶囊使用「浅底 + 深字」。

深色模式下 `primary` 只用于填充，`primary_soft` 底上的文字使用 `primary_text` 以满足 4.5:1 对比度。

### 3.2 字体与排版

- 默认字体保持 `Noto Sans CJK SC`（窗口级 `default_font`）。
- 字号/字重：
  - 页面标题 26 / Bold；区块标题 17 / Bold；卡片标题 14 / Semibold；
  - 正文与表单 13 / Regular；说明与元信息 11–12 / Regular；徽标与标签 10–11 / Bold。
- 文本层级通过颜色（text / text_muted / text_faint）+ 字号区分，不滥用彩色文字。

### 3.3 间距、圆角与阴影

- 间距刻度：4 / 8 / 12 / 16 / 24 / 32。
- 圆角：卡片 14px，输入框与按钮 8px，胶囊 999px，模态框 16px，封面占位 10px。
- 阴影：浅色卡片 `Shadow { color: shadow, offset: (0,2), blur_radius: 8 }`；
  深色卡片以 `border` 内描边为主，阴影只用于模态框。

## 4. 布局与页面

### 4.1 根布局

```
row[
  sidebar(width=200, fill_height),
  content(fill)
]
```

- 侧栏：Logo（图形块 + gameHub）、导航项「游戏库 / Runner 管理 / 设置」、
  底部会话信息（`Wayland`/`X11`/`未知` + 默认 Runner 名称）。
- 导航项：高 36、圆角 8、左侧 3px 选中竖条 + `primary_soft` 底 + `primary` 文字；
  非选中为 `text_muted`，悬停 `surface_alt` 底。
- 当前页面映射：`Library` 与 `Logs` → 游戏库；`GameDetail` → 游戏库；`RunnerManager` → Runner 管理；`Settings` → 设置。
- 内容区：padding 24，底部留 24；标题与操作在页头同一行。

### 4.2 游戏库

1. 页头：标题「游戏库」、副标题「N 个游戏 · M 个正在运行」、右侧「Runner 管理」次要按钮 +「新建游戏」主按钮。
2. 「正在游玩」主卡片：仅当存在 `starting` 或 `running` 游戏时显示，取第一个运行中游戏。
   内容：封面 56×76（3:4）、状态胶囊、名称、Runner 与运行时长/启动中提示、右侧「结束游戏」危险按钮 +「日志」次要按钮。
3. 「全部游戏」区块标题 + 数量。
4. 卡片网格：响应式列数，窗口宽度 ≥1100 时 4 列，否则 3 列；卡片宽度按内容区均分
   （上限 240），间距 16，封面 3:4。必要时通过 `window::resize_events()` 更新列数。
5. 卡片内容：封面（自定义图片或默认渐变）、左上角状态胶囊、名称（单行省略）、
   元信息（最近游玩 · Prefix 状态）、底部「启动/结束」「设置」「日志」小按钮。
6. 空状态：居中卡片，渐变圆形图标位、「还没有游戏」、「新建游戏」主按钮。
7. 加载中（`pending.loading`）：内容区显示「正在加载…」；`fatal` 仍使用独立错误页。

### 4.3 游戏详情

- 页头：返回按钮、游戏名、副标题（最近游玩 · 环境变量数 · DLL Override 数）、
  右侧「删除游戏」危险次要按钮 +「保存」主按钮。
- 主体：`row[ 左侧滚动区(fill), 右侧摘要卡(width=260, 不滚动) ]`，间距 24。
- 左侧卡片（顺序不变，控件不删减）：
  1. 基本信息：名称、可执行文件、工作目录、架构。
  2. 兼容层与 Prefix：Runner、Prefix 路径、状态胶囊、操作按钮、DXVK 目录与安装、提示文字。
  3. 显示：后端、gamescope、显示器 + 刷新、全屏开关、分辨率/刷新率（仅 gamescope 生效时）、各类提示。
  4. 环境变量：KEY/VALUE 行 + 删除 + 添加。
  5. DLL Override：名称 + 模式 + 删除 + 添加。
  6. 启动参数：参数行 + 删除 + 添加。
- 右侧摘要卡：封面预览、名称、Runner/架构说明、启动/结束主按钮、状态胶囊、
  「选择封面」「默认」两个次要按钮、事实列表（Prefix 状态、DXVK、最近游玩）。
  新建游戏时按钮置灰并提示「保存后才能启动」。
- 所有原有 `.size(12)` 提示改为 `text_faint` 的说明文字，保留原文案。

### 4.4 Runner 管理

- 页头：标题、说明、右侧「重新探测系统 Wine」「导入 Proton 目录」。
- 2 列卡片：名称 + 类型胶囊（Wine/Proton）、版本、已验证/未验证胶囊、默认胶囊、
  路径（faint、省略中间）、操作「设为默认」「删除」。
- 空状态：说明 + 两个导入/探测按钮。

### 4.5 设置

- 页头：标题 +「保存」主按钮（保存中禁用）。
- 卡片顺序：外观、默认 Runner、Prefix 根目录、全局环境变量、兼容层增强。
- 外观卡片：标题「外观」、说明「切换立即生效并保存」、右侧浅色/深色分段控件。
- 布尔项（Esync/Fsync/MangoHud/DXVK-NVAPI 自动安装）使用「标题 + 说明 + Toggler」行；
  DXVK-NVAPI 目录、MangoHud 检测提示保留。
- 保存按钮只提交除主题外的设置；主题单独即时保存。

### 4.6 日志

- 页头：返回、`{name} 的日志`、右侧「刷新」。
- 正文：深色代码面板（`log_surface` + `log_text` + 等宽字体 + 12px），padding 16，
  圆角 14，高度填满；面板顶部工具条显示行数。
- 无日志时在面板内显示「该游戏还没有启动日志」。

### 4.7 全局反馈

- 错误横幅：危险色软底卡片，左侧「错误」胶囊 + 标题，detail 小字，右侧「关闭」；
  固定显示在内容区顶部（页头下方）。
- 删除确认：居中模态框（遮罩 + 卡片），标题、说明、右下「取消」「确认删除」；
  标题使用游戏名 / Runner 名（由 UI 从当前状态解析），不直接使用 `DeleteTarget::label()`
  的 UUID 文本；`Esc` 不作为必需功能，取消按钮必须可用。
- 致命错误页：居中卡片，标题「gamehub 无法启动」、原因文本。

## 5. 组件库

新增 `crates/ui/src/widgets/` 下的可复用组件：

| 组件 | 说明 |
|---|---|
| `page_header` | 标题、副标题、右侧操作区 |
| `nav_item` | 侧栏导航项（选中态） |
| `card` | 标准卡片容器（surface + border + radius + padding） |
| `section_card` | 标题 + 可选说明 + 内容卡片 |
| `badge(tone)` | 胶囊；tone = Live / Starting / Warning / Danger / Neutral / Info |
| `status_badge` | 把 `launch_state_label` / `PrefixStatus` / `verified` / `default` 映射到 tone 与文案 |
| `switch_row` | 标题 + 说明 + `toggler` |
| `segmented` | 两段式选择（主题） |
| `cover` | 封面：自定义图片或渐变首字，固定尺寸参数 |
| `empty_state` | 空状态卡片 |
| `error_banner` | 错误横幅（重写） |
| `confirm_modal` | 居中确认模态框（替换顶部确认条） |

按钮样式函数（`styles.rs`）：`primary`、`secondary`、`danger`、`quiet`、`nav`，
每个都实现 default/hover/pressed/disabled 四种 `button::Status`。`text_input` 提供
default/hovered/focused（2px primary 边框 + 外发光）三态；`pick_list` 提供按钮与
菜单样式；`scrollable` 提供细滚动条（8px）。

所有样式函数通过 `tokens(theme)` 取色，禁止在视图里出现颜色字面量
（渐变封面的 6 组色值集中定义在 `theme.rs`）。

## 6. 主题系统

### 6.1 模型与持久化

- `ThemeMode { Light, Dark }`（`ui` crate），默认 `Light`。
- 设置键 `ui_theme`，值 `"light"` / `"dark"`；常量 `SETTING_UI_THEME` 定义在
  `application::settings`，并在 `application` 根模块 re-export。
- `SettingsState::from_map`：缺失或非法值一律回退 `Light`，不使其他设置解析失败。
- `to_map` 写入 `ui_theme`。
- 切换主题：更新 `SettingsState.theme` → 立即重绘 → 异步 `save_settings({ui_theme})`；
  保存失败保留当前主题并显示错误横幅。
- 主题保存串行化：仅在上一次主题写入完成后发起下一次；底部保存不包含 `ui_theme`，避免覆盖即时主题。

### 6.2 主题解析

- `App::theme(&self) -> iced::Theme` 返回 `Theme::custom(name, Palette { ... })`：
  - Light：background `#f4f5fb`、text `#171a2b`、primary `#5b5bd6`、success `#0f9f6e`、danger `#e11d48`
  - Dark：background `#10121a`、text `#e8eaf3`、primary `#6060e0`、success `#34d399`、danger `#fb7185`
- `iced::application(...).theme(App::theme)`。
- `tokens(theme: &Theme) -> &'static Tokens` 通过 `theme.extended_palette().is_dark`
  选择浅/深令牌表；所有自定义样式函数使用它。
- 内置控件（`pick_list` 菜单、`text_input` 选区、`toggler` 等）在自定义样式未覆盖的
  属性上依赖 `Theme::custom` 的 Palette。

## 7. 游戏封面

### 7.1 数据模型与存储

- `launcher_core::Game` 新增 `pub cover_path: Option<PathBuf>`。
- `AppPaths` 新增 `covers_dir: PathBuf`（`data_root/covers`），并在 `ensure_app_dirs`
  中创建。
- 数据库保存「应用数据目录内的绝对路径」，封面文件命名为
  `covers/<game_id>.<ext>`（ext 小写白名单：`png` `jpg` `jpeg` `webp` `bmp`）。
- 不引用用户原始文件：保存时复制，避免原文件移动/删除导致封面失效。

### 7.2 草稿与交互

- `GameDraft` 新增：
  - `cover_path: Option<PathBuf>`：已保存的封面；
  - `cover_source: Option<PathBuf>`：本次新选择、尚未落盘的源文件。
- 预览路径 = `cover_source` 优先，否则 `cover_path`。
- 新增 `DialogIntent::PickCover`（rfd，过滤器为上述图片扩展名）。
- 新增 `DraftAction::CoverSource(PathBuf)` 与 `DraftAction::ClearCover`：
  - 选择：设置 `cover_source`；
  - 「默认」：清空 `cover_source` 与 `cover_path`。
- `GameDraft::to_game` 把 `cover_path` 写入 `Game`。
- 游戏详情右侧摘要卡显示预览与「选择封面」「默认」；新建与编辑均可用。

### 7.3 保存流程

`AppService::save_game(game, is_new, cover_source: Option<PathBuf>, covers_dir: PathBuf)`
在 `spawn_blocking` 中（参数按值传入以满足 `'static`）：

1. 保留现有的 exe / working_dir 校验。
2. 若 `cover_source` 为 `Some`：
   - 校验扩展名白名单；
   - 用 `image` 读取尺寸：必须是可解析图片，宽高均 > 0，且单边 ≤ 8192；
   - 复制到 `covers_dir/<game_id>.<ext>`（覆盖同名文件）；
   - `game.cover_path = Some(目标路径)`；
   - 记录旧封面路径（编辑场景从数据库读取）。
3. 执行 `insert_game` / `update_game`。
4. 成功且旧路径与新路径不同：删除旧文件（失败仅 `tracing::warn!`）。
5. 若数据库写入失败：删除本次复制的文件（失败仅 warn），返回原错误。

清空封面（`cover_source=None` 且 `game.cover_path=None`）时，更新成功后删除旧文件。

### 7.4 删除游戏

`AppService::delete(DeleteTarget::Game(id), ..., logs_dir)` 在删除行前读取 `cover_path`，
删除数据库行后尽力删除封面文件与游戏日志 `logs_dir/games/<id>.log`；文件删除失败（日志文件不存在除外）不阻断删除。

### 7.5 渲染与回退

- App 维护 `cover_handles: HashMap<Uuid, iced::widget::image::Handle>`：
  `apply_loaded` 时为每个存在且为文件的 `cover_path` 创建 `Handle::from_path`；
  `DraftSaved` 后更新对应条目；删除后移除。
- `cover` 组件使用 `stack![gradient_tile, image]`：
  - 无 handle：只显示渐变首字；
  - 有 handle：图片以 `ContentFit::Cover` 填充，解码失败时自然露出底层占位。
- 渐变首字：取名称首个字符，6 组渐变按 `game_id` 字节哈希选择。
- 草稿预览使用 `cover_source`/`cover_path` 直接构造 `Handle::from_path`；源文件不可读时
  同样回退渐变封面，保存阶段再给出明确错误。
- 占位封面圆角 10；自定义图片因 iced 0.13 不支持图片圆角裁剪而保持直角，
  外层卡片仍为圆角并在图片四周保留 0–1px 内描边。

### 7.6 错误信息

新增应用错误（文案面向用户）：
- 「封面必须是 PNG / JPEG / WebP / BMP 图片」
- 「无法读取封面图片：{detail}」
- 「封面尺寸过大（{w}×{h}），单边不能超过 8192 像素」
- 「复制封面失败：{detail}」

错误通过现有 `ErrorNotice` 横幅展示。

## 8. 数据库迁移

- `SCHEMA_VERSION` 升至 `2`。
- 已有 v1 数据库：在事务中执行
  `ALTER TABLE games ADD COLUMN cover_path TEXT;`
  并将 `schema_version.version` 更新为 2。
- 新数据库直接创建含 `cover_path` 的 v2 结构。
- 版本高于支持的版本仍返回 `UnsupportedSchema`；v1 迁移失败时事务回滚并返回存储错误。
- 仓储层 `GAME_COLUMNS`、`insert_game`、`update_game`、`row_to_game` 增加该列。

## 9. 代码结构

```
crates/ui/src/
  theme.rs            # ThemeMode、Tokens、LIGHT/DARK、App::theme、tokens()
  styles.rs           # 按钮/容器/输入/下拉/开关/滚动条样式函数
  widgets/
    mod.rs            # re-export、path_text、format_timestamp
    choices.rs        # 现有 choice 辅助（保留）
    layout.rs         # page_header、nav_item、card、section_card、empty_state
    badges.rs         # badge、status_badge
    controls.rs       # 按钮构造、switch_row、segmented
    cover.rs          # cover 组件与渐变
    modal.rs          # confirm_modal
  views/{library,game_detail,runners,settings,logs}.rs   # 重写
  app/{mod,view,message,update}.rs                       # 主题、封面状态、消息
  draft.rs                                               # 封面字段与动作
  dialogs.rs                                             # PickCover
  lib.rs                                                 # theme + resize 订阅
```

其他 crate：

- `launcher-core/src/types.rs`：`Game.cover_path`。
- `launcher-core/src/paths.rs`：`covers_dir`。
- `storage/src/schema.rs`、`mapping.rs`、`repos/games.rs`：v2 与列读写。
- `application/src/{settings,games,delete,error,lib}.rs`：主题键、封面导入/清理、错误。
- `Cargo.toml`：iced features 增加 `"image"`；`application` 增加 `image = { version = "0.24", default-features = false, features = ["png", "jpeg", "webp", "bmp"] }`。
- `README.md`：数据目录表增加 `covers/`，已知限制补充「自定义封面直角（iced 0.13 限制）」。`CHANGELOG.md` 留到发布时再写。

## 10. 消息与状态

新增/修改：

- `Message::WindowResized(iced::Size)` + `window::resize_events()` 订阅（与日志刷新用
  `Subscription::batch` 合并），用于游戏库列数。
- `SettingsAction::Theme(ThemeMode)`。
- `Message::ThemeSaved(Result<(), String>)`。
- `DraftAction::CoverSource(PathBuf)`、`DraftAction::ClearCover`。
- `DialogIntent::PickCover`。
- `App` 新增 `cover_handles: HashMap<Uuid, image::Handle>`、`window_width: f32`。
- `App::subscription` 始终订阅窗口尺寸；仅在日志页额外订阅每秒刷新。

## 11. 测试

必须新增或更新：

1. `launcher-core`：`Game` 序列化包含 `cover_path`；现有构造器补字段。
2. `storage`：
   - 新库存在 `cover_path` 列且可往返；
   - v1 库升级到 v2 后保留原游戏数据并可写入封面路径；
   - 未来版本仍报 `UnsupportedSchema`。
3. `application`：
   - 保存带封面：文件被复制到 covers 目录，`game.cover_path` 指向副本；
   - 替换封面：旧文件被删除，新文件存在；
   - 清空封面：`cover_path` 为 `None` 且旧文件删除；
   - 非图片/超尺寸被拒绝，且不留下副本文件；
   - 删除游戏移除封面文件；
   - 封面导入失败时数据库不写入。
4. `ui`：`ui_theme` 解析默认值与非法回退、`to_map` 往返；草稿封面字段往返、
   `ClearCover` 清空；状态到胶囊文案的映射。
5. 更新所有 `Game { .. }` 测试构造。

验证命令：

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

## 12. 手工验收

在 GNOME Wayland 主机上运行 `cargo run -p ui`：

1. 首次启动为浅色；五个页面均可进入且布局正确。
2. 设置 → 外观切深色：立即变化；重启应用后仍为深色；切回浅色同样持久。
3. 新建游戏：选择 PNG 封面→预览出现→保存→返回游戏库卡片显示封面；重启后仍在。
4. 编辑游戏：替换封面（旧文件被删除）、点「默认」→回退渐变封面。
5. 删除游戏：模态框确认；确认后游戏与 `covers/<id>.*` 均被删除。
6. 运行中的游戏：库页出现「正在游玩」主卡；结束按钮可用；状态胶囊正确。
7. 日志页为深色代码面板且每秒刷新。
8. 用现有 v1 数据库副本启动，确认自动迁移且游戏列表、封面为空、功能正常。
9. 最小窗口 900×600 下 3 列网格不溢出；1180×760 下 4 列。

## 13. 风险与限制

- **图片圆角**：iced 0.13 的 `image` 组件不支持圆角裁剪，自定义封面为直角；
  通过外层圆角卡片与细描边弱化差异。
- **解码开销**：封面仅在渲染可见卡片时解码并由 iced 缓存；导入时限制尺寸，
  避免超大纹理。文件缺失时使用缓存的 handle 判定，不每帧访问磁盘。
- **二进制体积**：启用 `image` 特性与 codec 会增加若干 MB（当前 release 14 MB）。
- **字体字重**：若系统只安装了 Noto Sans CJK SC 的 Regular/Bold，Semibold 可能
  回退/合成；不影响布局。
- **迁移安全**：迁移在事务中执行；失败回滚并报错，不会留下半迁移数据库。
- **路径失效**：封面存绝对路径；数据目录整体移动后封面回退为默认封面（不崩溃）。
