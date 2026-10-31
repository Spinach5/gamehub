# P9 配置编辑与启动打通 Implementation Plan

> 历史实施计划（2026-10-27，P9）。Task U12–U15 的代码片段与验收记录保留原样；
> 与当前代码不一致的契约、文件路径和流程以文末「当前实现（v0.10.0）」为准。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成 GameDetail 的显示、环境变量、DLL override、启动参数编辑；打通启动、运行中状态、`last_played_at` 更新、退出码/错误展示、游戏删除。

**Architecture:** 草稿继续承载全部表单；`DraftAction` 追加显示/环境/DLL/参数动作。启动时用草稿（允许未保存的本次修改）构造 `LaunchRequest`，但全局环境使用已保存设置；`tasks::launch_game` 在 `spawn_blocking` 中构建计划、异步等待进程、再写回 `last_played_at`。

**Tech Stack:** Iced 0.13、launch、prefix、storage、tokio、chrono。

**前置条件：** P8 完成；P6 已实现。

---

## 本阶段追加的契约

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub enum DllAction {
    Add,
    Remove(usize),
    Name { index: usize, name: String },
    Mode { index: usize, mode: OverrideMode },
}

#[derive(Debug, Clone)]
pub enum ArgsAction {
    Add,
    Remove(usize),
    Value { index: usize, value: String },
}
```

`DraftAction` 追加：

```rust
Display(DisplayConfig),
ResolutionWidth(String),
ResolutionHeight(String),
RefreshRate(String),
Env(crate::draft::EnvAction),
Dll(DllAction),
Args(ArgsAction),
```

`Message` 追加：

```rust
Launch(Uuid),
LaunchFinished {
    game_id: Uuid,
    result: Result<LaunchOutcome, String>,
},
```

---

## Task U12: 草稿动作扩展

**Files:**
- Modify: `crates/ui/src/draft.rs`
- Modify: `crates/ui/src/app.rs`

- [x] **Step 1: 在 `draft.rs` 追加枚举**

```rust
#[derive(Debug, Clone)]
pub enum DllAction {
    Add,
    Remove(usize),
    Name { index: usize, name: String },
    Mode { index: usize, mode: OverrideMode },
}

#[derive(Debug, Clone)]
pub enum ArgsAction {
    Add,
    Remove(usize),
    Value { index: usize, value: String },
}

pub fn apply_dll_action(rows: &mut Vec<DllRow>, action: DllAction) {
    match action {
        DllAction::Add => rows.push(DllRow {
            name: String::new(),
            mode: OverrideMode::NativeBuiltin,
        }),
        DllAction::Remove(index) => {
            if index < rows.len() {
                rows.remove(index);
            }
        }
        DllAction::Name { index, name } => {
            if let Some(row) = rows.get_mut(index) {
                row.name = name;
            }
        }
        DllAction::Mode { index, mode } => {
            if let Some(row) = rows.get_mut(index) {
                row.mode = mode;
            }
        }
    }
}

pub fn apply_args_action(rows: &mut Vec<String>, action: ArgsAction) {
    match action {
        ArgsAction::Add => rows.push(String::new()),
        ArgsAction::Remove(index) => {
            if index < rows.len() {
                rows.remove(index);
            }
        }
        ArgsAction::Value { index, value } => {
            if let Some(row) = rows.get_mut(index) {
                *row = value;
            }
        }
    }
}
```

`DraftAction` 追加：

```rust
    Display(DisplayConfig),
    ResolutionWidth(String),
    ResolutionHeight(String),
    RefreshRate(String),
    Env(EnvAction),
    Dll(DllAction),
    Args(ArgsAction),
```

`GameDraft::apply` 追加：

```rust
            DraftAction::Display(display) => self.display = display,
            DraftAction::ResolutionWidth(value) => self.resolution_width = value,
            DraftAction::ResolutionHeight(value) => self.resolution_height = value,
            DraftAction::RefreshRate(value) => self.refresh_rate = value,
            DraftAction::Env(action) => apply_env_action(&mut self.env_rows, action),
            DraftAction::Dll(action) => apply_dll_action(&mut self.dll_rows, action),
            DraftAction::Args(action) => apply_args_action(&mut self.args_rows, action),
