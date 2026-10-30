# P8 Runner / Prefix / Settings UI Implementation Plan

> 历史实施计划（2026-10-27，P8）。Task U7–U11 的代码片段与验收记录保留原样；
> 与当前代码不一致的契约、文件路径和流程以文末「当前实现（v0.9.0）」为准。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 UI 中打通 Runner 探测/导入/默认选择、prefix 状态/创建/DXVK 安装、设置保存、Runner/Prefix 删除保护。

**Architecture:** 所有重活放进 `tasks.rs`：Runner 探测、DB 写入、prefix 初始化、DXVK 安装、删除引用检查。`App::update` 只做状态转移与 Task 编排；`views/*` 只读 `App`。

**Tech Stack:** Iced 0.13、rfd、tokio、storage、prefix、runner-wine、launch。

**前置条件：** P7 完成；P4/P6 已实现。

---

## 本阶段向 `App`/`Message` 追加的契约

```rust
pub struct App {
    // ...P7 字段...
    pub dxvk_dir: Option<PathBuf>,
    pub confirming: Option<DeleteTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Game(Uuid),
    Runner(Uuid),
    Prefix { game_id: Uuid, path: PathBuf },
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    Env(EnvAction),
    DefaultRunner(Option<Uuid>),
    PrefixRoot(PathBuf),
}

#[derive(Debug, Clone)]
pub enum EnvAction {
    Add,
    Remove(usize),
    Key { index: usize, key: String },
    Value { index: usize, value: String },
}
```

`Message` 追加：

```rust
DetectSystemWine,
WineDetected(Result<Vec<Runner>, String>),
ImportProton,
ProtonImported(Result<Runner, String>),
SetDefaultRunner(Option<Uuid>),
DefaultRunnerSaved { id: Option<Uuid>, result: Result<(), String> },
CreatePrefix(Uuid),
PrefixCreated { game_id: Uuid, result: Result<PathBuf, String> },
InstallDxvk(Uuid),
DxvkInstalled { game_id: Uuid, result: Result<(), String> },
RefreshPrefixStatus(Uuid),
PrefixStatusLoaded { game_id: Uuid, status: PrefixStatus },
Settings(SettingsAction),
SaveSettings,
SettingsSaved(Result<(), String>),
ConfirmDelete(DeleteTarget),
CancelDelete,
DeleteConfirmed,
DeleteFinished { target: DeleteTarget, result: Result<(), String> },
```

`DraftAction` 追加：`Runner(Option<Uuid>)`、`Prefix(Option<PathBuf>)`、`ClearPrefix`。

`DialogIntent` 追加：`PickProtonDir`、`PickPrefixDir`、`PickDxvkDir`、`PickPrefixRoot`。

---

## Task U7: 扩展 App 状态、草稿字段与任务层

**Files:**
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/draft.rs`
- Modify: `crates/ui/src/tasks.rs`
- Modify: `crates/ui/src/dialogs.rs`

- [x] **Step 1: 在 `draft.rs` 增加动作与行编辑器**

```rust
#[derive(Debug, Clone)]
pub enum EnvAction {
    Add,
    Remove(usize),
    Key { index: usize, key: String },
    Value { index: usize, value: String },
}

pub fn apply_env_action(rows: &mut Vec<EnvRow>, action: EnvAction) {
    match action {
        EnvAction::Add => rows.push(EnvRow {
            key: String::new(),
            value: String::new(),
        }),
        EnvAction::Remove(index) => {
            if index < rows.len() {
                rows.remove(index);
            }
        }
        EnvAction::Key { index, key } => {
            if let Some(row) = rows.get_mut(index) {
                row.key = key;
            }
        }
        EnvAction::Value { index, value } => {
            if let Some(row) = rows.get_mut(index) {
                row.value = value;
            }
        }
    }
}
```

`DraftAction` 增加：

```rust
    Runner(Option<Uuid>),
    Prefix(Option<PathBuf>),
    ClearPrefix,
```

`GameDraft::apply` 增加：

```rust
            DraftAction::Runner(value) => self.runner_id = value,
            DraftAction::Prefix(value) => self.prefix_path = value,
            DraftAction::ClearPrefix => self.prefix_path = None,
```

- [x] **Step 2: 在 `app.rs` 增加字段、枚举与设置序列化**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Game(Uuid),
    Runner(Uuid),
    Prefix { game_id: Uuid, path: PathBuf },
}

impl DeleteTarget {
    pub fn label(&self) -> String {
        match self {
            Self::Game(id) => format!("游戏 {id}"),
            Self::Runner(id) => format!("Runner {id}"),
            Self::Prefix { path, .. } => format!("Prefix {}", path.display()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    Env(crate::draft::EnvAction),
    DefaultRunner(Option<Uuid>),
    PrefixRoot(PathBuf),
}
```

`App` 增加：

```rust
    pub dxvk_dir: Option<PathBuf>,
    pub confirming: Option<DeleteTarget>,
```

`App::new` 初始化：

```rust
            dxvk_dir: None,
            confirming: None,
```

`SettingsState` 增加：

```rust
    pub fn to_map(&self) -> Result<BTreeMap<String, Option<String>>, String> {
        if !self.prefix_root.is_absolute() {
            return Err("prefix_root 必须是绝对路径".into());
        }
        let env = crate::draft::env_rows_to_map(&self.global_env_rows)
            .map_err(|error| error.to_string())?;
        let global_env = serde_json::to_string(&env)
            .map_err(|error| format!("global_env 序列化失败: {error}"))?;

        let mut map = BTreeMap::new();
        map.insert(SETTING_GLOBAL_ENV.to_string(), Some(global_env));
        let prefix_root = self
            .prefix_root
            .to_str()
            .ok_or_else(|| "prefix_root 不是 UTF-8 路径".to_string())?
            .to_string();
        map.insert(SETTING_PREFIX_ROOT.to_string(), Some(prefix_root));
        map.insert(
            SETTING_DEFAULT_RUNNER_ID.to_string(),
            self.default_runner_id.map(|id| id.to_string()),
        );
        Ok(map)
    }
```

把 `draft.rs` 的 `env_rows_to_map` 可见性从 `fn` 改为 `pub(crate) fn`，供 `app.rs` 使用。

`Message` 增加本阶段的所有变体（见上方契约）。

- [x] **Step 3: 在 `tasks.rs` 增加任务**

