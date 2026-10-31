# P7 UI 骨架 Implementation Plan

> 历史实施计划（2026-10-27，P7）。Task U1–U6 的代码片段与验收记录保留原样；
> 与当前代码不一致的契约、文件路径和流程以文末「当前实现（v0.12.0）」为准。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 把 `ui` 从冒烟程序变成可用的应用骨架：启动加载、Library、基础 GameDetail、本地 exe 导入、草稿校验与保存、rfd 文件对话框、错误页。

**Architecture:** `ui` 同时提供 lib 与 bin。`main.rs` 只调用 `ui::run()`；`run()` 负责路径/日志/DB 与 Iced 启动。`App` 持有全部状态；DB 操作一律通过 `Arc<Mutex<Db>>` + `spawn_blocking`；视图拆成 `views/*`，纯逻辑拆成 `draft.rs`/`widgets.rs`。

**Tech Stack:** Iced 0.13.1（`Task`、tokio executor）、rfd 0.17、tokio、storage、launcher-core、chrono。

**前置条件：** P2、P3 完成；P0 的冒烟 UI 已存在。

---

## 最终状态与消息目录（09/10 会继续追加）

> **当前实现提示（v0.12.0）：** 下面的 `App` 是 P7 时的最小状态，仅作为历史起点。现在的状态由 `AppService` 取代 `db` 字段，并新增运行态、平台、显示器、组件、日志、封面、主题/语言/字体等字段（完整列表见文末「当前实现」）。`Message`、`GameDraft`、`DraftAction` 的当前定义见 `crates/ui/src/app/message.rs` 与 `application::editor`。

```rust
pub struct App {
    pub paths: AppPaths,
    pub db: Option<Arc<Mutex<Db>>>,
    pub fatal: Option<String>,
    pub screen: Screen,
    pub games: Vec<Game>,
    pub runners: Vec<Runner>,
    pub settings: SettingsState,
    pub selected_game: Option<Uuid>,
    pub draft: Option<GameDraft>,
    pub prefix_status: Option<prefix::PrefixStatus>,
    pub session: SessionType,
    pub x11_outputs: Vec<X11Output>,
    pub gamescope_path: Option<PathBuf>,
    pub pending: PendingState,
    pub running: HashMap<Uuid, PathBuf>, // game id -> prefix path
    pub error: Option<AppError>,
}
```

本阶段实现的 `Message`：`Navigate`、`SelectGame`、`NewGame`、`Loaded`、`LoadFailed`、`Draft`、`SaveDraft`、`DraftSaved`、`Pick`、`DialogResult`、`DismissError`、`Error`。

后续阶段追加：09 增加 Runner/Prefix/Settings/删除相关消息；10 增加启动与显示/环境/DLL/参数消息。新消息必须按同样的命名风格追加，不得重命名已有消息。

---

## Task U1: lib/bin 拆分、`run()`、日志与应用骨架

**Files:**
- Modify: `crates/ui/Cargo.toml`
- Modify: `crates/ui/src/main.rs`
- Create: `crates/ui/src/lib.rs`
- Create: `crates/ui/src/app.rs`
- Create: `crates/ui/src/tasks.rs`
- Create: `crates/ui/src/views/mod.rs`
- Create: `crates/ui/src/widgets.rs`

- [x] **Step 1: 修改 `Cargo.toml`**

在 `[[bin]]` 之前加：

```toml
[lib]
name = "ui"
path = "src/lib.rs"
```

- [x] **Step 2: 把 `main.rs` 缩到最小**

```rust
fn main() -> iced::Result {
    ui::run()
}
```

- [x] **Step 3: 写 `lib.rs`（含日志初始化）**

```rust
pub mod app;
pub mod dialogs;
pub mod draft;
pub mod tasks;
pub mod views;
pub mod widgets;

pub use app::{App, Message};

use launcher_core::AppPaths;
use std::io::Write;
use std::path::Path;
use storage::Db;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub fn run() -> iced::Result {
    let paths = match AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("gamehub: cannot determine data directory: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = storage::ensure_app_dirs(&paths) {
        eprintln!("gamehub: cannot create application directories: {error}");
    }

    let _log_guard = init_logging(&paths.logs_dir);
    tracing::info!(?paths, "gamehub starting");

    let db = Db::open(&paths.db_path);

    iced::application("gamehub", App::update, App::view)
        .theme(|_| iced::Theme::Dark)
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 760.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            ..Default::default()
        })
        .run_with(move || App::new(paths, db))
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

`_log_guard` 必须在 `run()` 结束前存活。

- [x] **Step 4: 写 `app.rs` 的状态与启动加载**

```rust
use crate::draft::{DraftAction, EnvRow, GameDraft};
use crate::tasks::{self, LoadedData};
use display::{SessionType, X11Output};
use launcher_core::{AppPaths, Game, Runner};
use prefix::PrefixStatus;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use storage::{Db, StorageError};
use uuid::Uuid;

pub const SETTING_GLOBAL_ENV: &str = "global_env";
pub const SETTING_DEFAULT_RUNNER_ID: &str = "default_runner_id";
pub const SETTING_PREFIX_ROOT: &str = "prefix_root";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    GameDetail,
    RunnerManager,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    pub title: String,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsState {
    pub global_env_rows: Vec<EnvRow>,
    pub default_runner_id: Option<Uuid>,
    pub prefix_root: PathBuf,
}

impl SettingsState {
    pub fn default_for(default_prefix_root: &Path) -> Self {
        Self {
            global_env_rows: Vec::new(),
            default_runner_id: None,
            prefix_root: default_prefix_root.to_path_buf(),
        }
    }

    pub fn from_map(
        map: &BTreeMap<String, String>,
        default_prefix_root: &Path,
    ) -> Result<Self, String> {
        let global_env: BTreeMap<String, String> = match map.get(SETTING_GLOBAL_ENV) {
            Some(raw) => serde_json::from_str(raw)
                .map_err(|error| format!("global_env 不是合法的 JSON 对象: {error}"))?,
            None => BTreeMap::new(),
        };

        if global_env.contains_key("WINEDLLOVERRIDES") {
            return Err("global_env 不能包含 WINEDLLOVERRIDES，请使用结构化 DLL override".into());
        }
        for key in global_env.keys() {
            launch::validate_env_key(key).map_err(|error| error.to_string())?;
        }

        let default_runner_id = match map.get(SETTING_DEFAULT_RUNNER_ID) {
            Some(raw) => Some(
                Uuid::parse_str(raw)
                    .map_err(|_| "default_runner_id 不是合法 UUID".to_string())?,
            ),
            None => None,
        };

        let prefix_root = map
            .get(SETTING_PREFIX_ROOT)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_prefix_root.to_path_buf());
        if !prefix_root.is_absolute() {
            return Err("prefix_root 必须是绝对路径".into());
        }

