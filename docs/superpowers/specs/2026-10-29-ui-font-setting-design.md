# 界面字体设置设计规格

- 日期：2026-10-29
- 状态：已确认（外观卡片内联展开、默认仅显示中文字体、点击立即生效并保存）
- 范围：`crates/ui`（新增字体扫描模块、主题字体状态、设置页字体面板）、`crates/application`（新增设置键常量）
- 非范围：日志/winetricks 输出使用的等宽字体（保持 `Font::MONOSPACE`）、按语言自动切换字体、字体文件安装或下载

## 1. 背景

界面所有文本目前硬编码为 `Noto Sans CJK SC`（`crates/ui/src/theme.rs` 的 `FONT_FAMILY`，
以及 `crates/ui/src/lib.rs` 的 `default_font`）。用户机器上若没有该字体，或希望换成更
符合自己审美的字体，没有任何入口。

本规格增加「界面字体」设置：扫描本机已安装字体（fontconfig），在设置页「外观」卡片内
内联展示，点击即切换并立即保存。

## 2. 目标与非目标

### 目标

1. 设置页「外观」卡片新增「界面字体」行，显示当前字体名与「更换字体」按钮。
2. 点击后展开内联面板：搜索框、「仅显示支持中文」开关、可滚动的字体列表。
3. 列表每一项用该字体本身渲染预览文本（`游戏库 Aa 123`），点击即应用。
4. 应用后立即刷新整个界面并持久化（与主题/语言一致），失败时显示错误横幅。
5. 默认选项「Noto Sans CJK SC」固定置顶，选中后写回默认值。
6. 字体扫描失败、保存的字体已卸载时优雅降级，不让界面变成空白文字。

### 非目标

- 不解析字体文件、不引入 `font-kit` / `fontdb` 等新依赖；只调用系统 `fc-list`。
- 不支持字体大小、行高、字重设置。
- 不做「仅显示支持中文」之外的字体过滤（如等宽、手写体分类）。
- 不改变日志与 winetricks 输出面板的等宽字体。
- 不监听系统字体安装变化；提供「重新扫描」按钮手动刷新。

## 3. 架构

### 3.1 字体扫描（`crates/ui/src/fonts.rs`）

```rust
pub struct FontEntry {
    pub name: &'static str, // 扫描时 Box::leak 一次，满足 Family::Name 的 'static 要求
    pub cjk: bool,
    pub has_bold: bool,
}

pub fn scan() -> Result<Vec<FontEntry>, String>;
fn parse_fc_list(output: &str) -> Vec<FontEntry>;
```

- 调用 `fc-list --format='%{family[0]}\t%{style[0]}\t%{lang}\n'`。
- 每行 `family \t style \t langs`：
  - `family` 去空白后为空则跳过；
  - `cjk` = `langs` 以 `|` 分割后存在以 `zh-` 开头的 tag 或恰为 `zh`（覆盖 `zh-cn` / `zh-tw` / `zh-hk`）；
  - `has_bold` = 同一 family 下任一 style 含 `Bold`（大小写不敏感）。
- 同一 family 去重合并：`cjk` 与 `has_bold` 取并集，最终按名称（大小写不敏感）排序。
- 过滤 fontconfig 的别名/重复项：family 以 `.` 开头（如 `.AquaBase`）或包含 `Noto Sans CJK SC` 的
  重复条目由去重自然处理。
- 解析函数是纯函数，单元测试直接喂样本字符串。
- `scan()` 在 `spawn_blocking` 中执行，返回 `Result`，错误带上 `fc-list` 的 stderr 摘要。

### 3.2 运行时字体状态（`theme.rs`）

`Family::Name` 需要 `&'static str`，因此运行时切换用进程级静态状态：

```rust
pub struct ActiveFont {
    pub family: &'static str,
    pub has_bold: bool,
}

static ACTIVE: RwLock<Option<ActiveFont>> = /* None 表示默认字体 */;

pub fn set_active_font(family: Option<(&str, bool)>); // 泄漏名称转 'static
pub fn active_font_name() -> &'static str;            // 默认返回 FONT_FAMILY
pub fn font(weight: Weight) -> Font;
```

- `set_active_font(None)` 恢复 `FONT_FAMILY`。
- `font(weight)`：字体族取自 `ACTIVE`；若该族没有 Bold 面，则所有字重强制 `Normal`。
  这是现有 `shipped_weight` 的推广——请求不存在的字体会让 iced 回退到无中文字形的
  面，中文直接不可见（见 commit `71cf3c6`）。