```rust
use crate::app::DeleteTarget;
// 以下导入与文件顶部已有导入合并，避免重复定义：Game/Runner/BTreeMap/PathBuf/Arc/Mutex/Db 已在 tasks.rs 顶部导入。
use launcher_core::{Game, Runner};
use prefix::{DxvkSource, PrefixPlan, PrefixStatus};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use storage::Db;
use uuid::Uuid;

pub async fn prefix_status(plan: PrefixPlan) -> PrefixStatus {
    tokio::task::spawn_blocking(move || prefix::status(&plan))
        .await
        .unwrap_or(PrefixStatus::NotInitialized)
}

pub async fn create_prefix(
    plan: PrefixPlan,
    dxvk: Option<DxvkSource>,
) -> Result<PathBuf, String> {
    plan.initialize().await.map_err(|error| error.to_string())?;
    if let Some(source) = dxvk {
        prefix::install_dxvk(&plan, &source)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(plan.path)
}

pub async fn install_dxvk(
    plan: PrefixPlan,
    source: DxvkSource,
) -> Result<(), String> {
    prefix::install_dxvk(&plan, &source)
        .await
        .map_err(|error| error.to_string())
}

pub async fn detect_and_persist(
    db: Arc<Mutex<Db>>,
) -> Result<Vec<Runner>, String> {
    let found = runner_wine::detect_system_wine().await;
    persist_runners(db, found).await
}

pub async fn persist_runners(
    db: Arc<Mutex<Db>>,
    runners: Vec<Runner>,
) -> Result<Vec<Runner>, String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        for mut runner in runners {
            let existing = db
                .list_runners()
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|existing| existing.binary_path == runner.binary_path);

            if let Some(existing) = existing {
                runner.id = existing.id;
                db.update_runner(&runner)
                    .map_err(|error| error.to_string())?;
            } else {
                match db.insert_runner(&runner) {
                    Ok(()) | Err(storage::StorageError::DuplicateRunner { .. }) => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        db.list_runners().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Runner 探测任务失败: {error}"))?
}

pub async fn import_proton(
    db: Arc<Mutex<Db>>,
    dir: PathBuf,
) -> Result<Runner, String> {
    tokio::task::spawn_blocking(move || {
        let runner = runner_wine::validate_proton_dir(&dir)
            .map_err(|error| error.to_string())?;
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        db.insert_runner(&runner).map_err(|error| error.to_string())?;
        Ok(runner)
    })
    .await
    .map_err(|error| format!("Proton 导入任务失败: {error}"))?
}

pub async fn save_default_runner(
    db: Arc<Mutex<Db>>,
    id: Option<Uuid>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        match id {
            Some(id) => db
                .set_setting(crate::app::SETTING_DEFAULT_RUNNER_ID, &id.to_string())
                .map_err(|error| error.to_string()),
            None => db
                .delete_setting(crate::app::SETTING_DEFAULT_RUNNER_ID)
                .map_err(|error| error.to_string()),
        }
    })
    .await
    .map_err(|error| format!("默认 Runner 保存任务失败: {error}"))?
}

pub async fn save_settings(
    db: Arc<Mutex<Db>>,
    values: BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
        for (key, value) in values {
            match value {
                Some(value) => db
                    .set_setting(&key, &value)
                    .map_err(|error| error.to_string())?,
                None => db
                    .delete_setting(&key)
                    .map_err(|error| error.to_string())?,
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("设置保存任务失败: {error}"))?
}

pub async fn delete_target(
    db: Arc<Mutex<Db>>,
    target: DeleteTarget,
    running: HashMap<Uuid, PathBuf>,
    default_runner_id: Option<Uuid>,
) -> Result<(), String> {
    match target {
        DeleteTarget::Runner(id) => tokio::task::spawn_blocking(move || {
            if default_runner_id == Some(id) {
                return Err("该 Runner 是全局默认，请先更换默认 Runner".to_string());
            }
            let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
            let count = db
                .count_games_using_runner(id)
                .map_err(|error| error.to_string())?;
            if count > 0 {
                return Err(format!("仍有 {count} 个游戏使用该 Runner"));
            }
            db.delete_runner(id).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| format!("删除任务失败: {error}"))?,
        DeleteTarget::Prefix { game_id, path } => {
            if running.contains_key(&game_id)
                || running.values().any(|running_path| running_path == &path)
            {
                return Err("游戏正在运行，不能删除 Prefix".into());
            }

            let path_for_db = path.clone();
            tokio::task::spawn_blocking(move || {
                let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
                let count = db
                    .count_games_using_prefix(&path_for_db)
                    .map_err(|error| error.to_string())?;
                if count > 1 {
                    return Err("该 Prefix 被多个游戏共享，不能删除".to_string());
                }
                if count == 1 {
                    let mut game = db
                        .get_game(game_id)
                        .map_err(|error| error.to_string())?
                        .ok_or_else(|| "游戏不存在".to_string())?;
                    if game.prefix_path.as_deref() != Some(path_for_db.as_path()) {
                        return Err("该 Prefix 被其他游戏使用".to_string());
                    }
                    game.prefix_path = None;
                    db.update_game(&game)
                        .map_err(|error| error.to_string())?;
                }
                Ok::<(), String>(())
            })
            .await
            .map_err(|error| format!("删除任务失败: {error}"))??;

            prefix::delete_prefix(&path)
                .await
                .map_err(|error| error.to_string())
        }
        DeleteTarget::Game(id) => {
            if running.contains_key(&id) {
                return Err("游戏正在运行，不能删除".into());
            }
            tokio::task::spawn_blocking(move || {
                let db = db.lock().map_err(|_| "数据库锁不可用".to_string())?;
                db.delete_game(id).map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| format!("删除任务失败: {error}"))?
        }
    }
}
```

- [x] **Step 4: 扩展 `dialogs.rs`**

`DialogIntent` 增加 `PickProtonDir`、`PickPrefixDir`、`PickDxvkDir`、`PickPrefixRoot`。

`dialog_impl`：

```rust
        DialogIntent::PickExe => dialog
            .add_filter("Windows 程序", &["exe"])
            .pick_file(),
        DialogIntent::PickWorkingDir
        | DialogIntent::PickProtonDir
        | DialogIntent::PickPrefixDir
        | DialogIntent::PickDxvkDir
        | DialogIntent::PickPrefixRoot => dialog.pick_folder(),
```

- [x] **Step 5: 编译**

Run: `cargo build -p ui`
Expected: update 的 match 还不完整，出现非穷尽错误；在 U8 补 handler 后通过。本任务可以先只编译 `cargo check -p ui` 并记录错误数量。

- [x] **Step 6: 提交（与 U8 合并提交亦可）**