        let global_env_rows = global_env
            .iter()
            .map(|(key, value)| EnvRow {
                key: key.clone(),
                value: value.clone(),
            })
            .collect();

        Ok(Self {
            global_env_rows,
            default_runner_id,
            prefix_root,
        })
    }
}

#[derive(Debug, Default)]
pub struct PendingState {
    pub loading: bool,
    pub saving: bool,
    pub prefix: HashSet<Uuid>,
    pub dxvk: HashSet<Uuid>,
    pub detecting_wine: bool,
    pub importing_proton: bool,
    pub deleting: bool,
    pub settings_saving: bool,
}

pub struct App {
    pub paths: AppPaths,
    pub db: Option<Arc<Mutex<Db>>>,
    pub fatal: Option<String>,
    pub screen: Screen,
    pub games: Vec<Game>,
    pub runners: Vec<Runner>,
    pub settings: SettingsState,
    pub selected_game: Option<Uuid>,
    pub draft: Option<GameDraft>,
    pub prefix_status: Option<PrefixStatus>,
    pub session: SessionType,
    pub x11_outputs: Vec<X11Output>,
    pub gamescope_path: Option<PathBuf>,
    pub pending: PendingState,
    pub running: HashMap<Uuid, PathBuf>,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Screen),
    SelectGame(Uuid),
    NewGame,
    Loaded(LoadedData),
    LoadFailed(String),
    Draft(DraftAction),
    SaveDraft,
    DraftSaved(Result<Game, String>),
    Pick(crate::dialogs::DialogIntent),
    DialogResult {
        intent: crate::dialogs::DialogIntent,
        path: Option<PathBuf>,
    },
    DismissError,
    Error(String),
}

impl App {
    pub fn new(paths: AppPaths, db: Result<Db, StorageError>) -> (Self, iced::Task<Message>) {
        let settings = SettingsState::default_for(&paths.default_prefix_root);
        let mut app = Self {
            paths: paths.clone(),
            db: None,
            fatal: None,
            screen: Screen::Library,
            games: Vec::new(),
            runners: Vec::new(),
            settings,
            selected_game: None,
            draft: None,
            prefix_status: None,
            session: SessionType::Unknown,
            x11_outputs: Vec::new(),
            gamescope_path: None,
            pending: PendingState {
                loading: true,
                ..PendingState::default()
            },
            running: HashMap::new(),
            error: None,
        };

        match db {
            Ok(db) => {
                let db = Arc::new(Mutex::new(db));
                app.db = Some(db.clone());
                let task = iced::Task::perform(tasks::load_initial(db), |result| {
                    match result {
                        Ok(data) => Message::Loaded(data),
                        Err(error) => Message::LoadFailed(error),
                    }
                });
                (app, task)
            }
            Err(error) => {
                app.pending.loading = false;
                app.fatal = Some(format!(
                    "无法打开数据库 {}: {error}",
                    app.paths.db_path.display()
                ));
                (app, iced::Task::none())
            }
        }
    }

    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Navigate(screen) => {
                self.screen = screen;
                iced::Task::none()
            }
            Message::SelectGame(id) => {
                self.selected_game = Some(id);
                self.draft = self
                    .games
                    .iter()
                    .find(|game| game.id == id)
                    .map(GameDraft::from_game);
                self.prefix_status = None;
                self.screen = Screen::GameDetail;
                iced::Task::none()
            }
            Message::NewGame => {
                self.draft = None;
                self.selected_game = None;
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickExe)
            }
            Message::Loaded(data) => {
                self.apply_loaded(data);
                iced::Task::none()
            }
            Message::LoadFailed(error) => {
                self.pending.loading = false;
                self.error = Some(AppError::new("加载数据失败").with_detail(error));
                iced::Task::none()
            }
            Message::Draft(action) => {
                if let Some(draft) = &mut self.draft {
                    draft.apply(action);
                }
                iced::Task::none()
            }
            Message::SaveDraft => self.save_draft(),
            Message::DraftSaved(result) => self.apply_draft_saved(result),
            Message::Pick(intent) => crate::dialogs::pick(intent),
            Message::DialogResult { intent, path } => self.apply_dialog_result(intent, path),
            Message::DismissError => {
                self.error = None;
                iced::Task::none()
            }
            Message::Error(error) => {
                self.error = Some(AppError::new(error));
                iced::Task::none()
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        if let Some(fatal) = &self.fatal {
            return crate::widgets::fatal(fatal);
        }

        let screen: iced::Element<'_, Message> = match self.screen {
            Screen::Library => crate::views::library::view(self),
            Screen::GameDetail => crate::views::game_detail::view(self),
            Screen::RunnerManager => crate::views::runners::view(self),
            Screen::Settings => crate::views::settings::view(self),
        };

        match &self.error {
            Some(error) => iced::widget::column![
                crate::widgets::error_banner(error),
                screen
            ]
            .spacing(8)
            .padding(8)
            .into(),
            None => screen,
        }
    }

    fn apply_loaded(&mut self, data: LoadedData) {
        self.pending.loading = false;
        self.games = data.games;
        self.runners = data.runners;
        self.session = data.session;
        self.x11_outputs = data.x11_outputs;
        self.gamescope_path = data.gamescope_path;

        match SettingsState::from_map(
            &data.settings_map,
            &self.paths.default_prefix_root,
        ) {
            Ok(settings) => self.settings = settings,
            Err(error) => {
                self.settings = SettingsState::default_for(&self.paths.default_prefix_root);
                self.error = Some(AppError::new("设置读取失败").with_detail(error));
            }
        }

        if let Some(id) = self.settings.default_runner_id {
            if !self.runners.iter().any(|runner| runner.id == id) {
                self.settings.default_runner_id = None;
                self.error = Some(AppError::new(
                    "默认 Runner 不存在，已重置为未选择（保存设置后生效）",
                ));
            }
        }

        self.sort_games();
    }

    fn save_draft(&mut self) -> iced::Task<Message> {
        let Some(draft) = self.draft.clone() else {
            return iced::Task::none();
        };
        let game = match draft.to_game() {
            Ok(game) => game,
            Err(error) => {
                self.error =
                    Some(AppError::new("表单校验失败").with_detail(error.to_string()));
                return iced::Task::none();
            }
        };
        let Some(db) = self.db.clone() else {
            self.error = Some(AppError::new("数据库不可用"));
            return iced::Task::none();
        };

        self.pending.saving = true;
        iced::Task::perform(
            tasks::save_game(db, game, draft.is_new),
            Message::DraftSaved,
        )
    }

    fn apply_draft_saved(&mut self, result: Result<Game, String>) -> iced::Task<Message> {
        self.pending.saving = false;
        match result {
            Ok(game) => {
                if let Some(existing) = self
                    .games
                    .iter_mut()
                    .find(|existing| existing.id == game.id)
                {
                    *existing = game.clone();
                } else {
                    self.games.push(game.clone());
                }
                self.sort_games();
                self.selected_game = Some(game.id);
                self.draft = Some(GameDraft::from_game(&game));
                self.error = None;
            }
            Err(error) => {
                self.error = Some(AppError::new("保存失败").with_detail(error));
            }
        }
        iced::Task::none()
    }

    fn apply_dialog_result(
        &mut self,
        intent: crate::dialogs::DialogIntent,
        path: Option<PathBuf>,
    ) -> iced::Task<Message> {
        use crate::dialogs::DialogIntent;

        match intent {
            DialogIntent::PickExe => {
                if let Some(path) = path {
                    if let Some(draft) = &mut self.draft {
                        draft.executable_path = Some(path);
                    } else {
                        self.draft = Some(GameDraft::new(path));
                        self.selected_game = None;
                        self.screen = Screen::GameDetail;
                    }
                }
            }
            DialogIntent::PickWorkingDir => {
                if let (Some(draft), Some(path)) = (&mut self.draft, path) {
                    draft.working_dir = Some(path);
                }
            }
        }
        iced::Task::none()
    }

    fn sort_games(&mut self) {
        self.games
            .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    }
}
```

- [x] **Step 5: 写 `tasks.rs` 的加载与保存**

```rust
use display::{SessionType, X11Output};
use launcher_core::{Game, Runner};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use storage::Db;