```

- [x] **Step 2: 追加动作测试**

```rust
    #[test]
    fn env_actions_add_edit_and_remove() {
        let mut rows = Vec::new();
        apply_env_action(&mut rows, EnvAction::Add);
        apply_env_action(
            &mut rows,
            EnvAction::Key {
                index: 0,
                key: "A".into(),
            },
        );
        apply_env_action(
            &mut rows,
            EnvAction::Value {
                index: 0,
                value: "1".into(),
            },
        );
        assert_eq!(rows[0].key, "A");
        assert_eq!(rows[0].value, "1");
        apply_env_action(&mut rows, EnvAction::Remove(0));
        assert!(rows.is_empty());
    }

    #[test]
    fn dll_and_args_actions_are_bounded() {
        let mut dlls = Vec::new();
        apply_dll_action(&mut dlls, DllAction::Add);
        apply_dll_action(
            &mut dlls,
            DllAction::Name {
                index: 0,
                name: "d3d11".into(),
            },
        );
        apply_dll_action(
            &mut dlls,
            DllAction::Mode {
                index: 0,
                mode: OverrideMode::Native,
            },
        );
        assert_eq!(dlls[0].name, "d3d11");
        assert_eq!(dlls[0].mode, OverrideMode::Native);
        apply_dll_action(&mut dlls, DllAction::Remove(99));
        assert_eq!(dlls.len(), 1);

        let mut args = Vec::new();
        apply_args_action(&mut args, ArgsAction::Add);
        apply_args_action(
            &mut args,
            ArgsAction::Value {
                index: 0,
                value: "-skip".into(),
            },
        );
        assert_eq!(args, vec!["-skip".to_string()]);
        apply_args_action(&mut args, ArgsAction::Remove(99));
        assert_eq!(args.len(), 1);
    }
```

- [x] **Step 3: 在 `app.rs` 增加 `LaunchOutcome` 与消息**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
}
```

`Message`：

```rust
    Launch(Uuid),
    LaunchFinished {
        game_id: Uuid,
        result: Result<LaunchOutcome, String>,
    },
```

- [x] **Step 4: 运行测试**

Run: `cargo test -p ui draft`
Expected: 14 passed（12 + 2）。

- [x] **Step 5: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add display, env, dll and args draft actions"
```

---

## Task U13: 显示 / 环境 / DLL / 参数视图

**Files:**
- Modify: `crates/ui/src/widgets.rs`
- Modify: `crates/ui/src/views/game_detail.rs`

- [x] **Step 1: 在 `widgets.rs` 追加选项构造**

```rust
use display::X11Output;
use launcher_core::{DisplayBackend, GamescopeMode, OverrideMode};

pub fn backend_choices() -> Vec<Choice<DisplayBackend>> {
    vec![
        Choice::new(DisplayBackend::Auto, "自动"),
        Choice::new(DisplayBackend::X11, "X11"),
        Choice::new(DisplayBackend::Wayland, "Wayland"),
    ]
}

pub fn backend_choice(value: DisplayBackend) -> Choice<DisplayBackend> {
    backend_choices()
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(DisplayBackend::Auto, "自动"))
}

pub fn gamescope_choices() -> Vec<Choice<GamescopeMode>> {
    vec![
        Choice::new(GamescopeMode::Off, "关闭"),
        Choice::new(GamescopeMode::Auto, "自动（Wayland 且可用时）"),
        Choice::new(GamescopeMode::Forced, "强制"),
    ]
}

pub fn gamescope_choice(value: GamescopeMode) -> Choice<GamescopeMode> {
    gamescope_choices()
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(GamescopeMode::Auto, "自动（Wayland 且可用时）"))
}

pub fn override_mode_choices() -> Vec<Choice<OverrideMode>> {
    vec![
        Choice::new(OverrideMode::Disabled, "禁用 (disabled)"),
        Choice::new(OverrideMode::Native, "原生 (n)"),
        Choice::new(OverrideMode::Builtin, "内置 (b)"),
        Choice::new(OverrideMode::NativeBuiltin, "原生+内置 (n,b)"),
    ]
}

pub fn override_mode_choice(value: OverrideMode) -> Choice<OverrideMode> {
    override_mode_choices()
        .into_iter()
        .find(|choice| choice.value == value)
        .unwrap_or_else(|| Choice::new(OverrideMode::NativeBuiltin, "原生+内置 (n,b)"))
}

pub fn monitor_choices(outputs: &[X11Output]) -> Vec<Choice<Option<String>>> {
    let mut choices = vec![Choice::new(None, "（自动）")];
    choices.extend(
        outputs
            .iter()
            .filter(|output| output.connected)
            .map(|output| Choice::new(Some(output.name.clone()), output.name.clone())),
    );
    choices
}