```bash
git add crates/ui
git commit -m "feat(ui): extend app state and tasks for runners and prefixes"
```

---

## Task U8: update handler 与辅助函数

**Files:**
- Modify: `crates/ui/src/app.rs`

- [x] **Step 1: 增加辅助函数**

在 `app.rs` 顶部把 `use prefix::PrefixStatus;` 改为 `use prefix::{PrefixPlan, PrefixStatus};`，然后在 `impl App` 中：

```rust
    fn game_for_actions(&self, game_id: Uuid) -> Result<Game, String> {
        if let Some(draft) = &self.draft {
            if draft.id == game_id {
                return draft.to_game().map_err(|error| error.to_string());
            }
        }
        self.games
            .iter()
            .find(|game| game.id == game_id)
            .cloned()
            .ok_or_else(|| "游戏不存在".to_string())
    }

    fn prefix_plan_for(
        &self,
        game: &Game,
    ) -> Result<PrefixPlan, AppError> {
        let runner = launch::resolve_runner(
            game,
            &self.runners,
            self.settings.default_runner_id,
        )
        .map_err(|error| {
            AppError::new("Runner 不可用").with_detail(error.to_string())
        })?;

        Ok(prefix::plan_for_game(
            game,
            runner,
            &self.settings.prefix_root,
            self.paths.steam_compat_client.clone(),
        ))
    }

    fn refresh_prefix_status(&self, game_id: Uuid) -> iced::Task<Message> {
        let game = match self.game_for_actions(game_id) {
            Ok(game) => game,
            Err(_) => {
                return iced::Task::none();
            }
        };
        let plan = match self.prefix_plan_for(&game) {
            Ok(plan) => plan,
            Err(_) => {
                return iced::Task::none();
            }
        };

        iced::Task::perform(tasks::prefix_status(plan), move |status| {
            Message::PrefixStatusLoaded { game_id, status }
        })
    }
```

- [x] **Step 2: 修改 `Draft`、`SelectGame` 与 `apply_draft_saved`**

`Message::Draft` 分支改为仅在影响 prefix 的动作后刷新状态（名称、环境变量等文本输入不触发查询）：

```rust
            Message::Draft(action) => {
                let affects_prefix = matches!(
                    &action,
                    DraftAction::Runner(_)
                        | DraftAction::Prefix(_)
                        | DraftAction::ClearPrefix
                        | DraftAction::Arch(_)
                );
                let refresh_id = self.draft.as_ref().map(|draft| draft.id);
                if let Some(draft) = &mut self.draft {
                    draft.apply(action);
                }
                match (affects_prefix, refresh_id) {
                    (true, Some(id)) => {
                        self.prefix_status = None;
                        self.refresh_prefix_status(id)
                    }
                    _ => iced::Task::none(),
                }
            }
```

`SelectGame` 最后改为：

```rust
                let task = self.refresh_prefix_status(id);
                self.screen = Screen::GameDetail;
                task
```

`apply_draft_saved` 成功分支末尾改为：

```rust
                return self.refresh_prefix_status(game.id);
```

并把函数最后原先的 `iced::Task::none()` 保留给失败分支。

- [x] **Step 3: 扩展 `DialogResult`**

```rust
            DialogIntent::PickProtonDir => {
                if let Some(path) = path {
                    if let Some(db) = self.db.clone() {
                        self.pending.importing_proton = true;
                        return iced::Task::perform(
                            tasks::import_proton(db, path),
                            Message::ProtonImported,
                        );
                    }
                }
            }
            DialogIntent::PickPrefixDir => {
                if let (Some(draft), Some(path)) = (&mut self.draft, path) {
                    draft.prefix_path = Some(path);
                    let id = draft.id;
                    self.prefix_status = None;
                    return self.refresh_prefix_status(id);
                }
            }
            DialogIntent::PickDxvkDir => {
                self.dxvk_dir = path;
            }
            DialogIntent::PickPrefixRoot => {
                if let Some(path) = path {
                    self.settings.prefix_root = path;
                    self.prefix_status = None;
                    if let Some(id) = self.selected_game {
                        return self.refresh_prefix_status(id);
                    }
                }
            }
```

- [x] **Step 4: 增加 Message handler**