#[derive(Debug, Clone)]
pub struct LoadedData {
    pub games: Vec<Game>,
    pub runners: Vec<Runner>,
    pub settings_map: BTreeMap<String, String>,
    pub session: SessionType,
    pub x11_outputs: Vec<X11Output>,
    pub gamescope_path: Option<PathBuf>,
}

pub async fn load_initial(
    db: Arc<Mutex<Db>>,
) -> Result<LoadedData, String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        let games = db.list_games().map_err(|error| error.to_string())?;
        let runners = db.list_runners().map_err(|error| error.to_string())?;
        let settings_map = db
            .get_all_settings()
            .map_err(|error| error.to_string())?;

        Ok(LoadedData {
            games,
            runners,
            settings_map,
            session: display::detect_session(),
            x11_outputs: display::list_x11_outputs(),
            gamescope_path: display::gamescope_path(),
        })
    })
    .await
    .map_err(|error| format!("加载任务失败: {error}"))?
}

pub async fn save_game(
    db: Arc<Mutex<Db>>,
    game: Game,
    is_new: bool,
) -> Result<Game, String> {
    tokio::task::spawn_blocking(move || {
        if !game.executable_path.is_file() {
            return Err(format!(
                "游戏可执行文件不存在: {}",
                game.executable_path.display()
            ));
        }
        if let Some(working_dir) = &game.working_dir {
            if !working_dir.is_dir() {
                return Err(format!(
                    "工作目录不存在: {}",
                    working_dir.display()
                ));
            }
        }

        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        if is_new {
            db.insert_game(&game).map_err(|error| error.to_string())?;
        } else {
            db.update_game(&game).map_err(|error| error.to_string())?;
        }
        Ok(game)
    })
    .await
    .map_err(|error| format!("保存任务失败: {error}"))?
}
```

- [x] **Step 6: 写最小 `widgets.rs`**

```rust
use crate::app::{AppError, Message};
use chrono::{DateTime, Local};
use iced::widget::{button, column, row, text};
use iced::Element;
use std::fmt;
use std::path::Path;

pub fn section<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![text(title).size(18), content.into()]
        .spacing(8)
        .padding(12)
        .into()
}

pub fn error_banner(error: &AppError) -> Element<'_, Message> {
    let mut content = column![
        row![
            text(format!("错误：{}", error.title)).size(16),
            iced::widget::horizontal_space(),
            button("关闭").on_press(Message::DismissError)
        ]
        .spacing(8)
    ]
    .spacing(4)
    .padding(8);

    if let Some(detail) = &error.detail {
        content = content.push(text(detail).size(12));
    }
    content.into()
}

pub fn fatal(message: &str) -> Element<'_, Message> {
    column![
        text("gamehub 无法启动").size(28),
        text(message).size(14),
    ]
    .spacing(12)
    .padding(24)
    .into()
}

pub fn path_text(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "未选择".into())
}

pub fn format_timestamp(timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0))
        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "从未".into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice<T> {
    pub value: T,
    pub label: String,
}