pub fn monitor_choice(
    outputs: &[X11Output],
    selected: Option<String>,
) -> Choice<Option<String>> {
    match selected {
        Some(name) if outputs.iter().any(|output| output.connected && output.name == name) => {
            Choice::new(Some(name.clone()), name)
        }
        _ => Choice::new(None, "（自动）"),
    }
}
```

- [x] **Step 2: 在 `views/game_detail.rs` 追加区块**

在 `runner_section` 之后构造：

```rust
    let base_display = draft.display.clone();
    let gamescope_effective = app.gamescope_path.is_some()
        && display::should_use_gamescope(
            &draft.display,
            app.session,
            app.gamescope_path.is_some(),
        );

    let backend_pick = pick_list(
        crate::widgets::backend_choices(),
        Some(crate::widgets::backend_choice(draft.display.backend)),
        {
            let base = base_display.clone();
            move |choice| {
                let mut config = base.clone();
                config.backend = choice.value;
                Message::Draft(DraftAction::Display(config))
            }
        },
    );

    let gamescope_pick = pick_list(
        crate::widgets::gamescope_choices(),
        Some(crate::widgets::gamescope_choice(draft.display.gamescope)),
        {
            let base = base_display.clone();
            move |choice| {
                let mut config = base.clone();
                config.gamescope = choice.value;
                Message::Draft(DraftAction::Display(config))
            }
        },
    );

    let fullscreen = {
        let base = base_display.clone();
        iced::widget::checkbox("全屏", draft.display.fullscreen).on_toggle(move |value| {
            let mut config = base.clone();
            config.fullscreen = value;
            Message::Draft(DraftAction::Display(config))
        })
    };

    let monitor: iced::Element<'_, Message> = if app.session == display::SessionType::X11 {
        let base = base_display.clone();
        pick_list(
            crate::widgets::monitor_choices(&app.x11_outputs),
            Some(crate::widgets::monitor_choice(
                &app.x11_outputs,
                draft.display.monitor.clone(),
            )),
            move |choice| {
                let mut config = base.clone();
                config.monitor = choice.value;
                Message::Draft(DraftAction::Display(config))
            },
        )
        .into()
    } else {
        let value = draft.display.monitor.clone().unwrap_or_default();
        let base = base_display.clone();
        text_input("输出名（可留空）", &value)
            .on_input(move |value| {
                let mut config = base.clone();
                config.monitor = if value.trim().is_empty() {
                    None
                } else {
                    Some(value)
                };
                Message::Draft(DraftAction::Display(config))
            })
            .into()
    };

    let resolution_controls = if !gamescope_effective {
        column![text(
            "当前不会使用 gamescope：分辨率与刷新率不生效；X11 只设置 __GL_SYNC_DISPLAY_DEVICE。"
        )
        .size(12)]
    } else {
        column![
            row![
                text("分辨率："),
                text_input("宽", draft.resolution_width.as_str())
                    .on_input(|value| Message::Draft(DraftAction::ResolutionWidth(value)))
                    .width(Length::Fixed(100.0)),
                text("x"),
                text_input("高", draft.resolution_height.as_str())
                    .on_input(|value| Message::Draft(DraftAction::ResolutionHeight(value)))
                    .width(Length::Fixed(100.0)),
            ]
            .spacing(8),
            row![
                text("刷新率："),
                text_input("例如 144", draft.refresh_rate.as_str())
                    .on_input(|value| Message::Draft(DraftAction::RefreshRate(value)))
                    .width(Length::Fixed(120.0)),
            ]
            .spacing(8),
        ]
        .spacing(8)
    };

    let display_section = column![
        row![text("后端："), backend_pick].spacing(8),
        text("后端字段仅作记录；实际 X11/Wayland 以启动时的会话检测为准。").size(12),
        row![text("gamescope："), gamescope_pick].spacing(8),
        row![text("显示器："), monitor].spacing(8),
        fullscreen,
        resolution_controls,
    ]
    .spacing(8);

    let mut env_rows = column![].spacing(8);
    for (index, entry) in draft.env_rows.iter().enumerate() {
        env_rows = env_rows.push(
            row![
                text_input("KEY", entry.key.as_str())
                    .on_input(move |key| Message::Draft(DraftAction::Env(
                        crate::draft::EnvAction::Key { index, key }
                    )))
                    .width(Length::FillPortion(2)),
                text_input("VALUE", entry.value.as_str())
                    .on_input(move |value| Message::Draft(DraftAction::Env(
                        crate::draft::EnvAction::Value { index, value }
                    )))
                    .width(Length::FillPortion(3)),
                button("删除").on_press(Message::Draft(DraftAction::Env(
                    crate::draft::EnvAction::Remove(index)
                ))),
            ]
            .spacing(8),
        );
    }
    env_rows = env_rows.push(
        button("添加环境变量").on_press(Message::Draft(DraftAction::Env(
            crate::draft::EnvAction::Add
        ))),
    );

    let env_section = column![
        env_rows,
        text("WINEDLLOVERRIDES 请使用下方的结构化 DLL Override，不要手填。").size(12),
    ]
    .spacing(8);

    let mut dll_rows = column![].spacing(8);
    for (index, entry) in draft.dll_rows.iter().enumerate() {
        dll_rows = dll_rows.push(
            row![
                text_input("DLL 名称（不含 .dll）", entry.name.as_str())
                    .on_input(move |name| Message::Draft(DraftAction::Dll(
                        crate::draft::DllAction::Name { index, name }
                    )))
                    .width(Length::FillPortion(2)),
                pick_list(
                    crate::widgets::override_mode_choices(),
                    Some(crate::widgets::override_mode_choice(entry.mode)),
                    move |choice| Message::Draft(DraftAction::Dll(
                        crate::draft::DllAction::Mode {
                            index,
                            mode: choice.value,
                        }
                    )),
                ),
                button("删除").on_press(Message::Draft(DraftAction::Dll(
                    crate::draft::DllAction::Remove(index)
                ))),
            ]
            .spacing(8),
        );
    }
    dll_rows = dll_rows.push(
        button("添加 DLL Override").on_press(Message::Draft(DraftAction::Dll(
            crate::draft::DllAction::Add
        ))),
    );

    let mut args_rows = column![].spacing(8);
    for (index, argument) in draft.args_rows.iter().enumerate() {
        args_rows = args_rows.push(
            row![
                text_input("参数", argument.as_str())
                    .on_input(move |value| Message::Draft(DraftAction::Args(
                        crate::draft::ArgsAction::Value { index, value }
                    )))
                    .width(Length::Fill),
                button("删除").on_press(Message::Draft(DraftAction::Args(
                    crate::draft::ArgsAction::Remove(index)
                ))),
            ]
            .spacing(8),
        );
    }
    args_rows = args_rows.push(
        button("添加参数").on_press(Message::Draft(DraftAction::Args(
            crate::draft::ArgsAction::Add
        ))),
    );