```rust
            Message::DetectSystemWine => {
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.detecting_wine = true;
                iced::Task::perform(tasks::detect_and_persist(db), Message::WineDetected)
            }
            Message::WineDetected(result) => {
                self.pending.detecting_wine = false;
                match result {
                    Ok(runners) => {
                        if runners.is_empty() {
                            self.error = Some(AppError::new(
                                "未找到系统 Wine，请确认 wine 在 PATH 中",
                            ));
                        }
                        self.runners = runners;
                        self.runners.sort_by(|left, right| {
                            left.name.to_lowercase().cmp(&right.name.to_lowercase())
                        });
                    }
                    Err(error) => {
                        self.error =
                            Some(AppError::new("检测 Wine 失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::ImportProton => {
                crate::dialogs::pick(crate::dialogs::DialogIntent::PickProtonDir)
            }
            Message::ProtonImported(result) => {
                self.pending.importing_proton = false;
                match result {
                    Ok(runner) => {
                        self.runners.push(runner);
                        self.runners.sort_by(|left, right| {
                            left.name.to_lowercase().cmp(&right.name.to_lowercase())
                        });
                    }
                    Err(error) => {
                        self.error =
                            Some(AppError::new("导入 Proton 失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::SetDefaultRunner(id) => {
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.settings_saving = true;
                iced::Task::perform(
                    tasks::save_default_runner(db, id),
                    move |result| Message::DefaultRunnerSaved { id, result },
                )
            }
            Message::DefaultRunnerSaved { id, result } => {
                self.pending.settings_saving = false;
                match result {
                    Ok(()) => {
                        self.settings.default_runner_id = id;
                        self.prefix_status = None;
                        if let Some(game_id) = self.selected_game {
                            return self.refresh_prefix_status(game_id);
                        }
                    }
                    Err(error) => {
                        self.error =
                            Some(AppError::new("保存默认 Runner 失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::CreatePrefix(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(AppError::new("请先保存游戏，再创建 Prefix"));
                    return iced::Task::none();
                }
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(AppError::new(error));
                        return iced::Task::none();
                    }
                };
                let plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return iced::Task::none();
                    }
                };
                let dxvk = self.dxvk_dir.clone().map(|root| prefix::DxvkSource { root });
                self.pending.prefix.insert(game_id);
                iced::Task::perform(tasks::create_prefix(plan, dxvk), move |result| {
                    Message::PrefixCreated { game_id, result }
                })
            }
            Message::PrefixCreated { game_id, result } => {
                self.pending.prefix.remove(&game_id);
                match result {
                    Ok(path) => {
                        if let Some(draft) = &mut self.draft {
                            if draft.id == game_id {
                                draft.prefix_path = Some(path);
                            }
                        }
                        return self.refresh_prefix_status(game_id);
                    }
                    Err(error) => {
                        self.error =
                            Some(AppError::new("创建 Prefix 失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::InstallDxvk(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(AppError::new("请先保存游戏，再安装 DXVK"));
                    return iced::Task::none();
                }
                let Some(dxvk_dir) = self.dxvk_dir.clone() else {
                    self.error = Some(AppError::new("请先选择 DXVK 目录"));
                    return iced::Task::none();
                };
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(AppError::new(error));
                        return iced::Task::none();
                    }
                };
                let plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return iced::Task::none();
                    }
                };
                self.pending.dxvk.insert(game_id);
                iced::Task::perform(
                    tasks::install_dxvk(plan, prefix::DxvkSource { root: dxvk_dir }),
                    move |result| Message::DxvkInstalled { game_id, result },
                )
            }
            Message::DxvkInstalled { game_id, result } => {
                self.pending.dxvk.remove(&game_id);
                if let Err(error) = result {
                    self.error =
                        Some(AppError::new("安装 DXVK 失败").with_detail(error));
                }
                iced::Task::none()
            }
            Message::RefreshPrefixStatus(game_id) => self.refresh_prefix_status(game_id),
            Message::PrefixStatusLoaded { game_id, status } => {
                if self.selected_game == Some(game_id) {
                    self.prefix_status = Some(status);
                }
                iced::Task::none()
            }
            Message::Settings(action) => {
                let affects_prefix = matches!(
                    &action,
                    SettingsAction::DefaultRunner(_) | SettingsAction::PrefixRoot(_)
                );
                match action {
                    SettingsAction::Env(action) => {
                        crate::draft::apply_env_action(
                            &mut self.settings.global_env_rows,
                            action,
                        );
                    }
                    SettingsAction::DefaultRunner(id) => {
                        self.settings.default_runner_id = id;
                    }
                    SettingsAction::PrefixRoot(path) => {
                        self.settings.prefix_root = path;
                    }
                }
                match (affects_prefix, self.selected_game) {
                    (true, Some(id)) => {
                        self.prefix_status = None;
                        self.refresh_prefix_status(id)
                    }
                    _ => iced::Task::none(),
                }
            }
            Message::SaveSettings => {
                let values = match self.settings.to_map() {
                    Ok(values) => values,
                    Err(error) => {
                        self.error =
                            Some(AppError::new("设置校验失败").with_detail(error));
                        return iced::Task::none();
                    }
                };
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.settings_saving = true;
                iced::Task::perform(tasks::save_settings(db, values), Message::SettingsSaved)
            }
            Message::SettingsSaved(result) => {
                self.pending.settings_saving = false;
                match result {
                    Ok(()) => {
                        self.error = None;
                    }
                    Err(error) => {
                        self.error =
                            Some(AppError::new("保存设置失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
            Message::ConfirmDelete(target) => {
                self.confirming = Some(target);
                iced::Task::none()
            }
            Message::CancelDelete => {
                self.confirming = None;
                iced::Task::none()
            }
            Message::DeleteConfirmed => {
                let Some(target) = self.confirming.clone() else {
                    return iced::Task::none();
                };
                if let DeleteTarget::Prefix { game_id, path } = &target {
                    let saved = self
                        .games
                        .iter()
                        .find(|game| game.id == *game_id)
                        .map(|game| {
                            game.prefix_path.clone().unwrap_or_else(|| {
                                self.settings.prefix_root.join(game_id.to_string())
                            })
                        });
                    if saved.as_deref() != Some(path.as_path()) {
                        self.confirming = None;
                        self.error = Some(AppError::new(
                            "Prefix 变更尚未保存，请先保存游戏后再删除",
                        ));
                        return iced::Task::none();
                    }
                }
                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                self.pending.deleting = true;
                let running = self.running.clone();
                let default_runner_id = self.settings.default_runner_id;
                iced::Task::perform(
                    tasks::delete_target(db, target.clone(), running, default_runner_id),
                    move |result| Message::DeleteFinished { target, result },
                )
            }
            Message::DeleteFinished { target, result } => {
                self.pending.deleting = false;
                self.confirming = None;
                match result {
                    Ok(()) => {
                        self.prefix_status = None;
                        match target {
                            DeleteTarget::Runner(id) => {
                                self.runners.retain(|runner| runner.id != id);
                                if self.settings.default_runner_id == Some(id) {
                                    self.settings.default_runner_id = None;
                                }
                                if let Some(draft) = &mut self.draft {
                                    if draft.runner_id == Some(id) {
                                        draft.runner_id = None;
                                    }
                                }
                            }
                            DeleteTarget::Prefix { game_id, path } => {
                                if let Some(draft) = &mut self.draft {
                                    if draft.id == game_id {
                                        draft.prefix_path = None;
                                    }
                                }
                                if let Some(game) =
                                    self.games.iter_mut().find(|game| game.id == game_id)
                                {
                                    if game.prefix_path.as_deref() == Some(path.as_path()) {
                                        game.prefix_path = None;
                                    }
                                }
                            }
                            DeleteTarget::Game(id) => {
                                self.games.retain(|game| game.id != id);
                                if self.selected_game == Some(id) {
                                    self.selected_game = None;
                                    self.draft = None;
                                    self.screen = Screen::Library;
                                }
                            }
                        }
                        self.error = None;
                        if let Some(id) = self.selected_game {
                            return self.refresh_prefix_status(id);
                        }
                    }
                    Err(error) => {
                        self.error =
                            Some(AppError::new("删除失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
```

注意 `env_rows_to_map` 需要 `pub(crate)`；`SettingsAction::Env` 使用 `crate::draft::EnvAction`。

- [x] **Step 5: 编译**

Run: `cargo build -p ui`
Expected: 成功。

- [x] **Step 6: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): wire runner, prefix, settings and deletion handlers"
```

---

## Task U9: RunnerManager 与 Settings 表单

**Files:**
- Modify: `crates/ui/src/widgets.rs`
- Modify: `crates/ui/src/views/runners.rs`
- Modify: `crates/ui/src/views/settings.rs`
- Modify: `crates/ui/src/app.rs`（确认条）

- [x] **Step 1: 在 `widgets.rs` 增加确认条与 runner 选项**

```rust
use crate::app::DeleteTarget;