impl<T> Choice<T> {
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub fn arch_choices() -> Vec<Choice<launcher_core::Arch>> {
    vec![
        Choice::new(launcher_core::Arch::Win64, "64 位 (win64)"),
        Choice::new(launcher_core::Arch::Win32, "32 位 (win32)"),
    ]
}

pub fn arch_choice(arch: launcher_core::Arch) -> Choice<launcher_core::Arch> {
    match arch {
        launcher_core::Arch::Win64 => Choice::new(arch, "64 位 (win64)"),
        launcher_core::Arch::Win32 => Choice::new(arch, "32 位 (win32)"),
    }
}
```

- [x] **Step 7: 创建 `views/mod.rs`**

```rust
pub mod game_detail;
pub mod library;
pub mod runners;
pub mod settings;
```

不要创建临时 stub。`draft.rs`、`dialogs.rs` 与四个 view 文件分别在 U2、U3、U4 中按最终代码创建。

- [x] **Step 8: 与 U2–U4 一起编译**

U1 与 U2、U3、U4 属于同一个检查点：U1 引入了 `draft`/`dialogs`/`views` 模块，但对应文件在后续任务创建。执行顺序：

1. 完成 U2 的 `draft.rs`。
2. 完成 U3 的 `dialogs.rs`。
3. 完成 U4 的 `views/library.rs`、`views/runners.rs`、`views/settings.rs`。
4. 完成 U5 的 `views/game_detail.rs`。
5. 回到这里运行 `cargo build -p ui`。

不要在检查点中间提交带 stub 的代码。

- [x] **Step 9: 编译验证**

Run: `cargo build -p ui`
Expected: 成功。若报 `Message` 未实现 `Debug/Clone`，检查组件的 derive。

- [x] **Step 10: 提交（与 U2–U5 合并）**

```bash
git add crates/ui
git commit -m "feat(ui): add app shell, draft validation, dialogs and core views"
```

---

## Task U2: `GameDraft` 与校验

**Files:**
- Modify: `crates/ui/src/draft.rs`

- [x] **Step 1: 写完整测试**

创建 `crates/ui/src/draft.rs`，内容如下：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{
        Arch, DisplayConfig, DllOverride, GamescopeMode, OverrideMode,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn draft() -> GameDraft {
        GameDraft {
            id: Uuid::nil(),
            is_new: true,
            name: "Test Game".into(),
            executable_path: Some(PathBuf::from("/games/Test/game.exe")),
            working_dir: None,
            runner_id: None,
            prefix_path: None,
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            resolution_width: String::new(),
            resolution_height: String::new(),
            refresh_rate: String::new(),
            env_rows: Vec::new(),
            dll_rows: Vec::new(),
            args_rows: Vec::new(),
            created_at: 1_700_000_000,
            last_played_at: None,
        }
    }

    #[test]
    fn new_draft_uses_file_stem_as_name() {
        let draft = GameDraft::new(PathBuf::from("/games/Cyberpunk 2077.exe"));
        assert_eq!(draft.name, "Cyberpunk 2077");
        assert!(draft.is_new);
        assert_eq!(draft.executable_path, Some(PathBuf::from("/games/Cyberpunk 2077.exe")));
    }

    #[test]
    fn minimal_draft_converts_to_game() {
        let game = draft().to_game().unwrap();
        assert_eq!(game.name, "Test Game");
        assert_eq!(game.display.resolution, None);
        assert_eq!(game.display.refresh_rate, None);
        assert!(game.env_overrides.is_empty());
        assert!(game.dll_overrides.is_empty());
        assert!(game.args.is_empty());
    }

    #[test]
    fn empty_name_is_rejected() {
        let mut draft = draft();
        draft.name = "   ".into();
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn missing_or_relative_executable_is_rejected() {
        let mut draft = draft();
        draft.executable_path = None;
        assert!(draft.to_game().is_err());

        let mut draft = draft();
        draft.executable_path = Some(PathBuf::from("game.exe"));
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn resolution_requires_both_dimensions() {
        let mut draft = draft();
        draft.resolution_width = "1920".into();
        assert!(draft.to_game().is_err());

        draft.resolution_height = "1080".into();
        assert_eq!(
            draft.to_game().unwrap().display.resolution,
            Some((1920, 1080))
        );

        draft.resolution_width = "0".into();
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn refresh_rate_parses_and_validates() {
        let mut draft = draft();
        draft.refresh_rate = "144".into();
        assert_eq!(draft.to_game().unwrap().display.refresh_rate, Some(144));

        draft.refresh_rate = "0".into();
        assert!(draft.to_game().is_err());

        draft.refresh_rate = "abc".into();
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn empty_env_rows_are_skipped_and_duplicates_rejected() {
        let mut draft = draft();
        draft.env_rows = vec![
            EnvRow {
                key: "".into(),
                value: "".into(),
            },
            EnvRow {
                key: "DXVK_HUD".into(),
                value: "fps".into(),
            },
        ];
        let game = draft.to_game().unwrap();
        assert_eq!(game.env_overrides.len(), 1);
        assert_eq!(game.env_overrides["DXVK_HUD"], "fps");

        draft.env_rows.push(EnvRow {
            key: "DXVK_HUD".into(),
            value: "fps,frametimes".into(),
        });
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn manual_winedlloverrides_and_invalid_keys_are_rejected() {
        let mut draft = draft();
        draft.env_rows = vec![EnvRow {
            key: "WINEDLLOVERRIDES".into(),
            value: "x=n".into(),
        }];
        assert!(draft.to_game().is_err());

        draft.env_rows = vec![EnvRow {
            key: "BAD KEY".into(),
            value: "x".into(),
        }];
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn dll_names_are_normalized_and_duplicates_rejected() {
        let mut draft = draft();
        draft.dll_rows = vec![
            DllRow {
                name: "d3d11.dll".into(),
                mode: OverrideMode::NativeBuiltin,
            },
            DllRow {
                name: "  winmm ".into(),
                mode: OverrideMode::Native,
            },
            DllRow {
                name: "".into(),
                mode: OverrideMode::Native,
            },
        ];
        let game = draft.to_game().unwrap();
        assert_eq!(
            game.dll_overrides,
            vec![
                DllOverride {
                    dll_name: "d3d11".into(),
                    mode: OverrideMode::NativeBuiltin
                },
                DllOverride {
                    dll_name: "winmm".into(),
                    mode: OverrideMode::Native
                }
            ]
        );

        draft.dll_rows.push(DllRow {
            name: "D3D11".into(),
            mode: OverrideMode::Builtin,
        });
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn invalid_dll_names_are_rejected() {
        for name in ["../evil", "bad name", "a/b"] {
            let mut draft = draft();
            draft.dll_rows = vec![DllRow {
                name: name.into(),
                mode: OverrideMode::Native,
            }];
            assert!(draft.to_game().is_err(), "expected {name:?} to fail");
        }
    }

    #[test]
    fn args_are_trimmed_and_blank_rows_skipped() {
        let mut draft = draft();
        draft.args_rows = vec!["-skip".into(), "  ".into(), " --windowed ".into()];
        assert_eq!(
            draft.to_game().unwrap().args,
            vec!["-skip".to_string(), "--windowed".to_string()]
        );
    }

    #[test]
    fn from_game_round_trips() {
        let mut game = draft().to_game().unwrap();
        game.id = Uuid::new_v4();
        game.env_overrides = BTreeMap::from([("A".into(), "1".into())]);
        game.dll_overrides = vec![DllOverride {
            dll_name: "d3d11".into(),
            mode: OverrideMode::NativeBuiltin,
        }];
        game.args = vec!["-skip".into()];
        game.display = DisplayConfig {
            resolution: Some((2560, 1440)),
            refresh_rate: Some(144),
            fullscreen: false,
            gamescope: GamescopeMode::Forced,
            ..DisplayConfig::default()
        };

        let draft = GameDraft::from_game(&game);
        assert_eq!(draft.to_game().unwrap(), game);
    }
}
```

- [x] **Step 2: 运行并确认失败**

Run: `cargo test -p ui draft`
Expected: 编译失败，字段/方法缺失。

- [x] **Step 3: 写完整实现**

在测试模块之前写：

```rust
use launcher_core::{
    Arch, DisplayConfig, DllOverride, Game, OverrideMode,
};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvRow {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllRow {
    pub name: String,
    pub mode: OverrideMode,
}

#[derive(Debug, Clone)]
pub struct GameDraft {
    pub id: Uuid,
    pub is_new: bool,
    pub name: String,
    pub executable_path: Option<PathBuf>,
    pub working_dir: Option<PathBuf>,
    pub runner_id: Option<Uuid>,
    pub prefix_path: Option<PathBuf>,
    pub arch: Arch,
    pub display: DisplayConfig,
    pub resolution_width: String,
    pub resolution_height: String,
    pub refresh_rate: String,
    pub env_rows: Vec<EnvRow>,
    pub dll_rows: Vec<DllRow>,
    pub args_rows: Vec<String>,
    pub created_at: i64,
    pub last_played_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum DraftAction {
    Name(String),
    WorkingDir(Option<PathBuf>),
    ClearWorkingDir,
    Arch(Arch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftError {
    pub field: &'static str,
    pub message: String,
}

impl DraftError {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for DraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for DraftError {}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_resolution(
    width: &str,
    height: &str,
) -> Result<Option<(u32, u32)>, DraftError> {
    let width = width.trim();
    let height = height.trim();
    match (width.is_empty(), height.is_empty()) {
        (true, true) => Ok(None),
        (true, false) | (false, true) => Err(DraftError::new(
            "分辨率",
            "宽和高必须同时填写或同时留空",
        )),
        (false, false) => {
            let width: u32 = width
                .parse()
                .map_err(|_| DraftError::new("分辨率", "宽度必须是正整数"))?;
            let height: u32 = height
                .parse()
                .map_err(|_| DraftError::new("分辨率", "高度必须是正整数"))?;
            if width == 0 || height == 0 {
                return Err(DraftError::new("分辨率", "宽和高必须大于 0"));
            }
            Ok(Some((width, height)))
        }
    }
}

fn parse_refresh(value: &str) -> Result<Option<u32>, DraftError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let rate: u32 = value
        .parse()
        .map_err(|_| DraftError::new("刷新率", "必须是正整数"))?;
    if rate == 0 {
        return Err(DraftError::new("刷新率", "必须大于 0"));
    }
    Ok(Some(rate))
}

fn normalize_dll_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.to_ascii_lowercase().ends_with(".dll") {
        trimmed[..trimmed.len() - 4].trim_end().to_string()
    } else {
        trimmed.to_string()
    }
}

fn env_rows_to_map(
    rows: &[EnvRow],
) -> Result<BTreeMap<String, String>, DraftError> {
    let mut map = BTreeMap::new();
    for row in rows {
        let key = row.key.trim();
        if key.is_empty() && row.value.trim().is_empty() {
            continue;
        }
        if key.is_empty() {
            return Err(DraftError::new("环境变量", "变量名不能为空"));
        }
        if key == "WINEDLLOVERRIDES" {
            return Err(DraftError::new(
                "环境变量",
                "WINEDLLOVERRIDES 必须通过结构化 DLL override 配置",
            ));
        }
        launch::validate_env_key(key)
            .map_err(|error| DraftError::new("环境变量", error.to_string()))?;
        if map.contains_key(key) {
            return Err(DraftError::new(
                "环境变量",
                format!("变量 {key} 重复"),
            ));
        }
        map.insert(key.to_string(), row.value.clone());
    }
    Ok(map)
}

fn dll_rows_to_overrides(
    rows: &[DllRow],
) -> Result<Vec<DllOverride>, DraftError> {
    let mut overrides = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let name = normalize_dll_name(&row.name);
        if name.is_empty() {
            continue;
        }
        launch::validate_dll_name(&name)
            .map_err(|error| DraftError::new("DLL Override", error.to_string()))?;
        let key = name.to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(DraftError::new(
                "DLL Override",
                format!("DLL {name} 重复"),
            ));
        }
        overrides.push(DllOverride {
            dll_name: name,
            mode: row.mode,
        });
    }
    Ok(overrides)
}

impl GameDraft {
    pub fn new(executable_path: PathBuf) -> Self {
        let name = executable_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .unwrap_or("新游戏")
            .to_string();

        Self {
            id: Uuid::new_v4(),
            is_new: true,
            name,
            executable_path: Some(executable_path),
            working_dir: None,
            runner_id: None,
            prefix_path: None,
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            resolution_width: String::new(),
            resolution_height: String::new(),
            refresh_rate: String::new(),
            env_rows: Vec::new(),
            dll_rows: Vec::new(),
            args_rows: Vec::new(),
            created_at: now_unix(),
            last_played_at: None,
        }
    }

    pub fn from_game(game: &Game) -> Self {
        let (resolution_width, resolution_height) = game
            .display
            .resolution
            .map(|(width, height)| (width.to_string(), height.to_string()))
            .unwrap_or_default();

        Self {
            id: game.id,
            is_new: false,
            name: game.name.clone(),
            executable_path: Some(game.executable_path.clone()),
            working_dir: game.working_dir.clone(),
            runner_id: game.runner_id,
            prefix_path: game.prefix_path.clone(),
            arch: game.arch,
            display: game.display.clone(),
            resolution_width,
            resolution_height,
            refresh_rate: game
                .display
                .refresh_rate
                .map(|rate| rate.to_string())
                .unwrap_or_default(),
            env_rows: game
                .env_overrides
                .iter()
                .map(|(key, value)| EnvRow {
                    key: key.clone(),
                    value: value.clone(),
                })
                .collect(),
            dll_rows: game
                .dll_overrides
                .iter()
                .map(|entry| DllRow {
                    name: entry.dll_name.clone(),
                    mode: entry.mode,
                })
                .collect(),
            args_rows: game.args.clone(),
            created_at: game.created_at,
            last_played_at: game.last_played_at,
        }
    }

    pub fn apply(&mut self, action: DraftAction) {
        match action {
            DraftAction::Name(value) => self.name = value,
            DraftAction::WorkingDir(value) => self.working_dir = value,
            DraftAction::ClearWorkingDir => self.working_dir = None,
            DraftAction::Arch(value) => self.arch = value,
        }
    }

    pub fn to_game(&self) -> Result<Game, DraftError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(DraftError::new("名称", "不能为空"));
        }

        let executable_path = self
            .executable_path
            .clone()
            .ok_or_else(|| DraftError::new("可执行文件", "未选择"))?;
        if !executable_path.is_absolute() {
            return Err(DraftError::new("可执行文件", "必须是绝对路径"));
        }

        if let Some(working_dir) = &self.working_dir {
            if working_dir.as_os_str().is_empty() || !working_dir.is_absolute() {
                return Err(DraftError::new("工作目录", "必须是绝对路径"));
            }
        }
        if let Some(prefix_path) = &self.prefix_path {
            if prefix_path.as_os_str().is_empty() || !prefix_path.is_absolute() {
                return Err(DraftError::new("Prefix", "必须是绝对路径"));
            }
        }

        let resolution = parse_resolution(&self.resolution_width, &self.resolution_height)?;
        let refresh_rate = parse_refresh(&self.refresh_rate)?;
        let mut display = self.display.clone();
        display.resolution = resolution;
        display.refresh_rate = refresh_rate;

        let env_overrides = env_rows_to_map(&self.env_rows)?;
        let dll_overrides = dll_rows_to_overrides(&self.dll_rows)?;
        let args = self
            .args_rows
            .iter()
            .map(|arg| arg.trim())
            .filter(|arg| !arg.is_empty())
            .map(str::to_owned)
            .collect();

        Ok(Game {
            id: self.id,
            name: name.to_string(),
            executable_path,
            working_dir: self.working_dir.clone(),
            runner_id: self.runner_id,
            prefix_path: self.prefix_path.clone(),
            arch: self.arch,
            display,
            env_overrides,
            dll_overrides,
            args,
            created_at: self.created_at,
            last_played_at: self.last_played_at,
        })
    }
}
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p ui draft`
Expected: 12 passed。

- [x] **Step 5: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add game draft model with full validation"
```

---

## Task U3: 文件对话框与导入流程

**Files:**
- Modify: `crates/ui/src/dialogs.rs`

- [x] **Step 1: 替换 `dialogs.rs`**

```rust
use crate::app::Message;
use iced::Task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogIntent {
    PickExe,
    PickWorkingDir,
}

fn dialog_impl(intent: DialogIntent) -> Option<std::path::PathBuf> {
    let dialog = rfd::FileDialog::new();
    match intent {
        DialogIntent::PickExe => dialog
            .add_filter("Windows 程序", &["exe"])
            .pick_file(),
        DialogIntent::PickWorkingDir => dialog.pick_folder(),
    }
}

pub fn pick(intent: DialogIntent) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || dialog_impl(intent)).await {
                Ok(path) => Some((intent, path)),
                Err(_) => None,
            }
        },
        |result| match result {
            Some((intent, path)) => Message::DialogResult { intent, path },
            None => Message::Error("文件对话框失败".into()),
        },
    )
}
```

- [x] **Step 2: 编译并手工验证**

Run: `cargo build -p ui`
Expected: 成功。

Run: `cargo run -p ui`（需 U1–U5 一起编译后再验证）。点击“新建游戏”，对话框应能打开并选择 `.exe`；取消返回 `None`，不报错。

- [x] **Step 3: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add async rfd file dialogs"
```

