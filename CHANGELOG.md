# Changelog

本文件记录每个实施阶段完成的内容、与规格的偏差以及验证结果。

## v0.10.0 — DLL Override 目录扫描（2026-10-31）

### 完成内容

- 应用层新增 `application::dll_scan`（`scan_dlls`、`DllCandidate`、
  `DllScanOutcome`、`DllScanError`、`dll_name_key`）：显式栈深度优先遍历，
  不跟随目录符号链接，按小写基名去重，非 UTF-8 路径跳过并计数；上限为
  深度 16、结果 5000、访问文件 200000，超限以 `truncated`/`depth_limited`
  标记。设置新增前缀 `dll_scan_dir.`，按游戏保存上次成功扫描的目录
  （`dll_scan_dir.<game-id>`）。
- UI：游戏详情页 DLL Override 卡片新增「从目录添加」，与手动入口并存。
  模态框默认扫描可执行文件所在目录或该游戏上次的扫描目录，支持更改目录、
  重新扫描、实时搜索、名称升降序、全选/反选/清空与统一 Override 模式
  （复用现有四档，默认 `native,builtin`）。已有 Override 显示「已添加」
  并禁用复选框；列表渲染上限 300 条，超出时提示用搜索缩小范围。确认后
  批量追加草稿并复用 `save_draft` 落库；成功后关闭对话框并在页面顶部显示
  「已添加 N 个 DLL Override」，校验或写库失败则回滚新增行、保留勾选并
  在对话框内显示错误。扫描在 `spawn_blocking` 中执行，`update` 不做文件
  系统 IO；`scan_seq` 丢弃过期结果，关闭对话框即取消扫描。
- 错误与样式：新增中英文 `detail.dll_scan.*`、`errors.dll_scan.*`、
  `widgets.success`，覆盖目录不存在、无权限、非目录、读取失败与取消，
  空目录提示「该目录下未找到 .dll 文件」；新增 `styles::checkbox` 三态
  （选中/悬停/禁用，浅色与深色）与成功提示条组件。
- 文档：DLL 扫描选择器设计与实施计划写入 `docs/superpowers/`，
  `docs/specs/10-ui-config-launch.md` 追加当前实现说明。

### 与规格的偏差

- 结果态的模态卡片改为高度自适应：外层留 20px、卡片 `height: Fill`、
  列表高度改为 `Fill`。固定 `max_width(780)` / `max_height(640)` 与
  300 行渲染上限仍保留，避免截断/深度警告把页脚挤出窗口。
- 扫描目录写库失败只记录 `tracing::warn!`，不向用户报错（规格已确认）。

### 验证

- 自动：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets
  -- -D warnings` 通过；`cargo test --workspace` 350 passed，0 failed，
  2 ignored（真实 Wine prefix 测试仍为 ignored）。
- 实机（隔离 `XDG_DATA_HOME`，复用现有游戏与用户设置的 Microsoft JhengHei
  字体）：大小写与同名 DLL 去重、非 UTF-8 计数、勾选与 `native` 模式落库、
  成功提示、重开后「已添加」禁用、搜索/全选/反选/清空/排序/重扫、10,000
  个 DLL 与 15 万文件目录的响应性、空目录/不存在/无权限文案、浅色与深色
  下复选框图标与文字字迹、手动「添加 DLL Override」均通过。
- 未覆盖：扫描完成过快（15 万文件 <0.4s），未能实机点击「取消扫描」；
  取消语义由 `closing_cancels_a_running_scan_but_not_while_saving` 与
  `stale_scan_results_are_ignored` 单测覆盖。「更改扫描目录」的 Wayland
  portal 对话框无法用 X11 截图捕获，仅确认按钮与
  `DialogIntent::PickDllScanDir` 接线。

## v0.9.0 — Linux 原生游戏（2026-10-30）

### 完成内容

- 领域与存储：新增 `Platform { Windows, Linux }`（默认 `Windows`，serde
  `snake_case`，`is_native()` 标识原生分支），`Game.platform` 进入固定契约；
  SQLite schema 升到 v4（`platform TEXT NOT NULL DEFAULT 'windows'`），
  v1/v2/v3 库链式迁移原地升级，已有记录默认为 Windows；行映射与
  `update_game` 同步读写该字段。
- 编辑器：`GameDraft::new` 增加平台参数，新增 `DraftAction::Platform`；
  Prefix 的空路径/相对路径校验只对 Windows 生效。切换平台不清空 runner、
  prefix 与 DLL override，切回 Windows 后原值继续生效。
- 启动管线：`LaunchRequest.runner` 改为 `Option<Runner>`。原生游戏在
  `validate` 中跳过 runner 校验、prefix 状态与架构检查、DLL 名称校验，
  `build_plan` 直接执行 `executable_path`；`build_env` 不注入任何 Wine/Proton
  基础变量与调优变量（Esync/Fsync/MangoHud/NVAPI 均不生效），仍应用
  全局/游戏环境变量、启动参数、显示器选择与 gamescope 包装。
- 应用层：原生游戏跳过 prefix 计划与 DXVK-NVAPI 安装；运行态改为记录
  `wine_prefix: Option<PathBuf>`；会话结束只等待启动器进程组，停止也只终止
  进程组，不调用 `wineserver -k`。
- UI：新建游戏先弹出平台选择（Windows / Linux 原生），Windows 文件对话框只
  过滤 `.exe`、Linux 不过滤；详情页新增平台下拉（可随时切换），原生游戏隐藏
  Runner 与 DLL Override 卡片、架构字段、Prefix/DXVK 摘要与 winetricks 区块，
  但保留其数据；游戏卡片对原生游戏显示「Linux 原生」而非 Runner 名称；
  `running` 改为 `HashMap<Uuid, Option<PathBuf>>`。
- 中英文各新增 11 个文案键（`new_game.*`、`choices.platform.*`、
  `detail.platform`、`detail.subtitle.*_native`），游戏库空状态文案由
  「选择一个 .exe」改为「添加游戏」。

### 与规格的偏差

- 原生游戏没有 `wineserver -k` 兜底：若游戏自行 daemonize 脱离进程组，会话
  会被提前判定结束，停止按钮也可能失效。
- 显示选择对原生游戏只在 X11/XWayland 下注入 `__GL_SYNC_DISPLAY_DEVICE`；
  纯 Wayland 原生窗口无法保证落到所选显示器，也不做 primary 切换。
- 文件对话框只按平台设置过滤器，未额外校验扩展名：Windows 游戏仍可选到
  非 `.exe` 文件。
- 平台只影响启动路径，不迁移数据：把 Windows 游戏改为 Linux 后其 prefix 目录
  仍留在磁盘上，改回 Windows 时继续使用原路径。
- winetricks 组件与 DXVK/NVAPI 入口对原生游戏直接隐藏，没有额外说明文案。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：329 passed，0 failed，2 ignored（新增平台序列化、
  schema v3→v4 迁移、编辑器平台校验、启动计划与环境合成、应用层会话与 UI
  状态测试）。
- 原生启动只对照测试可执行文件验证，未覆盖真实商业游戏；gamescope 仍未安装，
  相关路径只有参数级测试。详见 `docs/specs/11-acceptance.md`「当前增量验收」。

## v0.8.0 — 界面字体切换（2026-10-29）

### 完成内容

- 新增 `ui::fonts`：在 `spawn_blocking` 中调用
  `fc-list --format='%{family[0]}\t%{style[0]}\t%{lang}\n'` 扫描本机字体家族，
  去重并合并同一家族的多条记录（`cjk`、`has_bold` 取并集），跳过空名与
  fontconfig 内部别名（`.` 开头），按名称不区分大小写排序；解析为纯函数，
  单测直接喂样本输出。
- 运行时字体状态（`ui::theme`）：进程级 `RwLock<Option<ActiveFont>>`，
  `theme::font(weight)` 优先使用当前选择的家族；`Family::Name` 要求
  `&'static str`，扫描时一次性 `Box::leak` 家族名。没有 Bold 面的家族强制使用
  Regular，避免 iced 回退到无中文字形的面导致文字消失。