```

把最终 `content` 改为：

```rust
    let content = column![
        header,
        section("基本信息", basic),
        section("兼容层与 Prefix", runner_section),
        section("显示", display_section),
        section("环境变量", env_section),
        section("DLL Override", dll_rows),
        section("启动参数", args_rows),
    ]
    .spacing(12)
    .padding(20);
```

文件顶部补充导入：

```rust
use iced::widget::{checkbox, pick_list, text_input};
```

（按实际已导入的 widget 合并，避免重复。）

- [x] **Step 3: 编译**

Run: `cargo build -p ui`
Expected: 成功。若 `checkbox` 不在 `iced::widget`，使用 `iced::widget::checkbox` 的完整路径；本机 iced 0.13 已在 U1 冒烟中确认 widget 可用。

- [x] **Step 4: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): add display, environment, dll and args editors"
```

---

## Task U14: 启动流程与运行状态

**Files:**
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/tasks.rs`
- Modify: `crates/ui/src/views/game_detail.rs`

- [x] **Step 1: 在 `tasks.rs` 增加启动任务**

```rust
use crate::app::LaunchOutcome;

pub async fn launch_game(
    db: Arc<Mutex<Db>>,
    request: launch::LaunchRequest,
) -> Result<LaunchOutcome, String> {
    let game_id = request.game.id;
    let plan_request = request.clone();

    let plan = tokio::task::spawn_blocking(move || launch::build_plan(&plan_request))
        .await
        .map_err(|error| format!("启动计划任务失败: {error}"))?
        .map_err(|error| error.to_string())?;

    let mut child = launch::spawn_plan(&plan)
        .await
        .map_err(|error| error.to_string())?;

    let wait_result = child.wait().await;
    let timestamp = chrono::Utc::now().timestamp();

    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(db) = db.lock() {
            let _ = db.touch_last_played(game_id, timestamp);
        }
    })
    .await;

    let status = wait_result.map_err(|error| format!("等待游戏进程失败: {error}"))?;
    Ok(LaunchOutcome {
        exit_code: status.code(),
        success: status.success(),
    })
}
```

- [x] **Step 2: 在 `app.rs` 增加消息 handler**

辅助函数：

```rust
    fn inherited_env() -> BTreeMap<String, String> {
        std::env::vars_os()
            .filter_map(|(key, value)| {
                Some((key.into_string().ok()?, value.into_string().ok()?))
            })
            .collect()
    }