pub fn confirm_bar(target: &DeleteTarget) -> Element<'_, Message> {
    row![
        text(format!("确认删除 {}？此操作不可撤销。", target.label())),
        iced::widget::horizontal_space(),
        button("确认删除").on_press(Message::DeleteConfirmed),
        button("取消").on_press(Message::CancelDelete),
    ]
    .spacing(8)
    .padding(8)
    .into()
}

pub fn runner_choices(
    runners: &[launcher_core::Runner],
    none_label: &str,
) -> Vec<Choice<Option<uuid::Uuid>>> {
    let mut choices = vec![Choice::new(None, none_label)];
    choices.extend(
        runners
            .iter()
            .map(|runner| Choice::new(Some(runner.id), runner.name.clone())),
    );
    choices
}

pub fn runner_choice(
    runners: &[launcher_core::Runner],
    none_label: &str,
    selected: Option<uuid::Uuid>,
) -> Choice<Option<uuid::Uuid>> {
    match selected {
        Some(id) => runners
            .iter()
            .find(|runner| runner.id == id)
            .map(|runner| Choice::new(Some(id), runner.name.clone()))
            .unwrap_or_else(|| Choice::new(None, none_label)),
        None => Choice::new(None, none_label),
    }
}
```

- [x] **Step 2: `App::view` 渲染确认条**

把 `view` 的末尾改为：

```rust
        let mut content = match &self.error {
            Some(error) => iced::widget::column![
                crate::widgets::error_banner(error),
                screen
            ]
            .spacing(8)
            .padding(8)
            .into(),
            None => screen,
        };

        if let Some(target) = &self.confirming {
            content = iced::widget::column![
                crate::widgets::confirm_bar(target),
                content
            ]
            .spacing(8)
            .padding(8)
            .into();
        }

        content
```

- [x] **Step 3: 替换 `views/runners.rs`**

```rust
use crate::app::{App, DeleteTarget, Message, Screen};
use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("Runner 管理").size(24),
        iced::widget::horizontal_space(),
        button("重新探测系统 Wine").on_press_maybe(
            (!app.pending.detecting_wine).then_some(Message::DetectSystemWine)
        ),
        button("导入 Proton 目录").on_press_maybe(
            (!app.pending.importing_proton).then_some(Message::ImportProton)
        ),
    ]
    .spacing(8);

    let mut list = column![].spacing(10);
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
                text(if runner.verified { "已验证" } else { "未验证" }).size(12),
                text(if is_default { "默认" } else { "" }).size(12),
                iced::widget::horizontal_space(),
                button("设为默认").on_press_maybe(
                    (!is_default).then_some(Message::SetDefaultRunner(Some(runner.id)))
                ),
                button("删除").on_press(Message::ConfirmDelete(
                    DeleteTarget::Runner(runner.id)
                )),
            ]
            .spacing(8),
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

- [x] **Step 4: 替换 `views/settings.rs`**

```rust
use crate::app::{App, Message, Screen, SettingsAction};
use crate::draft::EnvAction;
use crate::widgets::{runner_choice, runner_choices, section};
use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};

pub fn view(app: &App) -> Element<'_, Message> {
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text("设置").size(24),
        iced::widget::horizontal_space(),
        button("保存").on_press_maybe(
            (!app.pending.settings_saving).then_some(Message::SaveSettings)
        ),
    ]
    .spacing(8);

    let default_runner = section(
        "默认 Runner",
        pick_list(
            runner_choices(&app.runners, "无默认（使用游戏级选择）"),
            Some(runner_choice(
                &app.runners,
                "无默认（使用游戏级选择）",
                app.settings.default_runner_id,
            )),
            |choice| {
                Message::Settings(SettingsAction::DefaultRunner(choice.value))
            },
        ),
    );

    let prefix_root = section(
        "Prefix 根目录",
        column![
            text(app.settings.prefix_root.display().to_string()),
            button("选择目录").on_press(Message::Pick(
                crate::dialogs::DialogIntent::PickPrefixRoot
            )),
        ]
        .spacing(8),
    );

    let mut env_rows = column![].spacing(8);
    for (index, entry) in app.settings.global_env_rows.iter().enumerate() {
        env_rows = env_rows.push(
            row![
                text_input("KEY", entry.key.as_str())
                    .on_input(move |key| Message::Settings(SettingsAction::Env(
                        EnvAction::Key { index, key }
                    )))
                    .width(Length::FillPortion(2)),
                text_input("VALUE", entry.value.as_str())
                    .on_input(move |value| Message::Settings(SettingsAction::Env(
                        EnvAction::Value { index, value }
                    )))
                    .width(Length::FillPortion(3)),
                button("删除").on_press(Message::Settings(SettingsAction::Env(
                    EnvAction::Remove(index)
                ))),
            ]
            .spacing(8),
        );
    }
    env_rows = env_rows.push(
        button("添加变量").on_press(Message::Settings(SettingsAction::Env(EnvAction::Add))),
    );

    let global_env = section("全局环境变量", env_rows);

    let content = column![
        header,
        default_runner,
        prefix_root,
        global_env,
    ]
    .spacing(12)
    .padding(20);

    scrollable(content).height(Length::Fill).into()
}
```

- [x] **Step 5: 编译**

Run: `cargo build -p ui`
Expected: 成功。

- [x] **Step 6: 手工验证**

Run: `cargo run -p ui`：

1. Runner 管理 → “重新探测系统 Wine”，列表出现 `wine-10.0`。
2. 导入一个 GE-Proton 目录（可选，没有可跳过），列表出现。
3. “设为默认”后重启仍保留。
4. 设置 → 添加全局环境变量 → 保存 → 重启仍在。
5. 删除被引用的 Runner 会被拒绝；默认 Runner 删除被拒绝。