---

## Task U4: Library 与只读 Runner/Settings 视图

**Files:**
- Create: `crates/ui/src/views/library.rs`
- Create: `crates/ui/src/views/runners.rs`
- Create: `crates/ui/src/views/settings.rs`

- [x] **Step 1: 实现 Library**

```rust
use crate::app::{App, Message, Screen};
use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        text("游戏库").size(28),
        iced::widget::horizontal_space(),
        button("新建游戏").on_press(Message::NewGame),
        button("Runner 管理").on_press(Message::Navigate(Screen::RunnerManager)),
        button("设置").on_press(Message::Navigate(Screen::Settings)),
    ]
    .spacing(8);

    let mut list = column![].spacing(8);
    if app.games.is_empty() {
        list = list.push(text("还没有游戏。点击“新建游戏”选择一个 .exe。").size(14));
    }
    for game in &app.games {
        let running = app.running.contains_key(&game.id);
        let card = column![
            row![
                text(game.name.as_str()).size(18),
                text(if running { "运行中" } else { "" }).size(12),
            ]
            .spacing(8),
            text(game.executable_path.display().to_string()).size(12),
            text(format!("最近游玩：{}", crate::widgets::format_timestamp(game.last_played_at)))
                .size(12),
        ]
        .spacing(4)
        .padding(12);

        list = list.push(
            button(card)
                .on_press(Message::SelectGame(game.id))
                .width(Length::Fill),
        );
    }

    column![
        header,
        scrollable(list).height(Length::Fill),
    ]
    .spacing(16)
    .padding(20)
    .into()
}
```