```

`Launch`：

```rust
            Message::Launch(game_id) => {
                if self
                    .draft
                    .as_ref()
                    .is_some_and(|draft| draft.id == game_id && draft.is_new)
                {
                    self.error = Some(AppError::new("请先保存游戏，再启动"));
                    return iced::Task::none();
                }
                if self.running.contains_key(&game_id) {
                    self.error = Some(AppError::new("该游戏已在运行"));
                    return iced::Task::none();
                }
                let game = match self.game_for_actions(game_id) {
                    Ok(game) => game,
                    Err(error) => {
                        self.error = Some(AppError::new(error));
                        return iced::Task::none();
                    }
                };
                let prefix_plan = match self.prefix_plan_for(&game) {
                    Ok(plan) => plan,
                    Err(error) => {
                        self.error = Some(error);
                        return iced::Task::none();
                    }
                };
                if self
                    .running
                    .values()
                    .any(|path| path == &prefix_plan.path)
                {
                    self.error = Some(AppError::new(
                        "同一 Prefix 已有游戏在运行，请等待其退出",
                    ));
                    return iced::Task::none();
                }

                let Some(db) = self.db.clone() else {
                    self.error = Some(AppError::new("数据库不可用"));
                    return iced::Task::none();
                };
                let global_env = match crate::draft::env_rows_to_map(
                    &self.settings.global_env_rows,
                ) {
                    Ok(env) => env,
                    Err(error) => {
                        self.error = Some(AppError::new(
                            "全局环境变量无效",
                        ).with_detail(error.to_string()));
                        return iced::Task::none();
                    }
                };

                let request = launch::LaunchRequest {
                    game,
                    runner: prefix_plan.runner.clone(),
                    session: self.session,
                    global_env,
                    inherited_env: Self::inherited_env(),
                    steam_compat_client: self.paths.steam_compat_client.clone(),
                    default_prefix_root: self.settings.prefix_root.clone(),
                    gamescope_path: self.gamescope_path.clone(),
                };

                self.running.insert(game_id, prefix_plan.path);
                iced::Task::perform(tasks::launch_game(db, request), move |result| {
                    Message::LaunchFinished { game_id, result }
                })
            }
            Message::LaunchFinished { game_id, result } => {
                self.running.remove(&game_id);
                match result {
                    Ok(outcome) => {
                        let timestamp = chrono::Utc::now().timestamp();
                        if let Some(game) =
                            self.games.iter_mut().find(|game| game.id == game_id)
                        {
                            game.last_played_at = Some(timestamp);
                        }
                        if let Some(draft) = &mut self.draft {
                            if draft.id == game_id {
                                draft.last_played_at = Some(timestamp);
                            }
                        }
                        if !outcome.success {
                            self.error = Some(
                                AppError::new(format!(
                                    "游戏异常退出（退出码 {:?}）",
                                    outcome.exit_code
                                )),
                            );
                        }
                    }
                    Err(error) => {
                        self.error =
                            Some(AppError::new("启动失败").with_detail(error));
                    }
                }
                iced::Task::none()
            }
```

注意 `LaunchRequest.runner` 使用 `prefix_plan.runner`，它与 `launch::resolve_runner` 一致；不要重复解析导致两个 runner 来源不一致。

- [x] **Step 3: 在 GameDetail header 增加启动/删除按钮**

把 header 的按钮行改为：

```rust
    let header = row![
        button("返回").on_press(Message::Navigate(Screen::Library)),
        text(if draft.is_new { "新建游戏" } else { "游戏详情" }).size(24),
        text(if app.running.contains_key(&draft.id) { "运行中" } else { "" }).size(12),
        iced::widget::horizontal_space(),
        button("保存").on_press_maybe(
            (!app.pending.saving).then_some(Message::SaveDraft)
        ),
        button("启动").on_press_maybe(
            (!draft.is_new && !app.running.contains_key(&draft.id))
                .then_some(Message::Launch(draft.id))
        ),
        button("删除游戏").on_press_maybe(
            (!draft.is_new && !app.running.contains_key(&draft.id))
                .then_some(Message::ConfirmDelete(DeleteTarget::Game(draft.id)))
        ),
    ]
    .spacing(8);
```

- [x] **Step 4: 增加启动任务测试**

```rust
    #[tokio::test]
    async fn launch_runs_fake_wine_and_touches_last_played() {
        use display::SessionType;
        use launcher_core::{Arch, DisplayConfig, RunnerKind, RunnerSource};
        use std::collections::BTreeMap;

        let dir = tempdir().unwrap();
        let exe = dir.path().join("game.exe");
        fs::write(&exe, b"MZ").unwrap();

        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();

        let wine = dir.path().join("wine");
        fs::write(&wine, "#!/bin/sh\nexit 7\n").unwrap();
        let mut permissions = fs::metadata(&wine).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&wine, permissions).unwrap();

        let game = Game {
            id: Uuid::new_v4(),
            name: "Test".into(),
            executable_path: exe,
            working_dir: Some(dir.path().to_path_buf()),
            runner_id: None,
            prefix_path: Some(prefix),
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            env_overrides: BTreeMap::new(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 1,
            last_played_at: None,
        };
        let runner = Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: "fake".into(),
            binary_path: wine,
            root_dir: dir.path().to_path_buf(),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        };
        let request = launch::LaunchRequest {
            game: game.clone(),
            runner,
            session: SessionType::X11,
            global_env: BTreeMap::new(),
            inherited_env: BTreeMap::new(),
            steam_compat_client: dir.path().join("steam-client"),
            default_prefix_root: dir.path().join("prefixes"),
            gamescope_path: None,
        };

        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        db.lock().unwrap().insert_game(&game).unwrap();

        let outcome = launch_game(db.clone(), request).await.unwrap();
        assert_eq!(outcome.exit_code, Some(7));
        assert!(!outcome.success);
        let stored = db.lock().unwrap().get_game(game.id).unwrap().unwrap();
        assert!(stored.last_played_at.is_some());
    }