- [x] **Step 7: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add runner manager and settings forms"
```

---

## Task U10: GameDetail 的 Runner/Prefix/DXVK 区块

**Files:**
- Modify: `crates/ui/src/views/game_detail.rs`

- [x] **Step 1: 在现有 `basic` 之后追加兼容层区块**

```rust
    let runner_section = {
        let none_label = "使用默认 Runner";
        let choices = runner_choices(&app.runners, none_label);
        let selected = runner_choice(&app.runners, none_label, draft.runner_id);

        let default_prefix = app
            .settings
            .prefix_root
            .join(draft.id.to_string());
        let prefix_path = draft.prefix_path.clone().unwrap_or(default_prefix);
        let status_text = prefix_status_label(app.prefix_status.as_ref());

        let mut buttons = row![
            button("选择 Prefix 目录").on_press(Message::Pick(
                DialogIntent::PickPrefixDir
            )),
            button("清除").on_press_maybe(
                draft
                    .prefix_path
                    .is_some()
                    .then_some(Message::Draft(DraftAction::ClearPrefix))
            ),
            button("创建 / 初始化").on_press_maybe(
                (!draft.is_new
                    && !app.pending.prefix.contains(&draft.id))
                .then_some(Message::CreatePrefix(draft.id))
            ),
            button("删除 Prefix").on_press_maybe(
                (!draft.is_new
                    && matches!(
                        app.prefix_status,
                        Some(PrefixStatus::Initialized | PrefixStatus::ArchMismatch { .. })
                    ))
                .then_some(Message::ConfirmDelete(DeleteTarget::Prefix {
                    game_id: draft.id,
                    path: prefix_path.clone(),
                }))
            ),
        ]
        .spacing(8);

        if draft.is_new {
            buttons = buttons.push(text("保存游戏后才能创建 Prefix").size(12));
        }

        let dxvk_text = crate::widgets::path_text(app.dxvk_dir.as_deref());
        let mut content = column![
            row![
                text("Runner："),
                pick_list(choices, Some(selected), |choice| {
                    Message::Draft(DraftAction::Runner(choice.value))
                }),
            ]
            .spacing(8),
            text(format!("Prefix：{}", prefix_path.display())),
            text(format!("状态：{status_text}")),
            buttons,
            row![
                text(format!("DXVK 目录：{dxvk_text}")),
                button("选择目录").on_press(Message::Pick(
                    DialogIntent::PickDxvkDir
                )),
                button("安装 DXVK").on_press_maybe(
                    (!draft.is_new
                        && app.dxvk_dir.is_some()
                        && !app.pending.dxvk.contains(&draft.id))
                    .then_some(Message::InstallDxvk(draft.id))
                ),
            ]
            .spacing(8),
        ]
        .spacing(10);

        if let Some(status) = &app.prefix_status {
            if let PrefixStatus::ArchMismatch { existing } = status {
                content = content.push(text(format!(
                    "架构不匹配：现有 prefix 为 {existing:?}，请更换 Prefix 或删除后重建"
                )));
            }
        }

        content
    };

    let content = column![
        header,
        section("基本信息", basic),
        section("兼容层与 Prefix", runner_section),
    ]
    .spacing(12)
    .padding(20);
```

文件顶部把现有 `use crate::app::{App, Message, Screen};` 改为：

```rust
use crate::app::{App, DeleteTarget, Message, Screen};
use crate::widgets::{runner_choice, runner_choices};
use prefix::PrefixStatus;
```

不要重复导入 `Message`。

并实现：

```rust
fn prefix_status_label(status: Option<&PrefixStatus>) -> &'static str {
    match status {
        None => "未知（未解析 Runner 或未加载）",
        Some(PrefixStatus::Missing) => "未创建",
        Some(PrefixStatus::NotInitialized) => "目录存在，未初始化",
        Some(PrefixStatus::Initialized) => "已初始化",
        Some(PrefixStatus::ArchMismatch { .. }) => "架构不匹配",
    }
}
```

注意：`prefix_status` 在 `SelectGame` 和保存后自动刷新；新建草稿尚未保存时不会解析。

- [x] **Step 2: 编译**

Run: `cargo build -p ui`
Expected: 成功。若 `PrefixStatus` 不是 `Copy`，`app.prefix_status.as_ref()` 仍可用。

- [x] **Step 3: 手工验证**

1. 打开已保存游戏 → Runner 选择、默认 Runner 选项出现。
2. 选择系统 Wine → “创建 / 初始化” → 状态从“未创建”变为“已初始化”（本机 Wine 10.0）。
3. 选择 DXVK 目录 → 安装，成功后不报错；DLL 出现在 prefix 对应目录。
4. 删除共享 prefix 被拒绝；删除默认 Runner 被拒绝。
5. 新建草稿时 prefix 按钮禁用并提示先保存。

- [x] **Step 4: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add runner, prefix and dxvk controls to game detail"
```

---

## Task U11: 删除与设置任务测试

**Files:**
- Modify: `crates/ui/src/tasks.rs`
- Modify: `CHANGELOG.md`

- [x] **Step 1: 追加测试**