- [x] **Step 2: 实现只读 RunnerManager**

```rust
use crate::app::{App, Message, Screen};
use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("Runner 管理").size(24),
    ]
    .spacing(12);

    let mut list = column![].spacing(8);
    if app.runners.is_empty() {
        list = list.push(text("尚未检测到 Runner。"));
    }
    for runner in &app.runners {
        let is_default = app.settings.default_runner_id == Some(runner.id);
        list = list.push(
            row![
                text(runner.name.as_str()).size(16),
                text(format!("{:?}", runner.kind)).size(12),
                text(runner.version_string.clone().unwrap_or_default()).size(12),
                text(if is_default { "默认" } else { "" }).size(12),
            ]
            .spacing(12),
        );
    }

    column![
        header,
        scrollable(list).height(Length::Fill),
    ]
    .spacing(16)
    .padding(20)
    .into()
}
```

- [x] **Step 3: 实现只读 Settings**

```rust
use crate::app::{App, Message, Screen};
use iced::widget::{button, column, row, text};
use iced::Element;

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("设置").size(24),
    ]
    .spacing(12);

    let default_runner = app
        .settings
        .default_runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.clone())
        .unwrap_or_else(|| "未设置".into());

    let mut env_list = column![].spacing(4);
    if app.settings.global_env_rows.is_empty() {
        env_list = env_list.push(text("（空）"));
    }
    for entry in &app.settings.global_env_rows {
        env_list = env_list.push(text(format!("{}={}", entry.key, entry.value)));
    }

    column![
        header,
        crate::widgets::section("默认 Runner", text(default_runner)),
        crate::widgets::section(
            "Prefix 根目录",
            text(app.settings.prefix_root.display().to_string()),
        ),
        crate::widgets::section("全局环境变量", env_list),
    ]
    .spacing(12)
    .padding(20)
    .into()
}
```