```



- [x] **Step 5: 运行测试**

Run: `cargo test -p ui`
Expected: 全部通过。

- [x] **Step 6: 提交**

```bash
git add crates/ui
git commit -m "feat(ui): launch games and track running state"
```

---

## Task U15: 打磨与手动验收

**Files:**
- Modify: `crates/ui/src/views/game_detail.rs`
- Modify: `CHANGELOG.md`

- [x] **Step 1: 增加 pending 提示与不可用说明**

- 当 `draft.is_new` 时，在 header 下显示：`text("保存游戏后才能启动、创建 Prefix 或安装 DXVK").size(12)`。
- 当 `app.prefix_status != Some(PrefixStatus::Initialized)` 时，在兼容层区块追加提示：`text("启动前必须有一个已初始化的 Prefix").size(12)`。
- 当 `app.gamescope_path.is_none()` 且 `draft.display.gamescope == Forced` 时，提示：“强制 gamescope 但未检测到 gamescope，启动会被拒绝”。
- 当 `app.gamescope_path.is_none()` 且 `draft.display.gamescope == Auto` 时，提示：“未检测到 gamescope，自动模式不会生效，分辨率/刷新率不会应用”。
- 当 `draft.env_rows` 中存在 `WINEDLLOVERRIDES` 时，保存会失败；在环境变量区块显示固定提示（已有文案）。
- 所有异步按钮在 pending 时通过 `on_press_maybe(None)` 禁用：保存、启动、创建 Prefix、安装 DXVK、删除。

- [x] **Step 2: 运行全量自动检查**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

Expected: 全部通过。

- [x] **Step 3: 手工端到端（本机 Wine 10.0）**

1. `cargo run -p ui`
2. Runner 管理 → 重新探测系统 Wine。
3. 新建游戏 → 选择 `/home/<user>/.local/share/gamehub/.../notepad.exe` 或任意测试 exe；先保存。
4. 兼容层选择系统 Wine → 创建 Prefix → 状态“已初始化”。
5. 显示：本机会话为 Wayland，显示器字段是文本输入；gamescope 关闭（本机未安装），分辨率/刷新率应显示说明文字。
6. 环境变量添加 `DXVK_HUD=fps`；DLL Override 添加 `d3d11` → `n,b`。
7. 启动；正常关闭 notepad（退出码 0）后 UI 不显示错误，`最近游玩` 更新；若人为制造非零退出码，UI 显示退出码错误。
8. 重启应用，确认 Runner/Prefix/显示/环境/DLL 全部恢复。
9. 删除游戏：运行中拒绝；退出后可删除。
10. 删除 Prefix：单游戏引用时成功；共享时拒绝。

- [x] **Step 4: 更新 CHANGELOG**

```markdown
## P9 — 配置与启动 UI

- 显示表单（X11 输出、Wayland 手动输出、gamescope、分辨率/刷新、全屏）。
- 环境变量行编辑、结构化 DLL override、启动参数行编辑。
- 启动：build_plan + spawn + 等待 + last_played_at 更新。
- 运行中状态、同 prefix 并发保护、退出码/错误展示。
- 游戏删除确认与运行中保护。