- 设置 →「外观」卡片新增「界面字体」行与内联面板：搜索、重新扫描、
  「仅显示支持中文」开关（默认开）、最多 80 项（超出提示用搜索缩小范围）、
  「无中文」预警徽章与选中标记；点击即套用并保存。
- 新增设置键 `ui_font`：点击后异步写入，保存期间再次点击会补存最新选择；
  选「默认」清除该键。启动时由 `apply_loaded` 与 `FontsLoaded` 共同恢复
  （两者任一先完成都能收敛到正确状态）；已保存字体未安装时回退默认并只提示
  一次，设置值保留以便字体恢复后自动生效。
- 全局字体覆盖：所有文本、按钮、`text_input`、`pick_list`、徽章等改用
  `theme::font(...)`；日志正文与 winetricks 输出保持等宽。勾选符号固定使用
  内置字体，避免所选字体缺 U+2713 时选中状态不可见。
- 中英文各新增 15 个文案键（`settings.font.*`、`errors.font_scan`、
  `app.error.font_save`）。

### 与规格的偏差

- `FontEntry.name` 使用 `&'static str`（扫描时泄漏一次）而非 `String`，
  以满足 `Family::Name` 的静态生命周期要求；扫描次数有界。
- 保存失败使用 `app.error.font_save`（不带错误参数），而不是规格中的
  `errors.font_save`。
- 新增规格表外的 `settings.font.preview`（候选字体预览串）与
  `settings.font.missing`（已保存字体缺失提示）键。
- 候选名称用当前界面字体渲染，预览串用候选字体本身渲染；否则中文家族名在
  缺字形的家族下会显示为方框。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：311 passed，0 failed，2 ignored（含字体解析、
  运行时切换、设置往返与保存竞态测试）。
- 实机走查（X11 下抓取 iced 窗口）：浅色 / 深色主题下中文、英文、按钮文字
  均可见；选择 DengXian 后整个界面立即切换，`ui_font` 写入数据库，重启后
  保持；面板名称与预览完整显示，选中项有勾选标记。走查结束后已还原测试设置。

## v0.7.0 — 通过系统 winetricks 安装 Windows 组件（2026-10-29）

### 完成内容

- `prefix` crate 新增 winetricks 支持：在 `PATH` 中探测可执行的系统
  `winetricks`（不捆绑、不下载），校验 verb（仅 `[a-z0-9._=-]`、不得以 `-`
  开头、禁止 `list-all` 等管理命令、长度 ≤ 64），从
  `${WINEPREFIX}/winetricks.log` 读取已安装 verb（去重、剔除
  `w_workaround*` 内部项），并为 Wine / Proton 构造调用命令：
  Wine 使用 prefix 目录与 `WINE`；Proton 使用 `<compat>/pfx` 作为
  `WINEPREFIX`、`files/bin/wine` 作为 `WINE`，同时传入
  `STEAM_COMPAT_DATA_PATH`、`STEAM_COMPAT_CLIENT_INSTALL_PATH` 与
  `PROTONPATH`。
- `application` crate 新增组件安装服务：每次只跑一个 verb（`winetricks -q`），
  独立进程组启动，stdout/stderr 逐行流式回传并写入
  `logs/winetricks/<game>.log`（每次运行截断重写）；取消时用进程组
  SIGTERM→SIGKILL，连同 winetricks 拉起的安装器一起结束。安装前后校验 prefix
  已初始化，非法 verb 返回 `AppError::ComponentVerbInvalid`。
- `launcher-core` 新增 `AppPaths::component_log_path`。
- 详情页「兼容层与 Prefix」卡片内内联「Windows 组件」区块：预设
  （vcrun2019 / dotnet48 / corefonts / d3dx9 / xact / msxml6）与自定义 verb、
  已安装列表、最近 4 行实时输出、取消按钮与完整日志路径。未检测到 winetricks、
  游戏未保存、Prefix 未初始化、同 Prefix 正在运行或安装、删除游戏 / Runner /
  Prefix 时均有拦截与提示。
- 中英文各新增 31 个文案键；`AppError::ComponentVerbInvalid` 与
  `AppError::ComponentIo` 已接入本地化错误渲染。

### 与规格的偏差

- 只调用系统 `winetricks`：未检测到时仅提示用户安装，不自动下载或内置副本。
- 已安装状态以 prefix 内的 `winetricks.log` 为准，不写入数据库；手工执行的
  winetricks 安装同样会被列出。