- 有 Bold 面时沿用现有 `shipped_weight` 映射（Semibold 及以上 → Bold）。
- 名称泄漏只发生在用户切换字体时，次数有限，可接受。
- 写锁在绘制线程读取，读操作极短；`RwLock` 毒化时回退默认字体。

`lib.rs` 的 `default_font(Noto Sans CJK SC)` 保留为兜底：未设置字体、扫描未完成、
字体名无效时界面仍可正常显示中文。

### 3.3 持久化

- `crates/application/src/settings.rs` 新增 `SETTING_UI_FONT = "ui_font"`。
- `SettingsState` 新增 `font: Option<String>`：
  - `from_map`：读取 `ui_font`，空串视为 `None`；
  - `to_map`：`Some(name)` 写原名，`None` 删除该键（保存为 `None`）。
- 应用启动时 `apply_loaded`：若设置里的字体名不在扫描结果中，回退默认字体并保留设置
  值不变（用户可能只是暂时卸载了字体），同时不阻塞启动。
- 保存路径复用 `SaveSettings`（与 `ui_theme` / `ui_language` 一样在批量保存时排除，
  由切换动作单独即时保存）。

## 4. 界面接入

### 4.1 外观卡片

```
┌ 外观 ───────────────────────────────┐
│ 主题        [ 浅色 | 深色 ]          │
│ 语言        [ 简体中文          ▾ ]  │
│ 界面字体    Noto Sans CJK SC  [更换] │
│ ┌─（展开时）──────────────────────┐ │
│ │ [搜索字体…            ] [仅中文 ●]│ │
│ │ ┌──────────────────────────────┐ │ │
│ │ │ ✓ Noto Sans CJK SC  游戏库 Aa│ │ │
│ │ │   WenQuanYi Zen Hei 游戏库 Aa│ │ │
│ │ │   ...（最高 240px 滚动）     │ │ │
│ │ └──────────────────────────────┘ │ │
│ │ [重新扫描]                       │ │
│ └──────────────────────────────────┘ │
└─────────────────────────────────────┘
```

- 行的右侧按钮在展开时显示「收起」，收起时显示「更换字体」。
- 字体名过长使用现有 `middle_truncate`。
- 面板每行：
  - 左侧当前选中项显示 ✓；
  - 字体名用该字体自身渲染（`text(name).font(Font { family: Family::Name(...) })`），
    预览文本与字体名同行，右侧显示「无中文」徽章（仅未过滤时可能出现）；
  - 点击行应用该字体。
- 「Noto Sans CJK SC」固定为第一项，代表默认字体；选中它等价于 `set_active_font(None)`
  并在设置中删除 `ui_font`。
- 默认开启「仅显示支持中文」：列表只含 `cjk == true` 的字体。关闭后显示全部，未过滤
  且搜索为空时最多渲染 80 项，并提示「仅显示前 80 项，请使用搜索缩小范围」。
- 搜索对字体名做大小写不敏感子串匹配；搜索时结果上限仍为 80。

### 4.2 App 状态与消息

新增状态：

| 字段 | 说明 |
| --- | --- |
| `fonts: Vec<FontEntry>` | 扫描结果（含 `has_bold`） |
| `fonts_loading: bool` | 扫描中 |
| `fonts_error: Option<String>` | 扫描失败信息 |
| `font_panel_open: bool` | 面板是否展开 |
| `font_search: String` | 搜索框内容 |
| `font_cjk_only: bool` | 默认 `true` |
| `font_saving: bool` | 字体保存中 |

新增消息：

```rust
FontPanelToggled,
FontSearchChanged(String),
FontCjkOnlyToggled(bool),
FontSelected(Option<String>),   // None = 默认字体
FontsLoaded(Result<Vec<FontEntry>, String>),
ScanFonts,
FontSaved { name: Option<String>, result: Result<(), String> },
```

- `FontSelected`：立即 `theme::set_active_font`、更新 `settings.font`、收起面板、发起
  `save_settings`（仅 `ui_font` 一键），`font_saving` 期间按最新选择补发保存。
- `FontsLoaded`：替换列表，`fonts_loading = false`；成功时若设置字体存在则应用，不存在
  则保持默认并显示一次性提示（`NoticeKind::FontSave` 复用同一 `ErrorNotice` 通道）。
- 扫描在 `App::new` 完成首屏加载后通过 `spawn_blocking` 启动，不阻塞窗口出现。
- 「重新扫描」重发扫描任务。

### 4.3 全局字体覆盖

要让切换真正生效，必须给所有未显式设置字体的文本与输入控件补上 `theme::font(...)`：