- [x] **Step 4: 编译**

Run: `cargo build -p ui`
Expected: 成功。

- [x] **Step 5: 手工验证**

Run: `cargo run -p ui`
Expected: 顶部按钮可导航；空库有提示；找不到 `.exe` 时对话框取消不报错。

- [x] **Step 6: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add library, runner and settings read-only views"
```

---

## Task U5: 基础 GameDetail 与保存

**Files:**
- Create: `crates/ui/src/views/game_detail.rs`

- [x] **Step 1: 实现基础详情页**

```rust
use crate::app::{App, Message, Screen};
use crate::dialogs::DialogIntent;
use crate::draft::DraftAction;
use crate::widgets::{arch_choice, arch_choices, path_text, section};
use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let Some(draft) = &app.draft else {
        return column![
            button("返回").on_press(Message::Navigate(Screen::Library)),
            text("未选择游戏"),
        ]
        .spacing(12)
        .padding(20)
        .into();
    };

    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text(if draft.is_new { "新建游戏" } else { "游戏详情" }).size(24),
        iced::widget::horizontal_space(),
        button("保存").on_press_maybe(
            (!app.pending.saving).then_some(Message::SaveDraft)
        ),
    ]
    .spacing(8);

    let exe_text = path_text(draft.executable_path.as_deref());
    let basic = column![
        text_input("游戏名称", draft.name.as_str())
            .on_input(|value| Message::Draft(DraftAction::Name(value)))
            .width(Length::Fill),
        row![
            text("可执行文件："),
            text(exe_text),
            button("选择").on_press(Message::Pick(DialogIntent::PickExe)),
        ]
        .spacing(8),
        row![
            text("工作目录："),
            text(path_text(draft.working_dir.as_deref())),
            button("选择").on_press(Message::Pick(DialogIntent::PickWorkingDir)),
            button("清除").on_press_maybe(
                draft
                    .working_dir
                    .is_some()
                    .then_some(Message::Draft(DraftAction::ClearWorkingDir))
            ),
        ]
        .spacing(8),
        row![
            text("架构："),
            pick_list(
                arch_choices(),
                Some(arch_choice(draft.arch)),
                |choice| Message::Draft(DraftAction::Arch(choice.value)),
            ),
        ]
        .spacing(8),
        text(format!(
            "最近游玩：{}",
            crate::widgets::format_timestamp(draft.last_played_at)
        ))
        .size(12),
    ]
    .spacing(10);

    let content = column![
        header,
        section("基本信息", basic),
    ]
    .spacing(12)
    .padding(20);

    scrollable(content).height(Length::Fill).into()
}
```

- [x] **Step 2: 增加保存任务测试**

在 `tasks.rs` 测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, DisplayConfig, Game};
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;
    use uuid::Uuid;

    fn game(path: PathBuf) -> Game {
        Game {
            id: Uuid::new_v4(),
            name: "Test".into(),
            executable_path: path,
            working_dir: None,
            runner_id: None,
            prefix_path: None,
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            env_overrides: BTreeMap::new(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 1,
            last_played_at: None,
        }
    }

    #[tokio::test]
    async fn save_game_inserts_then_updates() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));

        let mut value = game(exe);
        let saved = save_game(db.clone(), value.clone(), true).await.unwrap();
        assert_eq!(saved, value);

        value.name = "Renamed".into();
        save_game(db.clone(), value.clone(), false).await.unwrap();
        let stored = db.lock().unwrap().get_game(value.id).unwrap().unwrap();
        assert_eq!(stored.name, "Renamed");
    }

    #[tokio::test]
    async fn save_game_rejects_missing_executable() {
        let dir = tempdir().unwrap();
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let value = game(dir.path().join("missing.exe"));
        assert!(save_game(db, value, true).await.is_err());
    }

    #[tokio::test]
    async fn load_initial_reads_database_state() {
        let db = Db::open_in_memory().unwrap();
        db.set_setting(super::super::app::SETTING_PREFIX_ROOT, "/tmp/prefixes")
            .unwrap();
        let db = Arc::new(Mutex::new(db));

        let data = load_initial(db).await.unwrap();
        assert_eq!(
            data.settings_map.get(super::super::app::SETTING_PREFIX_ROOT),
            Some(&"/tmp/prefixes".to_string())
        );
    }
}
```

`super::super::app::SETTING_PREFIX_ROOT` 在 `tasks::tests` 中指向 `crate::app`；更直接使用 `crate::app::SETTING_PREFIX_ROOT`。把上面两处替换为 `crate::app::SETTING_PREFIX_ROOT`。

- [x] **Step 3: 运行测试**

Run: `cargo test -p ui`
Expected: 15 passed（12 draft + 3 tasks）。

- [x] **Step 4: 手工全流程**

Run: `cargo run -p ui`

1. 点击“新建游戏” → 选择任意 `.exe`。
2. 名称自动填充，修改名称/工作目录/架构。
3. 点击“保存”，返回 Library 后能看到卡片。
4. 关闭应用再启动，卡片仍在。
5. 点击卡片进入详情，能重新编辑并保存。

- [x] **Step 5: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add basic game detail and persistence flow"
```

---

## Task U6: 收尾

**Files:**
- Modify: `CHANGELOG.md`

- [x] **Step 1: 全量检查**

```bash
cargo fmt --all
cargo clippy -p ui --all-targets -- -D warnings
cargo test -p ui
```

Expected: 全部通过。特别检查没有遗留的临时 stub 或未使用函数。

- [x] **Step 2: 更新 CHANGELOG**

```markdown
## P7 — UI 骨架