```rust
    use launcher_core::{RunnerKind, RunnerSource};
    use std::collections::HashMap;

    fn runner_value(path: PathBuf) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: "Wine".into(),
            binary_path: path,
            root_dir: PathBuf::from("/usr/bin"),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    #[tokio::test]
    async fn persist_runners_updates_existing_binary_path() {
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let runner = runner_value(PathBuf::from("/usr/bin/wine"));
        let stored = persist_runners(db.clone(), vec![runner.clone()])
            .await
            .unwrap();
        assert_eq!(stored.len(), 1);

        let mut duplicate = Runner {
            id: Uuid::new_v4(),
            ..runner
        };
        duplicate.version_string = Some("wine-10.0".into());
        let stored = persist_runners(db, vec![duplicate]).await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].version_string.as_deref(), Some("wine-10.0"));
    }

    #[tokio::test]
    async fn settings_round_trip_through_tasks() {
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let values = BTreeMap::from([
            (
                crate::app::SETTING_GLOBAL_ENV.to_string(),
                Some(r#"{"DXVK_HUD":"fps"}"#.to_string()),
            ),
            (
                crate::app::SETTING_PREFIX_ROOT.to_string(),
                Some("/tmp/prefixes".to_string()),
            ),
            (crate::app::SETTING_DEFAULT_RUNNER_ID.to_string(), None),
        ]);
        save_settings(db.clone(), values).await.unwrap();

        let stored = db.lock().unwrap().get_all_settings().unwrap();
        assert_eq!(stored[crate::app::SETTING_GLOBAL_ENV], r#"{"DXVK_HUD":"fps"}"#);
        assert_eq!(stored[crate::app::SETTING_PREFIX_ROOT], "/tmp/prefixes");
        assert!(!stored.contains_key(crate::app::SETTING_DEFAULT_RUNNER_ID));
    }

    #[tokio::test]
    async fn deleting_single_use_prefix_clears_game_reference_and_directory() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        let db = Db::open_in_memory().unwrap();
        let mut value = game(exe);
        value.prefix_path = Some(prefix.clone());
        db.insert_game(&value).unwrap();
        let db = Arc::new(Mutex::new(db));

        delete_target(
            db.clone(),
            DeleteTarget::Prefix {
                game_id: value.id,
                path: prefix.clone(),
            },
            HashMap::new(),
            None,
        )
        .await
        .unwrap();

        assert!(!prefix.exists());
        assert_eq!(
            db.lock().unwrap().get_game(value.id).unwrap().unwrap().prefix_path,
            None
        );
    }

    #[tokio::test]
    async fn deleting_shared_prefix_is_rejected() {
        let dir = tempdir().unwrap();
        let exe_a = dir.path().join("a.exe");
        let exe_b = dir.path().join("b.exe");
        fs::write(&exe_a, b"MZ").unwrap();
        fs::write(&exe_b, b"MZ").unwrap();
        let prefix = dir.path().join("shared");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        let db = Db::open_in_memory().unwrap();
        let mut a = game(exe_a);
        a.prefix_path = Some(prefix.clone());
        let mut b = game(exe_b);
        b.prefix_path = Some(prefix.clone());
        db.insert_game(&a).unwrap();
        db.insert_game(&b).unwrap();
        let db = Arc::new(Mutex::new(db));

        let result = delete_target(
            db,
            DeleteTarget::Prefix {
                game_id: a.id,
                path: prefix.clone(),
            },
            HashMap::new(),
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(prefix.exists());
    }

    #[tokio::test]
    async fn deleting_referenced_or_default_runner_is_rejected() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let db = Db::open_in_memory().unwrap();
        let runner = runner_value(dir.path().join("wine"));
        let default_only = runner_value(dir.path().join("wine-default"));
        db.insert_runner(&runner).unwrap();
        db.insert_runner(&default_only).unwrap();
        let mut value = game(exe);
        value.runner_id = Some(runner.id);
        db.insert_game(&value).unwrap();
        let db = Arc::new(Mutex::new(db));

        assert!(delete_target(
            db.clone(),
            DeleteTarget::Runner(runner.id),
            HashMap::new(),
            None,
        )
        .await
        .is_err());

        assert!(delete_target(
            db,
            DeleteTarget::Runner(default_only.id),
            HashMap::new(),
            Some(default_only.id),
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn deleting_game_removes_row_and_rejects_running() {
        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();
        let db = Db::open_in_memory().unwrap();
        let value = game(exe);
        db.insert_game(&value).unwrap();
        let db = Arc::new(Mutex::new(db));

        let mut running = HashMap::new();
        running.insert(value.id, PathBuf::from("/tmp/prefix"));
        assert!(delete_target(
            db.clone(),
            DeleteTarget::Game(value.id),
            running,
            None,
        )
        .await
        .is_err());

        delete_target(
            db.clone(),
            DeleteTarget::Game(value.id),
            HashMap::new(),
            None,
        )
        .await
        .unwrap();
        assert!(db.lock().unwrap().get_game(value.id).unwrap().is_none());
    }
```

`game` helper 已有；`Runner` 类型在文件顶部已导入。`HashMap` 导入在测试模块内。

- [x] **Step 2: 运行测试**

Run: `cargo test -p ui`
Expected: 全部通过（24+）。

- [x] **Step 3: 更新 CHANGELOG**

```markdown
## P8 — Runner / Prefix / Settings UI

- 系统 Wine 重新探测、Proton 目录导入、默认 Runner 持久化。
- Prefix 状态刷新、创建/初始化、DXVK 目录选择与安装。
- 全局环境变量、默认 Runner、Prefix 根目录设置表单。
- Runner/Prefix/Game 删除确认与引用保护。
- 删除共享 prefix、被引用 runner、默认 runner 均被拒绝。

验证：`cargo test -p ui`；本机 Wine 10.0 创建 prefix 成功。
```

- [x] **Step 4: 提交**

```bash
git add crates/ui CHANGELOG.md
git commit -m "test(ui): cover deletion guards and settings tasks"
```

---

## P8 验收

- [x] `cargo test -p ui` 通过。
- [x] 断网状态下“重新探测系统 Wine”得到本机 `wine-10.0`。
- [x] 导入 Proton 目录后 `verified=true`，重启仍在。（本机无真实 Proton，使用合成
  Proton 目录验证，见实施记录。）
- [x] 默认 Runner 保存后重启保留。
- [x] 为已保存游戏创建 prefix，UI 不冻结；Proton 使用 `pfx/`。
- [x] DXVK 安装后文件进入正确目录并写入 registry override。
- [x] 删除被游戏引用的 Runner、默认 Runner 被拒绝。
- [x] 删除共享 prefix 被拒绝；删除单游戏 prefix 会清除引用并删除目录。
- [x] 游戏运行中不能删除其 prefix 或游戏记录。（删除保护由单元测试覆盖；运行中
  状态在 P9 接入后手工复核。）

## 实施记录

- 环境：Wine `wine-10.0 (Debian 10.0~repack-6)`；本机没有真实 Proton，使用
  合成 Proton 目录 `ge-proton9-99`（`proton` 脚本仅写 `pfx/system.reg`，
  `version` 为 `GE-Proton9-99`）走完导入 → 重启持久化 → 创建 prefix 的 UI
  流程；DXVK 3.1.1（`~/.config/heroic/tools/dxvk/dxvk-3.1.1`）。
- Prefix 创建耗时：Wine 约 11 秒（阻塞任务线程，UI 保持响应）；合成 Proton
  立即返回。Proton prefix 的 registry 位于 `pfx/system.reg`，与规格一致。
- 测试：`cargo test -p ui` 23 passed（P7 16 + 规格 U11 6 + 额外
  `deleting_running_prefix_is_rejected` 1）；`cargo test --workspace`
  153 passed，1 ignored；fmt/clippy/build 全量通过。
- 手工验证（XWayland + XTEST 点击、桌面 portal 截图）：
  - Runner 管理：断网重新探测得到 `wine-10.0`；设为默认与取消默认均即时
    生效；导入合成 Proton 后显示“已验证”，重启后仍在；被游戏引用和全局默认
    的 Runner 删除均弹出中文拒绝原因。
  - Prefix：已保存游戏的“未创建 → 已初始化”状态刷新正确；创建后自动写入
    draft 的 prefix 路径，需再“保存”才持久化；删除单游戏 Prefix 会删除目录、
    清空 DB 引用并回到“未创建”；长路径确认条会换行，按钮不再被挤压。
  - DXVK：选择 `dxvk-3.1.1` 安装成功后，d3d11/dxgi 等 DLL 出现在
    `system32`/`syswow64`，`user.reg` 写入 `"d3d11"="native,builtin"`。
  - 设置：全局环境变量（含大写键名 `DXVK_HUD`）保存后重启保留；默认 Runner
    与 Prefix 根目录展示正确。