- `widgets/layout.rs`：`muted`、`faint`、`nav_item` 中的文本；
- `widgets/controls.rs`：`quiet`、`quiet_small`（其余按钮已使用 `label`）；
- `views/logs.rs`：错误文本、空状态、行数文本（日志正文保持等宽）；
- 所有 `text_input` / `pick_list` / `toggler` 调用补 `.font(theme::font(Weight::Normal))`。

已使用 `text(...).font(font(weight))` 的位置无需改动。`widgets/badges.rs` 若未设置字体需补齐。

新增的字体预览行是唯一使用「非当前字体」的地方。

### 4.4 i18n

两种语言各新增以下 key（分区 `settings.font.*`）：

| key | 中文 | English |
| --- | --- | --- |
| `settings.font` | 界面字体 | Interface font |
| `settings.font.change` | 更换字体 | Change font |
| `settings.font.collapse` | 收起 | Collapse |
| `settings.font.search` | 搜索字体… | Search fonts… |
| `settings.font.cjk_only` | 仅显示支持中文 | Chinese-capable only |
| `settings.font.rescan` | 重新扫描 | Rescan |
| `settings.font.loading` | 正在扫描字体… | Scanning fonts… |
| `settings.font.empty` | 没有匹配的字体 | No matching fonts |
| `settings.font.no_cjk` | 无中文 | No Chinese |
| `settings.font.preview` | 游戏库 Aa 123 | Library Aa 123 |
| `settings.font.missing` | 已保存的界面字体未安装，已回退到默认字体 | The saved interface font is not installed; using the default font |
| `settings.font.truncated` | 仅显示前 80 项，请使用搜索缩小范围 | Showing the first 80 fonts; search to narrow down |
| `settings.font.default` | 默认 | Default |
| `errors.font_scan` | 无法列出系统字体：{error} | Cannot list system fonts: {error} |
| `app.error.font_save` | 保存字体失败 | Failed to save font |

## 5. 测试

| 测试 | 内容 |
| --- | --- |
| `parser_dedupes_families_and_detects_cjk_and_bold` | 同族多条合并、`zh-cn` 判定、Bold 判定、空 family 跳过 |
| `parser_sorts_names_case_insensitively` | 输出按名称（大小写不敏感）排序 |
| `parser_skips_empty_family_names` | 空 family 行被丢弃 |
| `matching_is_case_insensitive` | 搜索过滤大小写不敏感 |
| `font_weights_snap_to_the_shipped_faces` | 有 Bold 面时 `Semibold/Bold` 映射到 Bold |
| `active_font_replaces_the_family_until_reset` | `resolve_font(Some(..))` 使用新族，`None` 恢复默认 |
| `families_without_a_bold_face_use_regular` | 无 Bold 面时任意权重都落到 Normal |
| `settings_round_trip_preserves_font` | `ui_font` 随设置保存/读取往返，空串解析为 `None` |
| `font_selection_updates_settings_and_closes_the_panel` | 选择字体后设置更新、面板收起 |
| `selecting_the_default_font_clears_the_setting` | 选择默认字体清除设置值 |
| `unknown_selected_font_falls_back_to_the_default` | 未知字体名回退默认 |
| `stored_font_missing_from_the_scan_reports_once` | 已保存字体未安装时只提示一次 |
| `stale_font_save_resaves_the_latest_choice` | 保存期间再次切换会补发最新值 |
| `catalogues_share_keys_and_placeholders` / `keys_used_in_source_exist` | 新增 i18n key 自动被现有守卫测试覆盖 |

手工验收（GNOME Wayland 下无法自动截图）：切换 2–3 种字体与两种主题，确认中文、英文、
数字与按钮文字均可见；卸载/伪造不存在字体名启动不崩溃；`fc-list` 不可用时设置页仍可
打开并显示错误提示。

## 6. 兼容性与风险

- 旧用户没有 `ui_font`：默认字体不变，行为与升级前一致。
- 保存的字体被卸载：回退默认字体并提示，设置值保留以便字体恢复后自动生效。
- 非中文界面用户关闭「仅显示中文」后可选择任意字体；选择无中文字形的字体时，中文
  会显示为方框或空白——这是用户显式选择的结果，列表用「无中文」徽章预警。
- 部分字体没有粗体面：`has_bold == false` 时整体使用 Normal，避免 iced 回退到无中文
  字形面导致文字消失。
- `fc-list` 缺失（极简发行版）：扫描失败，仅保留默认字体与「重新扫描」，界面正常。
- 字体列表最多一次性渲染 80 行，避免 500+ 字体时滚动卡顿。