- 每次安装都会截断该游戏的组件日志，不保留历史运行记录。
- 取消只负责终止进程，不清理 winetricks 已下载/解压的中间文件。
- 预设列表是固定集合，其余组件需在自定义输入框填写 verb。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo test --workspace`：299 passed，0 failed，2 ignored。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- 真实 winetricks 20250102：在 `/tmp` 临时 prefix 上按组件安装使用的环境
  （`WINEPREFIX` + `WINE=/usr/bin/wine`）执行 `winetricks -q fontsmooth-rgb`，
  退出码 0，`winetricks.log` 写入 `fontsmooth=rgb`，与已安装列表读取逻辑一致。
- 界面走查受限：本机为 GNOME Wayland，无法截取 iced 窗口，组件区块的布局与
  文案只做了编译与键值检查，未做像素级人工走查。

## v0.6.0 — 游玩时长统计与启动会话保活（2026-10-29）

### 完成内容

- 数据库 schema 升到 v3：`games.playtime_seconds INTEGER NOT NULL DEFAULT 0`；
  新增 v1→v3、v2→v3 迁移，已有库原地升级且游戏数据保留。`Game` 与
  `GameDraft` 同步增加该字段，插入语句与行映射一并更新。
- 启动会话在开始时就写入 `last_played_at`（`LaunchEvent::Started` 携带
  `started_at`），即使启动器随后崩溃也能保住「最近游玩」；会话结束时按墙钟差
  调用 `add_playtime` 累加总时长，`UPDATE` 用 `MAX(..., 0)` 保证不会出现负值。
  `update_game` 刻意不写时长，UI 合并草稿时对 `last_played_at` 与
  `playtime_seconds` 取 `max`，保存草稿不会把运行中的统计改小。
- 时长展示：详情页摘要新增「总时长」，游戏库卡片在「最近游玩」下方新增
  「总时长」行；两者都经 `widgets::format_playtime` 本地化，0 秒显示
  「0 分钟」，不足 1 分钟显示「1 分钟」，超过 1 小时显示「x 小时 y 分钟」。
- 会话保活：只有启动器进程组（`killpg(pgid, 0)`）与 Wine prefix 内的进程
  （扫描 `/proc/*/environ` 的 `WINEPREFIX=`，按 `/proc/<pid>/comm` 排除
  `wineserver` 自身）都为空时才结束会话，250 ms 轮询一次。修复 `proton run`
  等 wrapper 先于游戏退出时，UI 把仍在运行的游戏误判为空闲、会话时长被截断、
  停止按钮失效的问题。
- 封面加载全部改走 iced `Task`：首次加载、保存后刷新都在主线程外读取文件字节，
  只把 `image::Handle` 交给界面，启动时不再被磁盘 I/O 阻塞。
- 架构调整：`GameDraft` 及其校验从 `ui::draft` 迁到 `application::editor`，
  `ui` 只依赖 `application`（由后者重导出所需的 `launcher-core` 类型），
  本地化错误渲染仍留在 `ui::i18n::errors`。

### 与规格的偏差

- 只持久化累计总时长，不保留逐次会话记录；精度为秒，展示按分钟取整，不显示秒。
- 时长统计的是「启动到进程组与 prefix 都清空」的墙钟时间，包含 wineserver
  与 prefix 退出清理的耗时。
- `last_played_at` 的语义由「退出时间」改为「本次启动时间」；v0.5.0 之前写入的
  历史值不追溯修正。
- 封面仅把文件 I/O 移出主线程，图像解码仍由 iced 在渲染侧完成，超大封面首次
  显示仍可能有一次卡顿。
- `GameDraft` 不再位于 UI crate，界面代码必须通过 `application` 获取
  `Game` 等类型。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo test --workspace`：285 passed，0 failed，2 ignored。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- 真实数据库迁移：复制现有 v2 数据目录后启动，schema 升级到 3，原有游戏与封面
  保留，`playtime_seconds` 初始为 0；手工写入 11352 秒后再运行应用 8 秒，
  日志无 panic / ERROR，数值未被改写。
- 界面走查受限：本机为 GNOME Wayland，无法截取 iced 窗口，文案与布局只做了
  静态检查，未做像素级人工走查。

## v0.5.0 — 界面多语言：简体中文与 English（2026-10-28）

### 完成内容

- 新增内建 i18n 模块 `crates/ui/src/i18n`，语言目录为 `crates/ui/locales/zh-CN.json` 与
  `en-US.json`（各 271 个 key），编译期以 `include_str!` 嵌入二进制，未引入任何新依赖。
- 设置页「外观」卡片新增「语言」行，可选 跟随系统 / 简体中文 / English，持久化到 SQLite
  设置项 `ui_language`，切换即时生效并保存；默认「跟随系统」，按
  `LANGUAGE → LC_ALL → LC_MESSAGES → LANG` 探测（`zh*` → 简体中文，其余 → English）。
- 全部界面文案迁入 key：侧边栏与壳层、游戏库、游戏详情、Runner 管理、日志、设置、
  删除确认弹窗、文件对话框、下拉选项（架构 / 后端 / Gamescope / 显示器 / Override 模式）、
  时间文案与错误横幅。
- 应用层错误 23 个变体本地化（`i18n::errors::app_error`）；表单校验错误改为结构化
  `DraftError { field, kind }` 按当前语言渲染。环境变量名 / DLL 名无效的错误只携带违规名
  本身，不再把 `LaunchError` 的英文原文塞进已翻译的中文提示里。
- 复数形式：游戏库副标题与 Runner 占用数以 `*.one` / `*.other` 双 key 表达。
- 新增守卫测试：两种目录的 key 集合、非空值与占位符集合必须一致；扫描 `crates/ui/src`
  中 `tr` / `tr_args` / `tr_plural` / `tr_plural_args` 的字面量 key，确认每个 key 在两种
  目录中都存在（`keys_used_in_source_exist`）。
- 新增 `docs/i18n.md` 维护指南，说明目录格式、命名空间、新增语言步骤、回退规则与翻译范围。

### 与规格的偏差

- 底层技术错误原文（Wine、sqlite、IO）与日志正文不翻译，只翻译分类标签，与规格一致。
- 复数只支持 one/other 两种形式；需要更多复数形式的语言（如俄语、阿拉伯语）要先扩展该机制。
- 设置页环境变量输入框的 `KEY` / `VALUE` 占位符保留为技术性词元，未进目录。
- 兼容层卡片 DXVK 目录的选择按钮复用 `detail.choose_dxvk_dir`，中文由「选择目录」变为
  「选择 DXVK 目录」。
- 英文标签普遍长于中文，侧边栏页脚与路径行（`Wrapping::None` + 56 字符中段截断）在窄窗口
  下有截断风险，本轮只做静态检查，两种主题下的英文/中文人工走查尚未执行。

### 验证

- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：270 passed，0 failed，2 ignored。
- `cargo test -p ui`：67 passed，含目录一致性与源码 key 扫描守卫。
- 目录对齐检查：zh-CN 与 en-US 各 271 个 key，key 集合相同、均按字母序排列、占位符集合一致。
- `crates/ui/src/views/settings.rs` 中已无中文字面量（`grep -nP '[\x{4e00}-\x{9fff}]'` 无匹配）。

## v0.4.1 — 修复中文标签渲染为空（2026-10-27）

### 完成内容

- 修复 v0.4.0 界面中部分文字完全不显示的问题：侧边栏当前选中项、游戏卡片的
  “启动/设置”、游戏详情页的“启动游戏/删除游戏/保存”、游戏库右上角“新建游戏”
  以及删除确认弹窗的按钮都是空白，看起来像被背景色盖住。
- 根因是字重而非配色：`theme::font()` 请求 `Weight::Semibold`，而本机
  Noto Sans CJK SC 只有 Regular 与 Bold 两个字面（`fc-scan` 确认）。iced 在该
  字族内找不到对应字面后回退到不含中文字形的字面，中文不产生任何像素；
  使用 Normal 的未选中导航项与使用 Bold 的页面大标题因此正常。
- `theme::font()` 现在把请求字重吸附到真实存在的字面：Semibold/Bold/ExtraBold/Black
  → Bold，Thin/Light/Normal/Medium → Normal，调用点无需改动。
- 新增单元测试 `font_weights_snap_to_the_shipped_faces` 锁定该映射。

### 与规格的偏差

- 按钮与选中导航项的实际观感为 Bold，设计稿期望的是 Semibold 中等字重；要获得
  真正的 Semibold 需要安装带中间字重的中文字体（如 Noto Sans SC 可变字体）并
  放开映射。
- 本版只改字重解析，未调整任何颜色令牌、布局或组件行为。

### 验证

- 改用像素级验证（v0.4.0 仅做取色验证，无法发现该问题）：以 X11 截图加 XTEST
  合成点击驱动真实窗口，用两个独立检测器扫描每个填充控件内部 —— 一个按令牌色
  定位 pill，一个与主题无关（纯色块占比 + 内部颜色数与墨迹比例）。
- 修复前的 v0.4.0 release 二进制命中 3 个空 pill（右上角新建游戏、空状态添加
  按钮、侧边栏选中项，ink 0.000）；修复后的 v0.4.1 release 二进制在游戏库、
  游戏详情、Runner 管理、设置、日志与删除确认弹窗、浅色与深色下均为 0 个空
  pill（主按钮 ink 0.26/151 色，卡片启动 0.27/85，侧边栏选中项 0.09/152，
  确认删除按钮内部 161 种颜色）。
- 主题段切换与导航点击在实窗口中验证，浅色/深色均正常。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：246 passed，0 failed，2 ignored。
- `cargo build --release`：通过。

## v0.4.0 — 现代界面与自定义封面（2026-10-27）

### 完成内容

- 界面从单一黑蓝配色改为完整的令牌体系：浅色（Soft Studio）为默认主题，
  深色（D1 Soft Indigo）可在设置页顶部一键切换，无需重启。
- 新增 `crates/ui/src/theme.rs`（画布、侧边栏、卡片、边框、文本、主色、
  成功/警告/危险、日志面板等 24 项颜色令牌与字体辅助）与
  `crates/ui/src/styles.rs`（按钮、徽标、输入框、下拉、开关、滚动条、
  卡片等样式函数），所有视图统一从这里取样式。
- 新增通用组件层 `crates/ui/src/widgets/`：布局（页头、导航项、分节卡片、
  字段、空状态）、徽标、按钮与开关、封面、确认弹窗；旧的 `confirm.rs`
  与过渡用的 `section()` 已删除。
- 根布局改为左侧边栏（品牌标记、导航、底部会话与默认 runner 摘要）+
  顶部标题栏 + 内容画布；窗口宽度参与响应式计算，游戏库卡片自动换列。
- 五个页面全部重做：游戏库（英雄区统计 + 卡片网格 + 状态徽标）、
  游戏详情（六张分节卡片 + 260px 概览栏）、Runner 管理、设置、日志
  （深色等宽面板、横向滚动、错误提示与空状态）。
- 数据库 schema 升到 v2：`games` 新增 `cover_path`，v1 数据库首次打开时
  在事务内 `ALTER TABLE` 并回写版本号，失败自动回滚。
- 支持手动封面：添加/编辑游戏时可“选择封面”，图片校验格式
  （png/jpg/jpeg/webp/bmp）与边长（≤8192）后原子导入
  `~/.local/share/gamehub/covers/<id>.<ext>`；“恢复默认封面”清空字段并
  删除文件，回退到按游戏 ID 着色的渐变占位封面。
- 封面导入使用临时文件 + `.bak` 备份：数据库写入失败时还原旧封面，
  成功时清理备份与旧文件；删除游戏时一并移除封面与
  `logs/games/<id>.log`。
- 主题即时保存（`ui_theme`），写入串行化并在失败后重试最新值；底部批量
  保存排除该键，避免覆盖。草稿保存带 revision，过期的异步结果不会覆盖
  更新后的编辑内容。

### 与规格的偏差

- 自定义封面沿用封面框尺寸（卡片 56×76、概览 148×197）居中裁切，iced 0.13 下无法
  圆角裁剪，显示为直角；默认渐变封面仍为圆角。
- 依赖改为 iced `image-without-codecs` + 工作区 `image`（png/jpeg/webp/bmp），
  避免 iced `image` feature 强制拉入全部解码器。
- 弹窗打开时滚轮仍会作用于底层页面；删除封面时未校验路径是否位于 `covers`
  目录内。两者已补入 README 已知限制。
- 选封面、切主题、确认删除等交互路径由单元/集成测试与代码审查验证，
  未做自动化点击测试。

### 验证

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：245 passed，2 ignored（新增封面生命周期、
  schema 迁移、主题令牌与草稿并发等测试）。
- `cargo build --release`：通过，产物 `target/release/gamehub`。
- 在拷贝的 v1 数据库上验证迁移：升级至 version 2、`cover_path` 就位、
  原记录完好（真实数据库未改动）。
- 窗口截图取色验证浅色（#F4F5FB/#FFFFFF/#5B5BD6）与深色
  （#10121A/#151822/#6060E0）调色板及渐变占位封面渲染。

## v0.3.3 — Wayland 下真正应用所选显示器（2026-10-27）

### 完成内容

- 修复 GNOME Wayland 下选择显示器无效的问题。Mutter 会把新窗口（含全屏）
  固定放到首个输出，忽略 primary、指针位置和全屏输出请求。
- 选择显示器且存在 XWayland 时，Wine/Proton 改为 X11 模式
  （`WINE_GRAPHICS_DRIVER=x11`、`PROTON_ENABLE_WAYLAND=0`、
  `PROTON_USE_WAYLAND=0`），并在启动前通过 Mutter DisplayConfig 临时把
  目标显示器设为 primary；Wine X11 窗口据此落到目标显示器。
- 启动约 5 秒后自动恢复原 primary，游戏退出或启动失败时再次兜底恢复。
- X11 会话使用 `xrandr --primary`；显示器扫描新增 primary 与坐标解析。
- 无 XWayland 时保留 Wayland 变量回退，并在界面提示无法保证生效。

### 与规格的偏差

- 使用 gamescope 时仍走原有 `-O` 路径，未验证其嵌套 Wayland 的输出选择。
- 未对原生 GUI 做自动化点击验证；真实 primary 切换/恢复与 Wine X11 放置
  由手工测试和 ignored 测试覆盖。

### 验证

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：211 passed，2 ignored。
- 真实 Mutter primary 切换/恢复测试通过；GE-Proton + Wine notepad 在
  HDMI-1/eDP-1 间按 primary 正确定位，启动后恢复 primary 仍保持目标屏。

## v0.3.2 — 通过 wineserver 结束 Wine/Proton 游戏（2026-10-27）

### 完成内容

- 修复点击“结束”无法终止游戏的问题。Wine/Proton 的游戏进程由 `wineserver`
  托管并脱离启动器进程组，之前只结束进程组不会命中游戏本体。
- 启动时记录 runner 与 `WINEPREFIX`；结束时先执行对应 runner 的
  `wineserver -k`，再对启动器进程组发送 `SIGTERM`/`SIGKILL` 兜底。
- Proton 使用 `<prefix>/pfx`，系统 Wine 使用 prefix 本身；`wineserver` 按
  系统 Wine 同目录、Proton 的 `files/bin`、`dist/bin`、`bin` 布局查找。
- 两种终止机制任一成功即视为结束成功，避免重复误报。

### 与规格的偏差

- `wineserver -k` 会结束该 prefix 下的所有 Wine 进程；应用自身已禁止同一
  prefix 的并发游戏。
- 未对原生 GUI 做自动化点击验证；结束流程由集成测试覆盖。

### 验证

- 新增“脱离进程组的游戏进程可被结束”的集成测试。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：209 passed，1 ignored。

## v0.3.1 — Wayland 下应用所选显示器（2026-10-27）

### 完成内容

- 修复 Wayland 下游戏详情中选择的显示器未传给 Wine/Proton 的问题；此前只有
  X11 注入 `__GL_SYNC_DISPLAY_DEVICE`。
- 未使用 gamescope 且已选择显示器时：
  - X11：继续注入 `__GL_SYNC_DISPLAY_DEVICE`。
  - Wine：注入 `WAYLANDDRV_PRIMARY_MONITOR`。
  - Proton：注入 `PROTON_ENABLE_WAYLAND=1`、`PROTON_WAYLAND_MONITOR` 和
    `WAYLANDDRV_PRIMARY_MONITOR`。
- 使用 gamescope 时仍通过 `-O <显示器>` 传递，不重复注入环境变量。
- 游戏详情的显示区新增 Wayland 环境变量说明。

### 与规格的偏差

- 系统 Wine 需要已启用 Wayland 驱动才会响应显示器变量；仍走 XWayland 的 Wine
  不受影响。
- 未对原生 GUI 做自动化点击验证。

### 验证

- 本机 GE-Proton11-7 的 `proton` 脚本确认读取 `PROTON_ENABLE_WAYLAND`，并将
  `PROTON_WAYLAND_MONITOR` 映射为 `WAYLANDDRV_PRIMARY_MONITOR`。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：205 passed，1 ignored。

## v0.3.0 — 结束运行中的游戏与显示器自动扫描（2026-10-27）

### 完成内容

- 游戏运行后，游戏卡片与详情页的“启动”按钮变为红色“结束”按钮；结束中
  显示“结束中”。点击后向游戏进程组发送 `SIGTERM`，宽限期内未退出则发送
  `SIGKILL`，可一并结束 Wine/gamescope 派生的子进程。
- 主动结束游戏不再误报“游戏异常退出”；`LaunchOutcome` 新增 `stopped` 标记，
  UI 正常回到未运行状态。
- 游戏详情的显示器设置改为自动扫描并下拉选择，不再要求手输输出名；扫描顺序
  为 `xrandr` → `wayland-info` → `/sys/class/drm`，兼容 X11、Wayland 及
  无 XWayland 环境。
- 打开游戏详情时自动刷新显示器列表，并提供“刷新”按钮；选项展示名称、分辨率
  与刷新率。

### 与规格的偏差

- 未对原生 GUI 做自动化点击验证；启动/结束流程由应用层集成测试覆盖，
  显示器扫描在本机 Wayland 会话下确认可枚举 HDMI-1 与 eDP-1。

### 验证

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：202 passed，1 ignored。

## v0.2.0 — 游戏卡片操作、启动日志与兼容层设置（2026-10-27）

### 完成内容

- Library 卡片右侧新增启动/设置/日志按钮；启动后先显示“启动中”，spawn
  成功后显示“游玩”，退出后恢复。设置进入游戏详情，日志进入该游戏独立日志页。
- 日志页读取最近一次启动的日志尾部 1000 行，页面打开时每秒刷新；启动时
  轮转旧日志并写入时间戳与完整命令行。
- 设置页新增兼容层增强区：DXVK-NVAPI 自动安装与本地源目录、Esync、Fsync、
  MangoHud 开关；Esync/Fsync 默认开启，NVAPI 源为空时启动给出明确错误。
- Wine 通过 `WINEESYNC`/`WINEFSYNC` 与 prefix 内 DLL/`DllOverrides` 注入
  NVAPI；Proton 仅使用环境变量。MangoHud 优先使用 gamescope `--mangoapp`，
  否则以 `mangohud --dlsym` 包裹启动命令，未安装时拒绝启动。
- 启动编排改为 `LaunchEvent`（Started/Finished/Failed）事件流，UI 不再轮询
  进程状态。

### 与规格的偏差

- 本机无 mangohud、gamescope 与真实 DXVK-NVAPI 资源，相关启动路径由伪造
  可执行文件与单元测试覆盖。
- UI 已完成 XWayland 冒烟启动，但当前环境无法目视核对像素级布局。

### 验证

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：198 passed，1 ignored。
- 隔离 `XDG_DATA_HOME` 下以 XWayland 启动 `gamehub`，窗口创建成功且无 panic。

## v0.1.1 — 结构重构：应用服务层与模块拆分（2026-10-27）

### 完成内容

- 新增 `crates/application`（P0/P2）：把 `ui/tasks.rs` 的编排逻辑下沉为
  `AppService`，统一 `AppError`；`ui` 不再直接依赖 storage/prefix/display/
  runner-wine/launch。
- UI 拆分（P1）：`ui/app.rs` → `app/{mod,message,update,view}.rs`；
  `ui/widgets.rs` → `widgets/{mod,choices,confirm}.rs`。
- storage 拆分（P3）：CRUD 按实体移到 `storage/repos/{games,runners,settings}.rs`，
  `lib.rs` 只保留连接与 `ensure_app_dirs`。
- 日志下沉（P6）：`init_logging` 从 `ui` 移到 `launcher-core::logging`。
- 旧 `ui/src/tasks.rs` 的 12 个编排测试迁移为
  `crates/application/tests/orchestration.rs`。

### 与规格的偏差

- `docs/specs/08/09/10` 是历史实施记录，未回写新文件布局；架构以
  `docs/specs/00-overview.md` 为准。
- 未执行 P4（launch 重命名）、P5（为每个 crate 建 tests/）、P8（xtask）。
- 产品行为不变，仅调整 crate 边界与模块布局。

### 验证

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过。
- `cargo test --workspace`：156 passed，1 ignored（application 12、storage 18、
  ui 14）。

## P10 — 交付验收

### 完成内容

- 按 `docs/specs/11-acceptance.md` 执行 A1–A10，新增
  `docs/acceptance.md` 记录环境、自动/手工结果、未执行项与偏差。
- README 增加验收记录与限制说明；验收计划勾选完成项并追加实施记录。
- 本阶段只改文档，不修改产品代码；验收对象为 `bf0ecdd`。

### 与规格的偏差

- 系统未安装 `strace`：下载 6.13 `.deb` 解压到 `/tmp` 执行，未安装进系统；
  为识别子进程额外用 `-e trace=connect,sendto,execve` 复跑一次，两次均无
  `AF_INET/AF_INET6`。
- 自动化通过 XWayland + XTEST/`import -window` 完成；`XDG_SESSION_TYPE` 仍为
  `wayland`，显示配置按 Wayland 分支验证。
- 本机无 gamescope：A9 Step 2 未执行，仅有 10 项参数级单测。
- 本机无 X11 会话：X11 输出下拉与非 gamescope 分辨率未手工验证。
- 高 DPI 未验证（缩放因子 1）；900×600 最小尺寸正常。
- rfd 在隔离环境无 session bus，回退 zenity；正常桌面使用
  xdg-desktop-portal。
- 共享 Prefix 与同 Prefix 并发的第二个游戏由 DB 预置，验证后删除。
- 游戏删除只删 DB 行，不删除 Prefix 目录，符合 A3 Step 9 的“可另行删除”。

### 验证

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`：
  通过；`cargo test --workspace`：156 passed，1 ignored；`cargo test -p ui`：
  26 passed；debug/release 构建通过，release 二进制 14 MB。
- A2：`initializes_with_real_system_wine` 通过，耗时 10.22 s。
- A3：Wine 10.0 探测/默认 Runner、游戏导入与重启保留、Prefix 初始化、显示
  配置、环境变量与 DLL override、`WINEDLLOVERRIDES` 拒绝、启动 notepad
  显示运行中、子进程环境、关闭后 `last_played_at` 更新、默认/被引用 Runner
  与共享 Prefix 删除保护均通过。
- A4：导入 GE-Proton11-7，`STEAM_COMPAT_DATA_PATH/pfx/system.reg` 生成，
  notepad 启动/关闭与最近游玩更新通过。
- A5：`net.log` 462 行，`AF_INET/AF_INET6` 为 0，输出 `offline ok`；仅
  X11、系统 D-Bus、nscd、CUPS 等 `AF_UNIX`。
- A6：`ldd` 无 gtk/qt/webkit；仅 libc/libm/libgcc/ld-linux。
- A7：中文标签无方块（Noto Sans CJK SC），900×600 布局与滚动正常。
- A9：`cargo test -p display gamescope` 10 passed。

## P9 — 配置与启动 UI

### 完成内容

- GameDetail 配置区补全：环境变量、DLL Override、启动参数编辑器和显示
  设置（分辨率、刷新率、gamescope 模式）；所有编辑只改草稿，“保存”统一
  写库。
- 启动流程：保存后的游戏可启动；启动前校验未保存草稿、游戏/默认 Runner、
  同 Prefix 并发、DB 与全局环境变量读取；`LaunchRequest` 汇总游戏、Runner、
  session、全局/继承环境、Steam compat、默认 Prefix 根与 gamescope 路径。
- 运行状态：启动成功后记录 `running`（游戏 → Prefix 路径）并实时显示“运行
  中”；运行期间禁用启动、删除游戏和删除 Prefix；进程退出后清理状态并写入
  `last_played_at`，异常退出显示退出码。
- 启动前置提示：Prefix 未初始化、gamescope 未检测到、新建草稿未保存时在
  详情页给出明确提示；Forced/Auto 模式分别说明拒绝或降级原因。
- 重活（构建计划、spawn、等待进程、写库）留在 `tasks.rs`，`App::update`
  只做校验与状态转移。

### 与规格的偏差

- 本机为 GNOME Wayland，自动化测试通过 `WINIT_UNIX_BACKEND=x11` 让 UI 跑在
  XWayland 上以便 XTEST 点击；会话仍按 `XDG_SESSION_TYPE` 识别为 Wayland，
  显示配置按 Wayland 分支验证。
- 本机无 gamescope：只验证了 Auto/Forced 在未检测到 gamescope 时的提示与
  启动拒绝，未验证真实 gamescope 启动参数拼接。
- 同 Prefix 并发校验需要两个已保存游戏，手工验证时用 DB 预置第二个游戏，
  验证后删除。
- U15 的“删除中禁用创建/启动”只在 GameDetail 实现（本阶段没有其它创建或
  启动入口）；Library 未新增禁用逻辑。
- `last_played_at` 在进程异常退出时同样更新，便于“最近游玩”排序反映最后
  一次启动。
- 启动等待在 `spawn_blocking` 之外的异步任务中执行，关闭窗口后子进程继续
  运行（未做进程组回收，与规格一致）。

### 验证

- `cargo test -p ui`：26 passed；`cargo test --workspace`：156 passed，
  1 ignored。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo build --workspace`：通过。
- UI 手工验证（XWayland + XTEST 真实点击、`import -window` 精确截图）：
  - 系统 Wine 10.0 探测为默认 Runner；通过文件对话框选择 notepad.exe 新建
    并保存游戏，Prefix 约 12 秒初始化完成，状态实时刷新；
  - 显示设置输入 Wayland 显示器分辨率并保存；gamescope 未安装时 Auto 提
    示不会生效、Forced 提示启动会被拒绝；
  - 环境变量 `DXVK_HUD=fps`、DLL Override `d3d11`（native,builtin）、启动
    参数 `/c exit 7` 保存后写入 DB，重启后保留；手写 `WINEDLLOVERRIDES`
    保存被拒绝；
  - 启动 notepad 后显示“运行中”，启动/删除按钮禁用，删除 Prefix 被拒绝，
    子进程环境包含全局与游戏环境变量，`last_played_at` 更新；
  - 用 cmd.exe 以退出码 7 结束，界面显示“游戏异常退出（退出码 Some(7)）”，
    状态恢复可再次启动；
  - 同 Prefix 的第二个游戏启动被拒绝（“同一 Prefix 已有游戏在运行”），共
    享 Prefix 删除被拒绝；单游戏 Prefix 删除会清空引用并移除目录；
  - 关闭 gameHub 窗口后被启动的 notepad 仍在运行，符合规格。

## P8 — Runner / Prefix / Settings UI

### 完成内容

- Runner 管理：重新探测系统 Wine（覆盖已有记录按 binary_path 更新）、导入
  Proton 目录（`validate_proton_dir` 校验并持久化）、设置/取消全局默认
  Runner；列表按名称排序。
- Prefix：详情页展示路径与状态（未创建/目录存在未初始化/已初始化/架构不
  匹配）；选择/清除 Prefix 目录；后台创建/初始化；DXVK 目录选择与安装。
- 设置：默认 Runner、Prefix 根目录、全局环境变量行编辑（增删改）与保存。
- 删除：Game/Runner/Prefix 确认条；默认 Runner、被游戏引用的 Runner、
  共享 Prefix、运行中的游戏/Prefix 均拒绝删除；单游戏 Prefix 删除会清空
  DB 引用并移除目录。
- 所有重活（探测、DB 写入、prefix 初始化、DXVK 安装、引用检查）留在
  `tasks.rs`，`App::update` 只做状态转移；Runner/Prefix 变化后自动刷新
  状态。

### 与规格的偏差

- `WineDetected` 使用 `sort_by_key` 以满足 clippy `unnecessary_sort_by`。
- `iced::Task::perform` 的完成闭包是 `Fn`，`DeleteFinished` 使用
  `target.clone()`。
- 手工验证发现长 Prefix 路径会把确认条按钮挤成竖直窄条，确认条改为
  Fill 宽度文本 + `WordOrGlyph` 换行。
- 规格 U11 预期 `cargo test -p ui` 24+，实际为 23（P7 基线 16 + 规格 6 +
  额外运行中 Prefix 保护 1）。
- Prefix 创建成功后只更新 draft，仍需点击“保存”写入 DB；保持既有草稿
  语义。
- 本机无真实 Proton：用合成 Proton 目录验证导入、重启持久化与 `pfx/`
  路径；运行中删除保护在 P9 接入 running map 后才能手工复核。

### 验证

- `cargo test -p ui`：23 passed；`cargo test --workspace`：153 passed，
  1 ignored。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo build --workspace`：通过。
- UI 手工验证（XWayland + XTEST 真实点击、桌面 portal 截图）：
  - 断网探测得到 `wine-10.0 (Debian 10.0~repack-6)`；默认 Runner 设置/取消
    重启后保留；
  - 合成 Proton `GE-Proton9-99` 导入显示“已验证”，重启后仍在；
  - 已保存游戏切换 Runner/Prefix 并保存；Wine prefix 约 11 秒初始化成功，
    状态实时刷新；合成 Proton 在 `pfx/system.reg` 生成 registry；
  - DXVK 3.1.1 安装后 DLL 出现在 `system32`/`syswow64`，`user.reg` 写入
    `"d3d11"="native,builtin"`；
  - 全局环境变量 `DXVK_HUD` 保存后重启保留；
  - 删除默认/被引用 Runner、单游戏 Prefix 的提示与结果符合预期；
  - 长路径删除确认条换行正常，取消后状态不变。

## P7 — UI 骨架

### 完成内容

- ui 拆分 lib/bin，`ui::run()` 统一初始化应用目录、日志（stderr + 每日文件）、
  DB 与 Iced；`main.rs` 只调用 `ui::run()`。
- `App` 持有规格定义的全部状态；启动通过 `Arc<Mutex<Db>>` + `spawn_blocking`
  异步加载 games/runners/settings/session/xrandr/gamescope；DB 打开失败进入
  fatal 页。
- Library 列表（空态、运行中标记、最近游玩）、基础 GameDetail（名称、可执行
  文件、工作目录、架构、保存）、只读 RunnerManager 与 Settings。
- `GameDraft` 校验：名称非空、可执行文件为绝对路径、工作目录/prefix 为绝对
  路径、分辨率成对且 >0、刷新率 >0、环境变量键合法且禁止手写
  `WINEDLLOVERRIDES`、DLL 名去 `.dll` 归一化/去重/拒绝非法字符、参数去除
  空行。
- rfd 0.17 异步文件对话框（`spawn_blocking`）：选择 `.exe` 后创建草稿；
  取消或对话框失败返回 `None`，状态不变且不报错。保存任务在阻塞线程重新
  校验 exe 与工作目录后写入 DB。
- 默认字体设为 `Noto Sans CJK SC`。

### 与规格的偏差

- 提交顺序：U1–U5 按规格逐任务提交会让 crate 在中间状态无法编译（U1 声明
  了 U2–U5 才创建的模块），因此 U2 单独提交，U1/U3/U4/U5 在规格指定的检查点
  合并提交，U5 的测试随该提交加入。
- `sort_games` 使用 `sort_by_key` 以满足 clippy `unnecessary_sort_by`，排序
  语义不变。
- 首次实际运行发现 Iced 0.13 默认字体在本机不回落 CJK，中文显示为方块；
  按 01-foundation 的应急方案将 `default_font` 设为系统已安装的
  `Noto Sans CJK SC`，仍未打包字体。
- 额外增加 `saved_game_survives_reopen` 测试（任务测试共 16 个而非 15 个），
  用文件数据库重开验证重启持久化。

### 验证

- `cargo test -p ui`：16 passed（12 draft + 4 tasks）。
- `cargo test --workspace`：146 passed，1 ignored。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo build --workspace`：通过。
- UI 手工验证（XWayland + XTEST 真实点击，桌面 portal 截图核对）：
  - 启动后立即显示 Library，空态提示正确；
  - “新建游戏”打开 rfd 对话框（本机首选 Wayland portal；无会话总线时 rfd
    回退 zenity），选择 exe 后进入“新建游戏”，保存写入 DB；
  - 详情页编辑名称并保存，DB 与重启后的 Library 均更新；
  - Library → GameDetail → Settings → RunnerManager → Library 导航正常；
  - 重启后两张卡片仍在；
  - `XDG_DATA_HOME=/dev/null/gh` 时显示 fatal 页及数据库错误详情。
  - Wayland portal 对话框内部按钮无法用 X11 合成事件点击；取消路径
    （rfd 返回 `None` → 状态不变、无错误）由代码路径确认。

## P6 — 启动管线

### 完成内容

- runner 解析：游戏级 `runner_id` 优先且必须存在；否则使用全局默认，
  未配置默认报 `NoRunnerSelected`，绝不自动挑选唯一 runner。
- `WINEDLLOVERRIDES` 只由结构化 override 生成（`disabled`/`n`/`b`/`n,b`，
  输入顺序、分号分隔、空白名称跳过）；继承环境中的旧值被删除，
  全局或游戏级手动设置直接报 `DllOverrideConflict`。
- 环境优先级：继承 → runner base env → global → game → X11 monitor →
  结构化 DLL override；键名与 DLL 名均校验。
- `__GL_SYNC_DISPLAY_DEVICE` 仅在 X11 且 gamescope 未生效时注入。
- 启动前校验：exe 是文件、runner 可执行、working_dir 存在、prefix 已初始化
  且架构匹配、Forced gamescope 必须有二进制、env/DLL 配置合法。
  `build_plan` 内部先调用 `validate`。
- `build_plan` 经 `WineContext` 构造 runner 命令（Wine 直接执行 exe，
  Proton 加 `run`），必要时以 gamescope 参数包裹，cwd 回退到 exe 父目录。
- `spawn_plan`：`env_clear` + 合成环境、stdin/stdout null、stderr 继承、
  `kill_on_drop(false)`，spawn 失败携带程序路径。

### 与规格的偏差

- L1 测试片段缺少 `std::path::PathBuf` 导入，已补上；
  `default_runner_is_used_when_game_has_none` 的切片改用
  `std::slice::from_ref` 以通过 clippy。
- L4 计划中的 Proton 测试用 `fs::create_dir("compat/pfx")`，但父目录
  `compat/` 尚不存在，改为 `create_dir_all`。
- L4 两个测试的 `let mut request` 并无修改，去掉 `mut`（clippy）。
- `validate` 的 working_dir 检查使用 `filter` 形式，避免 clippy
  `collapsible_if`；行为与计划一致。
- 测试计数与计划一致：L1=5、L2=8、L3=17、L4=29、L5=32。

### 验证

- 本机未安装 gamescope，gamescope 包装路径由伪造可执行文件覆盖，
  `Forced` 且无二进制的拒绝路径有独立测试。
- `spawn::tests` 连续运行 10 次全部通过（3 passed / 次）。
- `cargo test -p launch`：32 passed；
  `cargo test --workspace`：130 passed，1 ignored。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo build --workspace`：通过。

## P5 — display

### 完成内容

- 会话检测：`XDG_SESSION_TYPE` 精确匹配 `x11`/`wayland`，缺失或其他值
  （包括大小写不匹配）一律为 Unknown。
- `xrandr --query` 解析：仅把第二个 token 为 `connected`/`disconnected`
  的行当作输出头；带 `+offset` 的几何 token 判定为活动输出；模式行以 `*`
  标记当前模式，小数刷新率四舍五入为整数（143.88 → 144）；断开与非活动
  输出仍保留在结果中。
- PATH 查找：按 `split_paths` 顺序返回第一个可执行普通文件，跳过目录与
  无执行位文件；`gamescope_path` 用 `OnceLock` 缓存一次 PATH 扫描。
- gamescope 真值表：Off 永不启用；Auto 仅在 Wayland 且二进制可用时启用；
  Forced 始终启用。参数顺序固定为 `-W/-H`、`-r`、`-f`、`-O <monitor>`
  （仅 Wayland）、`--backend wayland|x11`（Unknown 省略），始终以 `--` 结尾。

### 与规格的偏差

- 无测试计数偏差：D1=2、D2=6、D3=9、D4=16，与计划一致。
- D3 测试中 clippy 报 `useless_conversion`，将 `PathBuf::from(gamescope)`
  改为直接比较，并删除随之未使用的 `PathBuf` 导入。
- 本机仅有 HDMI-1 一个输出（无 eDP-1），D2 使用规格给定的典型笔记本样例
  覆盖 eDP-1 活跃输出与 143.88 → 144 的刷新率取整。

### 验证

- 本机 `xrandr --query`：`HDMI-1 connected primary 1920x1080+0+0`，
  当前模式 `1920x1080 143.88*+`，screen 当前 3840x1080。
- 本机未安装 gamescope（`command -v gamescope` 无结果）；
  `find_in_path`/`gamescope_path` 由临时目录中的伪造可执行文件覆盖，
  Auto 的可用性分支用 `available` 参数覆盖。
- `cargo test -p display`：16 passed。
- `cargo test --workspace`：98 passed，1 ignored。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo build --workspace`：通过。

## P4 — prefix 与 DXVK

### 完成内容

- `PrefixPlan` 与 `plan_for_game`：Wine 使用 `game.arch`，Proton 固定 win64；
  `prefix_path` 显式值优先，否则 `<default_root>/<game_id>`。
- `windows_root` 封装布局差异：Wine 在 prefix 根目录，Proton 在 `pfx/`；
  `detect_arch` 读取 `system.reg` 前 8 KiB 的 `#arch=` 标记。
- `PrefixStatus`：Missing / NotInitialized / Initialized / ArchMismatch。
- `initialize_with_timeout`：已初始化返回 `Ok(false)`；arch 不匹配在 spawn 前报错；
  `wineboot -u` 带完整 Wine/Proton 环境、piped stderr、超时 `kill_on_drop`、
  stderr 4 KiB 截断；wineboot 后必须以 `system.reg` 存在确认成功。
- DXVK：校验用户源目录（x64/x32），win64 映射 x64→system32、x32→syswow64，
  win32 映射 x32→system32；d3d11/dxgi 必需，d3d10core/d3d9 可选；
  复制后对每个唯一 DLL 执行一次 `reg add ... DllOverrides /d native,builtin /f`
  （Proton 走 `proton run reg add`），超时 30 s，错误携带 DLL 名与截断 stderr。
- `delete_prefix`：幂等；仅删除含 `system.reg` 或 `pfx/system.reg` 的目录，
  否则 `RefusingToDeleteNonPrefix`。

### 与规格的偏差

- 计划中的测试计数与实际测试模块定义不一致（PF1 10 而非 8，之后依次为
  14/15/20/23/25+1 ignored，而非 12/13/18/21/23+1）。
- `test_support` 额外提供 `EXEC_LOCK`（tokio Mutex），串行化"写脚本并 spawn"
  的测试，避免 P3 已复现的 ETXTBSY 竞态；prefix 全量测试 30 次运行 0 失败。
- `detect_arch` 的 `File::open` 去掉不必要的 `mut`（clippy）。
- PF5 测试删除未使用的 `PathBuf` 导入（clippy）。

### 验证

- 真实 wineboot：14.23 s 完成 win64 prefix 初始化（Wine 10.0）。
- DXVK：本机无 DXVK，也未引入下载；由伪造源目录与假 runner 覆盖。
- `cargo test -p prefix`：25 passed，1 ignored；
  `--ignored` 真实 Wine 测试通过。
- `cargo test --workspace`：82 passed，1 ignored。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all --check`、
  `cargo build --workspace`：通过。

## P3 — runner-wine

### 完成内容

- `WineContext`：Wine 环境恰好是 `WINEPREFIX`/`WINEARCH`/`WINEDEBUG=-all`，
  Proton 环境恰好是 `STEAM_COMPAT_DATA_PATH`/`STEAM_COMPAT_CLIENT_INSTALL_PATH`/
  `PROTONPATH`；两种 runner 的环境变量互不泄漏。
- 命令构造：Wine 为 `(wine, [exe, args...])`，Proton 为
  `(proton, ["run", exe, args...])`，纯函数、不触碰文件系统。
- `WineContext::spawn` 以 base env + extra env 构造 tokio 子进程，
  `kill_on_drop(false)`，失败映射为 `RunnerError::Spawn`。
- 系统 Wine 探测：PATH 条目 + `/usr/bin/wine{,-stable,-development}` 候选项，
  按 canonical path 去重；逐个执行 `--version`，默认 5s 超时，失败仅告警不 panic。
- Proton 目录校验：要求 `root/proton` 存在且可执行（规范化路径后检查），
  版本优先读 `version` 文件，否则用目录名；导入 runner 固定 win64 且 verified。
- `verify_runner` 在启动前检查二进制存在性与可执行位，分别报
  `MissingBinary`/`NotExecutable`。
- `RunnerError` 覆盖目录/二进制校验、规范化、探测失败/超时、spawn 与 IO。

### 与规格的偏差

- R1 计划预计 7 个测试，实际测试模块定义 6 个（计划计数笔误），后续数量与
  计划一致（R2=13、R3=16、R4=17）。
- `verify_runner` 实现覆盖 `NotExecutable`，计划测试只断言 `MissingBinary`。
- 并行测试暴露 ETXTBSY 竞态（fork 继承其它测试正在写入的脚本 fd，
  复现率 5/40）；新增仅测试可见的 `tokio::sync::Mutex`（`EXEC_LOCK`）
  串行化"写脚本并 spawn"的测试，修复后 50 次全量运行 0 失败。

### 验证

- 真实环境：Wine 10.0（`wine-10.0 (Debian 10.0~repack-6)`），
  `detect_system_wine()` 探测到 1 个 runner；本机无 Proton，
  `validate_proton_dir` 由伪造目录测试覆盖。
- `cargo test -p runner-wine`：17 passed；`cargo test --workspace`：57 passed。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`、`cargo build --workspace`：通过。

## P2 — 存储

### 完成内容

- SQLite v1 schema（WAL、5s busy timeout、外键开关）、schema_version 表与
  版本检查：新库自动建表，旧版本报 `MigrationRequired`，未来版本报
  `UnsupportedSchema`。
- `Db::open` 自动创建数据库父目录；`ensure_app_dirs` 创建应用全部目录。
- Game/Runner CRUD；Game 的 JSON 列（display/env/dll/args）与 Runner
  `source_json`；列表按名称 `NOCASE` 排序。
- Settings 键值表：upsert、查询、删除、全量 BTreeMap。
- 删除前引用计数：`count_games_using_runner`、`count_games_using_prefix`。
- 可诊断错误：损坏枚举/UUID（`CorruptValue` 带列名）、损坏 JSON、
  非 UTF-8 路径（`NonUtf8Path` 带列名）、重复 runner 二进制路径。
- `runners.binary_path` 唯一约束。

### 与规格的偏差

- 为避免 S2 提交出现 `dead_code` 警告，`row_to_runner` 与
  `parse_runner_kind` 延后到 S3 才加入 `mapping.rs`；最终公开 API 与
  spec 完全一致。
- 测试模块不再重复 `use rusqlite::params`（`use super::*` 已引入）。

### 验证

- 绑定 SQLite 版本：3.54.1（libsqlite3-sys 0.39.1 bundled）。
- `cargo test -p storage`：18 passed。
- `cargo clippy -p storage --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。

## P1 — 核心类型

### 完成内容

- `launcher-core::types` 定义跨 crate 契约：`Arch`、`RunnerKind`、
  `RunnerSource`、`Runner`、`OverrideMode`、`DllOverride`、`DisplayBackend`、
  `GamescopeMode`、`DisplayConfig`、`Game`。
- 枚举统一 `snake_case` serde 表示；`Arch`/`RunnerKind` 的 `as_str` 与 serde
  输出由测试保证一致，`parse` 仅接受小写。
- `RunnerSource` 使用内部 tag：`{"kind":"system"}` /
  `{"kind":"imported","original_path":...}`。
- `DisplayConfig::default()`：Auto / None / None / None / fullscreen / Auto。
- 集成测试 `tests/contracts.rs` 锁定 JSON 字段与未知枚举拒绝行为。

### 与规格的偏差

- 无。字段名、默认值与 spec 完全一致。

### 验证

- `cargo test -p launcher-core`：22 passed（20 单测 + 2 集成）。
- `cargo clippy -p launcher-core --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。

## P0 — 基线与冒烟

### 完成内容

- 将单 package 仓库转换为 7-crate Cargo workspace：
  `launcher-core`、`storage`、`runner-wine`、`prefix`、`display`、`launch`、`ui`。
- 锁定并提交依赖：iced 0.13.1（启用 `tokio` feature）、tokio 1.54、
  rusqlite 0.41（bundled）、rfd 0.17.2、serde、thiserror 2、tracing、uuid、chrono。
- `launcher-core`：
  - `AppPaths`：`$XDG_DATA_HOME/gamehub` 或 `$HOME/.local/share/gamehub`，
    派生数据库、日志、prefix 根、Proton 兼容客户端目录。
  - `truncate_utf8`：按 UTF-8 字符边界截断，用于限制系统命令 stderr。
- `ui`：tracing 双输出（stderr + 每日滚动日志文件）；Iced 0.13 + tokio
  执行器 + 中文渲染冒烟窗口。

### 与规格的偏差

- 规格假设仓库尚无 commit；实际仓库已有初始 commit `f9f6fa6`，
  因此移动 `src/main.rs` 时直接使用 `mv`（仍不使用 `git mv`）。
- 本机 GNOME 会话禁止通过 D-Bus/ImageMagick 截屏，中文渲染的目视确认
  无法自动完成；已确认系统存在 Noto Sans CJK SC 与 WenQuanYi Zen Hei，
  且 fontconfig 对 `U+6E38`（游）解析到 WenQuanYi Zen Hei。

### 验证

- `cargo build --workspace`：通过。
- `cargo test --workspace`：8 passed（launcher-core）。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all --check`：通过。
- `cargo run -p ui`：窗口进程持续运行 6 秒，无 panic、
  无 "no reactor running"；`~/.local/share/gamehub/logs/gamehub.log.2026-10-26` 已生成。
- `cargo tree --depth 0`：iced 0.13.1、rusqlite 0.41.0、rfd 0.17.2。