验证：`cargo test --workspace`；本机 Wine 10.0 端到端启动。
```

- [x] **Step 5: 提交**

```bash
git add crates/ui CHANGELOG.md
git commit -m "feat(ui): finish configuration editors and launch flow"
```

---

## P9 验收

- [x] `cargo test -p ui` 通过，`cargo test --workspace` 通过。
- [x] 显示配置可编辑并持久化；X11 非 gamescope 时分辨率输入被说明替代。
- [x] 环境变量/DLL/参数行可增删改；保存后重启保留。
- [x] 手动设置 `WINEDLLOVERRIDES` 会被草稿校验拒绝。
- [x] 启动成功后 `last_played_at` 更新；非零退出码在 UI 展示。
- [x] 同一 prefix 不允许并发启动。
- [x] 运行中的游戏不能删除、不能删除其 prefix。
- [x] 启动前校验覆盖：exe 缺失、runner 二进制缺失、working_dir 缺失、prefix 未初始化/arch 不匹配、强制 gamescope 不可用。
- [x] 关闭应用不会杀掉正在运行的游戏（子进程不在窗口关闭时被 kill）。

## 实施记录

- 环境：Cargo workspace（rustc 1.99.1），Iced 0.13.1；GNOME Wayland
  3840x1080（XDG_SESSION_TYPE=wayland），UI 以 XWayland 后端运行用于
  XTEST 自动化；Wine 10.0（Debian 10.0~repack-6），未安装 gamescope。
- 测试 exe：Wine 自带 `notepad.exe` 与 `cmd.exe`
  （`/usr/lib/x86_64-linux-gnu/wine/x86_64-windows/`）。
- 自动检查：`cargo test -p ui` 26 passed；`cargo test --workspace`
  156 passed、1 ignored；`cargo fmt --all --check`、
  `cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo build --workspace` 均通过。
- 手工验证：
  - Runner 管理重新探测得到 `wine-10.0` 并设为默认；新建游戏选择
    notepad.exe、保存成功。
  - Wine prefix 约 12 秒内初始化完成，状态刷新为“已初始化”。
  - 显示区按 Wayland 会话显示输出名文本输入；gamescope 缺失时自动模式
    提示生效，分辨率/刷新率被说明替代；全屏开关可编辑。
  - 环境变量 `DXVK_HUD=fps`、DLL Override `d3d11`（原生+内置）与启动
    参数 `/c exit 7` 保存后重启应用仍保留。
  - 草稿加入手写 `WINEDLLOVERRIDES` 后保存被拒绝，错误条展示
    “必须通过结构化 DLL override 配置”。
  - 启动 notepad：详情页显示“运行中”，运行期间“启动/删除游戏”不可用，
    删除 Prefix 被 `delete_target` 拒绝；子进程环境含 `DXVK_HUD=fps`
    与 `WINEDLLOVERRIDES=d3d11=n,b`。正常关闭后错误消失，`最近游玩`
    与 DB `last_played_at` 更新。
  - 启动 `cmd.exe /c exit 7` 后错误条显示“游戏异常退出（退出码 Some(7)）”，
    `last_played_at` 仍更新。
  - 同一 Prefix 并发保护：直接向测试 DB 插入第二个游戏（指向同一 prefix）
    后启动第二个游戏，得到“同一 Prefix 已有游戏在运行”。
  - 共享 Prefix 删除被拒绝；删除第二个游戏后，单游戏 Prefix 删除成功，
    DB 引用清空且目录移除。
  - 运行 notepad 时关闭 gameHub 窗口，应用退出而 notepad 进程继续运行。
- 偏差：
  - 手工环境用 XWayland 渲染，但会话检测仍按 `XDG_SESSION_TYPE` 判定为
    Wayland，因此显示器字段为文本输入；未在本机验证真实 Wayland
    winit 后端。
  - 本机无 gamescope，未做 gamescope 实际启动；强制模式拒绝路径由
    launch crate 的既有测试覆盖。
  - 同一 Prefix 并发验证的第二个游戏通过测试 DB 直接插入，而不是 UI
    新建（UI 新建与共享 Prefix 选择已在 P8 验证）。
  - 规格 U15 要求“所有异步按钮 pending 时禁用”；本次在 GameDetail
    为保存/启动/创建 Prefix/安装 DXVK/删除按钮补齐 (`pending` 条件)，
    Runner/Settings 页的既有禁用逻辑未改动。
  - `launch_game` 在进程退出后写 `last_played_at`；即使非零退出码也记录，
    与“最近游玩”语义一致。

## 当前实现（v0.10.0）

P9 的 `tasks::launch_game` 已由 `crates/application/src/launching.rs` 接管；
UI 只负责构造 `LaunchRequest`、消费事件并维护展示状态。

### 新建游戏与平台

- `NewGame` 先打开平台选择弹窗（Windows / Linux），确认后发送
  `BeginNewGame(platform)` 再打开文件对话框；`CancelNewGame` 关闭弹窗。
- `DialogIntent::PickExe(Platform::Windows)` 过滤 `*.exe`；
  `PickExe(Platform::Linux)` 不过滤扩展名。
- `GameDraft::new(executable_path, platform)`；编辑已有游戏时
  `GameDraft::from_game` 保留原平台。切换平台只保存字段，不改变已选的
  runner/prefix 等 Wine 配置。
- GameDetail 的显示区块：平台选择始终可见；Runner、Prefix、DXVK、
  DLL Override、winetricks 组件仅在 `Platform::Windows` 时渲染。
  原生游戏的“可执行文件”对话框不过滤 `.exe`。

### 启动契约（当前）

```rust
pub struct LaunchRequest {
    pub game: Game,
    pub runner: Option<Runner>, // 原生游戏为 None
    pub session: SessionType,
    pub global_env: BTreeMap<String, String>,
    pub inherited_env: BTreeMap<String, String>,
    pub steam_compat_client: PathBuf,
    pub default_prefix_root: PathBuf,
    pub gamescope_path: Option<PathBuf>,
    pub tuning: LaunchTuning,
    pub log_path: Option<PathBuf>,
    pub dxvk_nvapi_source: Option<PathBuf>,
}

pub struct LaunchTuning {
    pub enable_esync: bool,
    pub enable_fsync: bool,
    pub enable_mangohud: bool,
    pub mangohud_path: Option<PathBuf>,
    pub enable_dxvk_nvapi: bool,
}

pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stopped: bool,
    pub session_seconds: i64,
}