- ui 拆分 lib/bin，`run()` 统一初始化日志/DB/Iced。
- 启动异步加载 games/runners/settings/session/xrandr/gamescope。
- Library 列表、基础 GameDetail、只读 Runner/Settings。
- GameDraft 校验（名称/路径/分辨率/环境变量/DLL/参数）。
- rfd 文件对话框与本地 exe 导入保存。

验证：`cargo test -p ui`；重启后配置保留。
```

- [x] **Step 3: 提交**

```bash
git add crates/ui CHANGELOG.md
git commit -m "chore(ui): finish UI shell phase"
```

---

## P7 验收

- [x] `cargo test -p ui` 通过。
- [x] 应用启动后 1 秒内显示 Library（不阻塞在 DB/IO）。
- [x] 导入 exe → 编辑 → 保存 → 重启后仍在。
- [x] 取消文件对话框不产生错误。
- [x] DB 打开失败时显示 fatal 页，不进入空白界面。
- [x] 草稿校验覆盖：空名称、相对路径、半填分辨率、非法刷新率、重复/非法环境变量、`.dll` 归一化、重复 DLL、空参数行。
- [x] 视图导航 Library ↔ GameDetail / RunnerManager / Settings 正常。

## 实施记录

- Iced API：0.13.1 与计划一致（`iced::application(...).theme().window().run_with()`、
  `Task::perform`/`Task::none`、`horizontal_space()`、`pick_list`、
  `button(card)`、`on_press_maybe` 均可用）；另使用 builder 的
  `.default_font(iced::Font::with_name(...))`。
- 字体：Iced 默认 SansSerif 在本机不回落 CJK，首次运行 Library 中文为方块；
  按 01-foundation 的应急方案设置 `Noto Sans CJK SC` 后正常，未打包字体。
  README 的字体说明同步更新。
- 测试：`cargo test -p ui` 16 passed（12 draft + 4 tasks，额外
  `saved_game_survives_reopen`）；`cargo test --workspace` 146 passed，
  1 ignored；fmt/clippy/build 全量通过。
- 手工验证：XWayland + XTEST 真实点击应用窗口，桌面 portal 截图核对。
  已确认启动即显示 Library、导入 exe、编辑并保存、重启持久化、
  Library ↔ GameDetail / Settings / RunnerManager 导航、DB 打开失败显示
  fatal 页。本机 rfd 首选 Wayland portal；portal 对话框内部按钮无法用 X11
  合成事件点击，取消路径（返回 `None` → 状态不变、无错误）由代码路径与
  对话框失败路径确认。
- 偏差：U1–U5 按可编译检查点合并提交；`sort_by_key`（clippy）；额外持久化
  测试；显式 CJK 默认字体。详见 CHANGELOG P7。

## 当前实现（v0.12.0）

### 模块结构

```
crates/ui/src/
├── main.rs              # 调用 ui::run()
├── lib.rs               # run()、窗口与默认字体
├── app/{mod,message,update,view}.rs
├── dialogs.rs           # rfd：exe/封面/DXVK/NVAPI/字体目录/Prefix
├── fonts.rs             # fc-list 扫描（纯解析 + 阻塞调用）
├── i18n/{mod,errors}.rs # 语言目录、探测、AppError/DraftError 渲染
├── styles.rs            # 按钮/输入/下拉/滚动条样式
├── theme.rs             # 颜色令牌、ThemeMode、字体族与字重
├── widgets/{badges,choices,controls,cover,layout,modal,mod}.rs
└── views/{library,game_detail,logs,runners,settings,mod}.rs
```

### App 状态（与 P7 的差异）

- `service: Option<AppService>` 取代 `db`；持久化/探测/启动全部经 application。
- 新增：`monitors`（取代 `x11_outputs`）、`mangohud_path`、`winetricks_path`、`starting`/`stopping`、`running: HashMap<Uuid, Option<PathBuf>>`（原生游戏 prefix 为 `None`）、`new_game_platform`、`dxvk_dir`、`components`、`installed_verbs`、`logs_game`/`logs_content`/`logs_error`、`cover_handles`、`window_width`、`lang`。
- 设置状态包含主题、语言、字体与兼容层调优；`theme_saving`/`language_saving`/`font_saving` 分别去抖，错误用 `ErrorNotice` 区分来源。
- 草稿：`GameDraft` 迁到 `application::editor`，新增 `platform`、`cover_path`、`playtime_seconds`；`DraftAction::Platform` 切换平台时会刷新 prefix 状态并保留 Wine 设置。
- 新建流程：`NewGame` 先弹平台选择（`new_game_platform`），`BeginNewGame(platform)` 记录选择并打开按平台过滤的文件对话框；`CancelNewGame` 关闭弹窗。`DialogIntent::PickExe(Platform)` 携带平台，Windows 过滤 `.exe`，Linux 不过滤。

### 视图与交互

- 根布局：侧边栏 + 内容区；浅色/深色主题；游戏库为响应式封面卡片网格，含「正在游玩」卡、启动/停止、设置、日志。
- 侧边栏顶部品牌图标使用 `packaging/gamehub.svg`（经 `include_bytes!` 嵌入，`iced::widget::svg` 渲染为 22×22），主题切换时自动重绘。
- 侧边栏底部显示会话类型与默认 Runner；其下为 workspace 版本号（`env!("CARGO_PKG_VERSION")`），以链接样式呈现，点击经 `xdg-open` 打开项目 GitHub 主页，悬停显示本地化提示。
- 详情页：平台选择置于基本信息卡首位；原生游戏隐藏架构、Runner/Prefix/DXVK/winetricks/DLL override 卡片及摘要中的 Prefix/DXVK 事实，副标题与环境变量提示也使用原生文案。摘要卡列出平台与总时长。
- 日志页：读取 `logs/games/<id>.log` 最近 1000 行；组件输出在详情页内联展示最近 400 行。
- 弹窗：平台选择、删除确认、封面选择等使用 `widgets::modal`；模态层通过 `stack!` + `opaque` 叠加。
- 字体：所有文本/按钮/输入/下拉/徽章走 `theme::font(weight)`；勾选符号使用内置字体；日志与 winetricks 输出保持等宽。