- 偏差：
  - `WineDetected` 使用 `sort_by_key`（clippy `unnecessary_sort_by`）。
  - `iced::Task::perform` 的完成闭包是 `Fn`，`DeleteFinished` 需要
    `target.clone()`。
  - 手工验证发现长 Prefix 路径把确认条按钮挤成窄条，改为 Fill +
    `WordOrGlyph` 换行。
  - 规格 U11 预期 `cargo test -p ui` 24+，实际基线 16 + 7 = 23；额外测试
    覆盖运行中 Prefix 删除保护。
  - 运行中删除保护无法在 UI 手工触达（running map 在 P9 才接入），由
    `deleting_game_removes_row_and_rejects_running` 与
    `deleting_running_prefix_is_rejected` 覆盖。
  - 合成 Proton 只验证导入/持久化/`pfx` 路径；Proton 下的 DXVK 安装沿用
    共享实现，未额外手工验证。

## 当前实现（v0.9.0）

P8 的状态与任务层在后续版本中迁入 `crates/application`，UI 仅保留消息与视图。

### 架构变化

- `crates/ui/src/tasks.rs`、`draft.rs` 已删除；业务用例由 `AppService`
  （`crates/application/src/service.rs`）提供，按领域拆成
  `runners`、`prefixes`、`components`、`settings`、`games`、`delete`、
  `launching`、`data` 模块。
- `App.service: Option<AppService>` 取代 `App.db: Option<Db>`；初始化数据
  通过 `AppService::load_initial()` 返回 `LoadedData`
  （`games`、`runners`、`settings_map`、`session`、`monitors`、
  `gamescope_path`、`mangohud_path`、`winetricks_path`）。
- 草稿与行编辑器位于 `application::editor`：`GameDraft`、`EnvRow`、
  `DllRow`、`DraftAction`、`EnvAction`/`DllAction`/`ArgsAction`、
  `env_rows_to_map`、`DraftError`。UI 的 `crate::draft` 路径不再存在。
- `DeleteTarget` 迁到 `application::delete`，引用检查（默认 Runner、
  `count_games_using_runner`、`count_games_using_prefix`）与应用层删除
  一起实现，UI 只发起确认。
- 运行态由 `AppService.running_games: HashMap<Uuid, RunningGame>` 持有：
  `RunningGame { process_group, runner: Option<Runner>, wine_prefix }`。
  UI 的 `running: HashMap<Uuid, Option<PathBuf>>` 只用于展示（原生游戏为
  `None`）。

### Runner 管理（views/runners.rs）

- 双列卡片：名称、Wine/Proton 徽章、已验证/未验证、默认标记、版本号。
- 探测/导入期间两个按钮同时禁用；删除按钮在“是默认 Runner”或
  “被 N 个游戏引用”时禁用，应用层删除时仍会二次校验。
- 所有文案经 i18n（zh-CN/en-US），不再硬编码中文。

### GameDetail 的兼容层与组件

- Wine 专属区块（Runner、Prefix、状态、DXVK、DLL Override、winetricks
  组件）只在 `draft.platform == Platform::Windows` 时渲染。Linux 原生页
  仅保留名称、可执行文件、工作目录、显示、环境变量与启动参数。
- DXVK 与 DXVK-NVAPI 来源分离：
  - DXVK 目录仍在详情页选择，点“安装 DXVK”（`Message::InstallDxvk`）
    立即复制到当前 prefix。
  - DXVK-NVAPI 来源在设置页配置（`dxvk_nvapi_source`）；启动时若
    `enable_dxvk_nvapi` 为真且 runner 为 Wine，`AppService::launch_game`
    先调用 `prefix::install_dxvk_nvapi`，标记文件为
    `.gamehub-dxvk-nvapi`。未配置来源时启动返回
    `DxvkNvapiSourceMissing`。
- winetricks 组件：
  - 只使用系统 `PATH` 中的 winetricks（`prefix::winetricks_path()`），
    不打包、不下载；缺失时详情页提示且安装按钮不可用。
  - 已安装列表来自 prefix 内 `winetricks.log`
    （`prefix::installed_verbs`），按当前 prefix 刷新。
  - 预设 verb 下拉 + 自定义 verb 输入；`prefix::normalize_verb` 校验，
    非法 verb 返回 `ComponentVerbInvalid`。
  - 需要游戏已保存且 prefix 为 `Initialized`；同一游戏同时只允许一个
    组件任务，且同一 prefix 上不能与运行中的游戏或其他组件任务并发。
  - 输出流式写入 `logs/winetricks/<game-id>.log`，UI 显示尾部
    `COMPONENT_TAIL_LINES = 400` 行。
  - 取消通过 `terminate_process_group` 终止 winetricks 及其子进程，
    取消后刷新已安装列表。

### 设置页（views/settings.rs）

- 卡片顺序：外观、默认 Runner、Prefix 根目录、全局环境变量、兼容性。
- 外观卡片：
  - 主题为 Light/Dark 分段控件，默认 Light；
  - 语言为 `Auto`/`zh-CN`/`en-US`；
  - 字体面板通过 `fc-list` 扫描已安装字体，支持搜索、“仅含 CJK”过滤、
    预览，列表上限 `FONT_LIST_LIMIT = 80`。
  - 主题、语言、字体选择后立即单独保存
    （`SETTING_UI_THEME`/`SETTING_UI_LANGUAGE`/`SETTING_UI_FONT`），
    批量“保存”会跳过这三项；保存失败按 `NoticeKind::{ThemeSave,
    LanguageSave, FontSave}` 显示对应提示。
- 兼容性卡片：
  - esync/fsync/MangoHud/自动安装 DXVK-NVAPI 四个开关；
  - MangoHud 检测路径展示；开启但未找到 mangohud 时启动会被拒绝
    （`MangoHudMissing`）；
  - DXVK-NVAPI 目录选择/清除；开启自动安装但未选目录时显示警告徽章。
- 全局环境变量仍为行编辑并序列化为 `global_env` JSON；保存前校验
  键名、拒绝 `WINEDLLOVERRIDES`、Prefix 根目录必须为绝对路径。
- 批量设置写入 `enable_esync`、`enable_fsync`、`enable_mangohud`、
  `auto_install_dxvk_nvapi`、`dxvk_nvapi_source`、`global_env`、
  `default_runner_id`、`prefix_root`。