pub enum LaunchEvent {
    Started { game_id: Uuid, started_at: i64 },
    Finished { game_id: Uuid, outcome: LaunchOutcome },
    Failed { game_id: Uuid, error: String },
}
```

- UI 使用 `AppService::launch_game_with_events`，通过 `iced::stream` 把
  `LaunchEvent` 转成 `Message::LaunchEvent`；启动失败另有
  `Message::LaunchFailed` 兜底。
- `App.starting`/`App.stopping` 表示启动中/停止中；`App.running:
  HashMap<Uuid, Option<PathBuf>>` 仅用于展示（原生游戏为 `None`）。
- 启动前由应用层再次校验：游戏已保存；同一游戏不能重复启动；同一
  prefix 不能并发；Wine 游戏需要已初始化的 prefix 与可用 runner；
  全局/游戏环境变量合法且不手写 `WINEDLLOVERRIDES`；
  MangoHud 启用时必须已找到二进制；强制 gamescope 时必须可用。
- 原生游戏（`runner: None`）：`validate` 跳过 runner/prefix/DLL 检查，
  `build_plan` 直接以可执行文件 + 参数运行；`build_env` 不注入任何
  Wine/Proton 基础变量、esync/fsync、NVAPI 或 `WINEDLLOVERRIDES`，
  但显示器选择（`__GL_SYNC_DISPLAY_DEVICE`）以及 gamescope/MangoHud
  包装仍然生效。
- Wine 游戏在构建计划前，若 `tuning.enable_dxvk_nvapi` 为真，应用层先
  安装 DXVK-NVAPI（仅 Wine runner）；未配置来源时启动失败。
- 启动日志写入 `logs/games/<game-id>.log`，上一次内容保留为
  `.log.old`；stderr 行以 `[stderr]` 前缀记录。
- 会话结束判定同时检查 launcher 进程组与 prefix 内进程
  （`prefix_has_processes`），wineserver/Proton 包装器提前退出时不会
  误报游戏已结束。
- `last_played_at` 在启动成功时立即写入；退出后按实际
  `session_seconds` 调用 `add_playtime` 累加游玩时长（SQL 侧
  `MAX(playtime_seconds + ?, 0)` 保证非负）。被用户停止的会话
  （`stopped=true`）不显示“异常退出”提示。
- 停止：`Message::Stop` → `AppService::stop_game`，Wine 游戏先
  `wineserver -k` 关闭整个 prefix，再终止进程组；原生游戏只终止
  进程组。`StopFinished` 只在失败时显示错误。

### GameDetail 与日志

- 页头按钮：返回、保存、启动/停止（互斥）、删除游戏；`starting`/
  `stopping` 期间对应按钮禁用。
- 区块顺序：封面/概览、基本信息、兼容层与 Prefix（仅 Windows）、
  显示、环境变量、DLL Override（仅 Windows）、启动参数、组件
  （仅 Windows）。
- 封面：可选择/清除/恢复默认；保存时复制到应用数据目录的
  `covers/`，校验扩展名、可解码性、非零尺寸与最大边长 8192；
  Library 卡片与详情页共用预览。
- 概览展示 `最近游玩` 与 `游玩时长`
  （`widgets::format_playtime`）。
- 日志页（`Screen::Logs`）读取 `logs/games/<game-id>.log` 尾部
  `LOG_TAIL_LINES = 1000` 行，每秒自动刷新；Library 与详情页均可
  进入。

## 2026-10-31 当前实现：DLL Override 目录扫描

- DLL Override 区块新增「从目录添加」，与手动「添加 DLL Override」并存；
  默认扫描游戏可执行文件所在目录，优先使用该游戏上次选择的目录
  （settings 键 `dll_scan_dir.<game-id>`，扫描成功后才写入）。
- 扫描由 `application::scan_dlls` 在 `spawn_blocking` 中完成，`update`
  不执行文件系统 IO；上限为深度 16、结果 5000、访问文件 200000，
  超限时在对话框内提示。目录符号链接不跟随；非 UTF-8 路径跳过并计数。
- 结果按不含扩展名的小写基名去重，行内显示 DLL 名与相对路径；已有
  Override（含本次手动输入的草稿行）标为「已添加」并禁用复选框。
  支持实时搜索、名称排序、全选/反选/清空；列表渲染上限 300 行，
  超出时提示用搜索缩小范围。
- 每个对话框选择一个 Override 模式，确认后批量追加到草稿的
  `dll_rows`，复用 `save_draft` 持久化；校验或写库失败时回滚新增行、
  保留勾选并显示错误，成功后关闭对话框并显示添加数量。扫描可取消，
  关闭对话框会丢弃结果；过期扫描结果通过 `scan_seq` 忽略。
- 目录不存在、无权限、非目录、读取失败分别映射为可读错误文案；
  空目录显示「该目录下未找到 .dll 文件」。
